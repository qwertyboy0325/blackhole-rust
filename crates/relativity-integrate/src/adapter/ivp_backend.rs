//! Private `ivp` wiring — never re-exported.

use ivp::dense::StepInterpolant;
use ivp::ivp::FirstOrderSystem;
use ivp::methods::{Tolerance, DOP853};
use ivp::solout::{ControlFlag, SolOut};
use ivp::status::Status;
use relativity_core::KerrParams;
use std::cell::RefCell;
use std::rc::Rc;

use crate::config::Dop853Config;
use crate::error::{IntegrationError, IntegrationStage};
use crate::event::{
    is_eligible_crossing_tol, localize_sign_change, EventId, EventLocalizationStats, EventSurface,
};
use crate::outcome::{EventHit, IntegrationStats, RawSolverStop};
use crate::rhs::{DomainLatch, HamiltonianRhs};
use crate::state::{AffineParameter, GeodesicState};

pub struct IvpRhs {
    pub inner: HamiltonianRhs,
}

impl FirstOrderSystem for IvpRhs {
    fn derivative(&self, x: f64, y: &[f64], dydx: &mut [f64]) {
        self.inner.derivative(x, y, dydx);
    }
}

#[derive(Clone)]
pub(crate) struct PendingEvent {
    pub event_id: EventId,
    pub lambda: AffineParameter,
    pub state: GeodesicState,
    pub raw: RawSolverStop,
    pub event_value: f64,
    pub localization: EventLocalizationStats,
}

struct EventSolOut<'a> {
    surfaces: &'a [&'a dyn EventSurface],
    config: &'a Dop853Config,
    latch: DomainLatch,
    last_y: Rc<RefCell<Vec<f64>>>,
    last_lam: Rc<RefCell<f64>>,
    callback_count: Rc<RefCell<u64>>,
    steps_after_interrupt: Rc<RefCell<u64>>,
    interrupted: Rc<RefCell<bool>>,
    pending: Rc<RefCell<Option<PendingEvent>>>,
    endpoint_h: Rc<RefCell<Vec<f64>>>,
    endpoint_pt: Rc<RefCell<Vec<f64>>>,
    params: KerrParams,
}

impl SolOut for EventSolOut<'_> {
    fn solout(
        &mut self,
        xold: f64,
        x: &mut f64,
        y: &mut [f64],
        interpolant: Option<&StepInterpolant<'_>>,
    ) -> ControlFlag {
        *self.callback_count.borrow_mut() += 1;
        if *self.interrupted.borrow() {
            *self.steps_after_interrupt.borrow_mut() += 1;
            return ControlFlag::Continue;
        }

        if *self.callback_count.borrow() > self.config.max_accepted_steps {
            self.latch.set(IntegrationError::StepLimitExceeded {
                accepted_steps: *self.callback_count.borrow(),
            });
            *self.interrupted.borrow_mut() = true;
            return ControlFlag::Interrupt;
        }

        let y0 = self.last_y.borrow().clone();
        let Ok(state0) = GeodesicState::from_array(&y0) else {
            self.latch.set(IntegrationError::NonFiniteState {
                stage: IntegrationStage::Callback,
            });
            *self.interrupted.borrow_mut() = true;
            return ControlFlag::Interrupt;
        };
        let Ok(state1) = GeodesicState::from_array(y) else {
            self.latch.set(IntegrationError::NonFiniteState {
                stage: IntegrationStage::Callback,
            });
            *self.interrupted.borrow_mut() = true;
            return ControlFlag::Interrupt;
        };

        // Accepted-endpoint invariant samples (diagnostics only).
        if let Ok(h) = crate::rhs::initial_hamiltonian(&self.params, &state1) {
            self.endpoint_h.borrow_mut().push(h);
        }
        self.endpoint_pt.borrow_mut().push(state1.momentum.t);

        let Some(interp) = interpolant else {
            *self.last_y.borrow_mut() = y.to_vec();
            *self.last_lam.borrow_mut() = *x;
            return ControlFlag::Continue;
        };

        let (lam_lo, lam_hi) = interp.bounds();
        let mut best: Option<PendingEvent> = None;

        for surface in self.surfaces {
            let f0 = match surface.value(AffineParameter(xold), &state0) {
                Ok(v) => v,
                Err(e) => {
                    self.latch.set(e);
                    *self.interrupted.borrow_mut() = true;
                    return ControlFlag::Interrupt;
                }
            };
            let f1 = match surface.value(AffineParameter(*x), &state1) {
                Ok(v) => v,
                Err(e) => {
                    self.latch.set(e);
                    *self.interrupted.borrow_mut() = true;
                    return ControlFlag::Interrupt;
                }
            };
            if !is_eligible_crossing_tol(
                f0,
                f1,
                surface.crossing(),
                self.config.event_value_tolerance,
            ) {
                continue;
            }

            let raw = RawSolverStop {
                lambda: AffineParameter(*x),
                state: state1,
            };

            // Endpoint capture: surface reached within value tolerance without a
            // strict interior sign change (typical f64 horizon approach).
            if f0 * f1 >= 0.0 && f1.abs() <= self.config.event_value_tolerance {
                let cand = PendingEvent {
                    event_id: surface.id(),
                    lambda: AffineParameter(*x),
                    state: state1,
                    raw: raw.clone(),
                    event_value: f1,
                    localization: EventLocalizationStats {
                        interpolation_calls: 0,
                        final_bracket_width: 0.0,
                        iterations: 0,
                    },
                };
                let take = match &best {
                    None => true,
                    Some(b) => cand.lambda.0 < b.lambda.0,
                };
                if take {
                    best = Some(cand);
                }
                continue;
            }

            let interp_fn = |lam: f64| -> Result<GeodesicState, IntegrationError> {
                let mut yi = vec![0.0; 8];
                interp.interpolate(lam, &mut yi);
                GeodesicState::from_array(&yi)
            };
            let event_fn = |lam: AffineParameter, st: &GeodesicState| surface.value(lam, st);

            match localize_sign_change(
                lam_lo,
                lam_hi,
                &state0,
                &state1,
                f0,
                f1,
                &interp_fn,
                &event_fn,
                self.config.event_time_tolerance,
                self.config.event_value_tolerance,
            ) {
                Ok((lam, st, fv, loc)) => {
                    let cand = PendingEvent {
                        event_id: surface.id(),
                        lambda: lam,
                        state: st,
                        raw,
                        event_value: fv,
                        localization: loc,
                    };
                    let take = match &best {
                        None => true,
                        Some(b) => cand.lambda.0 < b.lambda.0,
                    };
                    if take {
                        best = Some(cand);
                    }
                }
                Err(e) => {
                    self.latch.set(e);
                    *self.interrupted.borrow_mut() = true;
                    return ControlFlag::Interrupt;
                }
            }
        }

        *self.last_y.borrow_mut() = y.to_vec();
        *self.last_lam.borrow_mut() = *x;

        if let Some(ev) = best {
            *self.pending.borrow_mut() = Some(ev);
            *self.interrupted.borrow_mut() = true;
            return ControlFlag::Interrupt;
        }

        ControlFlag::Continue
    }
}

pub(crate) struct BackendResult {
    pub pending: Option<PendingEvent>,
    pub final_lambda: f64,
    pub final_state: GeodesicState,
    pub stats: IntegrationStats,
    pub interrupted: bool,
    pub steps_after_interrupt: u64,
    pub endpoint_h: Vec<f64>,
    pub endpoint_pt: Vec<f64>,
    pub non_finite_checks: u64,
}

pub(crate) fn integrate_ivp(
    params: KerrParams,
    y0: &GeodesicState,
    config: &Dop853Config,
    surfaces: &[&dyn EventSurface],
) -> Result<BackendResult, IntegrationError> {
    let latch = DomainLatch::new();
    let rhs = HamiltonianRhs::new(params, latch.clone());
    let eval_count = rhs.eval_count.clone();
    let non_finite_checks = rhs.non_finite_checks.clone();
    let system = IvpRhs { inner: rhs.share() };

    let y0_arr = y0.to_array();
    let last_y = Rc::new(RefCell::new(y0_arr.to_vec()));
    let last_lam = Rc::new(RefCell::new(0.0));
    let callback_count = Rc::new(RefCell::new(0u64));
    let steps_after = Rc::new(RefCell::new(0u64));
    let interrupted = Rc::new(RefCell::new(false));
    let pending = Rc::new(RefCell::new(None));
    let endpoint_h = Rc::new(RefCell::new(Vec::new()));
    let endpoint_pt = Rc::new(RefCell::new(Vec::new()));

    let mut solout = EventSolOut {
        surfaces,
        config,
        latch: latch.clone(),
        last_y: last_y.clone(),
        last_lam: last_lam.clone(),
        callback_count: callback_count.clone(),
        steps_after_interrupt: steps_after.clone(),
        interrupted: interrupted.clone(),
        pending: pending.clone(),
        endpoint_h: endpoint_h.clone(),
        endpoint_pt: endpoint_pt.clone(),
        params,
    };

    let solver = DOP853::builder()
        .dense_output(true)
        .max_step(config.max_step)
        .build();

    let rtol = Tolerance::Vector(config.relative_tolerance.to_vec());
    let atol = Tolerance::Vector(config.absolute_tolerance.to_vec());

    let result = solver.solve(
        &system,
        0.0,
        &y0_arr,
        config.affine_limit,
        rtol,
        atol,
        Some(&mut solout),
    );

    // Typed error ordering: project latch → non-finite → solver status.
    if let Some(err) = latch.take() {
        return Err(err);
    }

    let final_y = last_y.borrow().clone();
    if final_y.iter().any(|v| !v.is_finite()) {
        return Err(IntegrationError::NonFiniteState {
            stage: IntegrationStage::Outcome,
        });
    }
    let final_state = GeodesicState::from_array(&final_y)?;

    match result {
        Ok(res) => {
            let interrupted_flag = matches!(res.status, Status::UserInterrupt);
            let stats = IntegrationStats {
                accepted_steps: res.steps.accepted as u64,
                rejected_steps: res.steps.rejected as u64,
                rhs_evaluations: *eval_count.borrow(),
                callback_count: *callback_count.borrow(),
            };
            let mut pending_ev = pending.borrow().clone();

            // Stall recovery: adaptive step collapsed (typical near r₊ in f64 KS)
            // while a Decreasing surface is already within value tolerance.
            if !res.status.is_success() && pending_ev.is_none() {
                if matches!(res.status, Status::StepSizeTooSmall) {
                    let lam = AffineParameter(*last_lam.borrow());
                    for surface in surfaces {
                        if surface.crossing() != crate::event::CrossingDirection::Decreasing {
                            continue;
                        }
                        let f = surface.value(lam, &final_state)?;
                        if f.is_finite() && f.abs() <= config.event_value_tolerance {
                            pending_ev = Some(PendingEvent {
                                event_id: surface.id(),
                                lambda: lam,
                                state: final_state,
                                raw: RawSolverStop {
                                    lambda: lam,
                                    state: final_state,
                                },
                                event_value: f,
                                localization: EventLocalizationStats {
                                    interpolation_calls: 0,
                                    final_bracket_width: 0.0,
                                    iterations: 0,
                                },
                            });
                            break;
                        }
                    }
                }
                if pending_ev.is_none() {
                    return Err(IntegrationError::Solver {
                        detail: format!("{:?}", res.status),
                    });
                }
            }
            Ok(BackendResult {
                pending: pending_ev,
                final_lambda: *last_lam.borrow(),
                final_state,
                stats,
                interrupted: interrupted_flag || pending.borrow().is_some(),
                steps_after_interrupt: *steps_after.borrow(),
                endpoint_h: endpoint_h.borrow().clone(),
                endpoint_pt: endpoint_pt.borrow().clone(),
                non_finite_checks: *non_finite_checks.borrow(),
            })
        }
        Err(e) => Err(IntegrationError::Solver {
            detail: format!("{e:?}"),
        }),
    }
}

pub(crate) fn pending_to_event_hit(p: PendingEvent, stats: IntegrationStats) -> EventHit {
    EventHit {
        event_id: p.event_id,
        lambda: p.lambda,
        state: p.state,
        raw_solver_stop: p.raw,
        event_value: p.event_value,
        localization: p.localization,
        integration: stats,
    }
}
