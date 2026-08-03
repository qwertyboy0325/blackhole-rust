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
    is_eligible_crossing, localize_sign_change, EventId, EventLocalizationStats, EventSurface,
};
use crate::outcome::{
    EventHit, IntegrationStats, RawSolverStop, SurfaceApproach, SurfaceApproachReason,
};
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

#[derive(Clone)]
pub(crate) struct PendingApproach {
    pub event_id: EventId,
    pub lambda: AffineParameter,
    pub state: GeodesicState,
    pub signed_event_value: f64,
    pub approach_tolerance: f64,
    pub reason: SurfaceApproachReason,
    pub raw: RawSolverStop,
}

#[derive(Clone)]
pub(crate) enum PendingTermination {
    ExactEvent(PendingEvent),
    SurfaceApproach(PendingApproach),
}

/// Project-owned solver status class (no `ivp` in public API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SolverStatusClass {
    Success,
    UserInterrupt,
    StepSizeTooSmall,
    OtherFailure(String),
}

impl SolverStatusClass {
    pub fn from_ivp(status: Status) -> Self {
        match status {
            Status::Success => Self::Success,
            Status::UserInterrupt => Self::UserInterrupt,
            Status::StepSizeTooSmall => Self::StepSizeTooSmall,
            other => Self::OtherFailure(format!("{other:?}")),
        }
    }

    pub fn is_success_or_interrupt(&self) -> bool {
        matches!(self, Self::Success | Self::UserInterrupt)
    }
}

/// Interpret solver status after latch/non-finite checks.
/// Returns `Err(Solver)` for non-domain generic failures without approach capture.
pub(crate) fn interpret_solver_status(status: &SolverStatusClass) -> Result<(), IntegrationError> {
    match status {
        SolverStatusClass::Success | SolverStatusClass::UserInterrupt => Ok(()),
        SolverStatusClass::StepSizeTooSmall => Err(IntegrationError::Solver {
            detail: "StepSizeTooSmall".into(),
        }),
        SolverStatusClass::OtherFailure(detail) => Err(IntegrationError::Solver {
            detail: detail.clone(),
        }),
    }
}

/// Require a finite outcome state vector (backend-level interpreter).
pub(crate) fn require_finite_outcome_state(y: &[f64]) -> Result<GeodesicState, IntegrationError> {
    if y.iter().any(|v| !v.is_finite()) {
        return Err(IntegrationError::NonFiniteState {
            stage: IntegrationStage::Outcome,
        });
    }
    GeodesicState::from_array(y).map_err(|_| IntegrationError::NonFiniteState {
        stage: IntegrationStage::Outcome,
    })
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
    pending: Rc<RefCell<Option<PendingTermination>>>,
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
        let mut best_event: Option<PendingEvent> = None;

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

            let raw = RawSolverStop {
                lambda: AffineParameter(*x),
                state: state1,
            };

            // Exact event kernel only (no proximity → Event).
            if is_eligible_crossing(f0, f1, surface.crossing()) {
                let interp_fn = |lam: f64| -> Result<GeodesicState, IntegrationError> {
                    let mut yi = vec![0.0; 8];
                    interp.interpolate(lam, &mut yi);
                    GeodesicState::from_array(&yi)
                };
                let event_fn = |lam: AffineParameter, st: &GeodesicState| surface.value(lam, st);
                match localize_sign_change(
                    surface.id(),
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
                            raw: raw.clone(),
                            event_value: fv,
                            localization: loc,
                        };
                        let take = match &best_event {
                            None => true,
                            Some(b) => cand.lambda.0 < b.lambda.0,
                        };
                        if take {
                            best_event = Some(cand);
                        }
                    }
                    Err(e) => {
                        self.latch.set(e);
                        *self.interrupted.borrow_mut() = true;
                        return ControlFlag::Interrupt;
                    }
                }
            }
            let _ = raw;
        }

        *self.last_y.borrow_mut() = y.to_vec();
        *self.last_lam.borrow_mut() = *x;

        if let Some(ev) = best_event {
            *self.pending.borrow_mut() = Some(PendingTermination::ExactEvent(ev));
            *self.interrupted.borrow_mut() = true;
            return ControlFlag::Interrupt;
        }

        ControlFlag::Continue
    }
}

pub(crate) struct BackendResult {
    pub pending: Option<PendingTermination>,
    pub final_lambda: f64,
    pub final_state: GeodesicState,
    pub stats: IntegrationStats,
    pub interrupted: bool,
    pub steps_after_interrupt: u64,
    pub endpoint_h: Vec<f64>,
    pub endpoint_pt: Vec<f64>,
    pub non_finite_checks: u64,
    #[allow(dead_code)]
    pub solver_status: SolverStatusClass,
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
    let final_state = require_finite_outcome_state(&final_y)?;

    match result {
        Ok(res) => {
            let status = SolverStatusClass::from_ivp(res.status);
            let interrupted_flag = matches!(status, SolverStatusClass::UserInterrupt);
            let stats = IntegrationStats {
                accepted_steps: res.steps.accepted as u64,
                rejected_steps: res.steps.rejected as u64,
                rhs_evaluations: *eval_count.borrow(),
                callback_count: *callback_count.borrow(),
            };
            let mut pending_term = pending.borrow().clone();

            // Stall → SurfaceApproach only under opt-in OuterHorizon proximity.
            if !status.is_success_or_interrupt() && pending_term.is_none() {
                if matches!(status, SolverStatusClass::StepSizeTooSmall) {
                    let pol = &config.horizon_proximity;
                    if pol.enabled {
                        let lam = AffineParameter(*last_lam.borrow());
                        for surface in surfaces {
                            if surface.id() != EventId::OuterHorizon {
                                continue;
                            }
                            let f = surface.value(lam, &final_state)?;
                            if f.is_finite() && f > 0.0 && f <= pol.approach_tolerance {
                                pending_term =
                                    Some(PendingTermination::SurfaceApproach(PendingApproach {
                                        event_id: EventId::OuterHorizon,
                                        lambda: lam,
                                        state: final_state,
                                        signed_event_value: f,
                                        approach_tolerance: pol.approach_tolerance,
                                        reason: SurfaceApproachReason::SolverStepSizeTooSmall,
                                        raw: RawSolverStop {
                                            lambda: lam,
                                            state: final_state,
                                        },
                                    }));
                                break;
                            }
                        }
                    }
                }
                if pending_term.is_none() {
                    interpret_solver_status(&status)?;
                }
            }

            Ok(BackendResult {
                pending: pending_term,
                final_lambda: *last_lam.borrow(),
                final_state,
                stats,
                interrupted: interrupted_flag || pending.borrow().is_some(),
                steps_after_interrupt: *steps_after.borrow(),
                endpoint_h: endpoint_h.borrow().clone(),
                endpoint_pt: endpoint_pt.borrow().clone(),
                non_finite_checks: *non_finite_checks.borrow(),
                solver_status: status,
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

pub(crate) fn pending_to_surface_approach(
    p: PendingApproach,
    stats: IntegrationStats,
) -> SurfaceApproach {
    SurfaceApproach {
        event_id: p.event_id,
        lambda: p.lambda,
        state: p.state,
        signed_event_value: p.signed_event_value,
        approach_tolerance: p.approach_tolerance,
        reason: p.reason,
        raw_solver_stop: p.raw,
        integration: stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_finite_outcome_state_typed() {
        let mut y = [1.0; 8];
        y[2] = f64::NAN;
        let err = require_finite_outcome_state(&y).unwrap_err();
        assert!(matches!(
            err,
            IntegrationError::NonFiniteState {
                stage: IntegrationStage::Outcome
            }
        ));
    }

    #[test]
    fn generic_solver_failure_stays_solver() {
        let err = interpret_solver_status(&SolverStatusClass::OtherFailure("ProbablyStiff".into()))
            .unwrap_err();
        assert!(matches!(err, IntegrationError::Solver { .. }));
        assert!(!matches!(err, IntegrationError::PhysicsDomain { .. }));
        assert!(!matches!(err, IntegrationError::EventDomain { .. }));
    }

    #[test]
    fn step_size_too_small_without_policy_is_solver() {
        let err = interpret_solver_status(&SolverStatusClass::StepSizeTooSmall).unwrap_err();
        assert!(matches!(err, IntegrationError::Solver { .. }));
    }
}
