//! Real accepted-step event loop for `ivp` Experiment E.

use gate_1b0_contract::{sho_analytic_p, sho_analytic_q};
use gate_1b0_contract::{RootLocalizationEvidence, SolverStopEvidence, SHO_EVENT_X};
use ivp::dense::StepInterpolant;
use ivp::methods::{IntegrationResult, Tolerance, DOP853};
use ivp::prelude::FirstOrderSystem;
use ivp::solout::{ControlFlag, SolOut};
use ivp::status::Status;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct EventLoopCapture {
    pub root: Rc<RefCell<Option<RootLocalizationEvidence>>>,
    pub stop: Rc<RefCell<Option<SolverStopEvidence>>>,
    pub callback_count: Rc<RefCell<u32>>,
    pub interrupted: Rc<RefCell<bool>>,
    pub steps_after_interrupt: Rc<RefCell<u32>>,
    pub last_y: Rc<RefCell<Vec<f64>>>,
}

impl EventLoopCapture {
    pub fn new(_x0: f64, y0: Vec<f64>) -> Self {
        Self {
            root: Rc::new(RefCell::new(None)),
            stop: Rc::new(RefCell::new(None)),
            callback_count: Rc::new(RefCell::new(0)),
            interrupted: Rc::new(RefCell::new(false)),
            steps_after_interrupt: Rc::new(RefCell::new(0)),
            last_y: Rc::new(RefCell::new(y0)),
        }
    }

    pub fn fill_stop_stats(&self, res: &IntegrationResult) {
        if let Some(ref mut stop) = *self.stop.borrow_mut() {
            stop.accepted_steps_at_stop = res.steps.accepted as u32;
            stop.rejected_steps_at_stop = res.steps.rejected as u32;
            stop.rhs_evaluations_at_stop = res.evals.ode as u32;
            stop.no_steps_after_stop = *self.steps_after_interrupt.borrow() == 0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn localize_on_ivp_interp(
    t0: f64,
    t1: f64,
    y0: &[f64],
    y1: &[f64],
    interp: &StepInterpolant<'_>,
    event: &dyn Fn(f64, &[f64]) -> f64,
    event_time_analytic: f64,
    analytic_at_event: &[f64],
    shallow: bool,
) -> RootLocalizationEvidence {
    let n = y0.len();
    let mut interp_calls = 0u32;
    let mut lo_t = t0;
    let mut hi_t = t1;
    let mut lo_f = event(lo_t, y0);
    let mut hi_f = event(hi_t, y1);

    let sample = |t: f64, interp: &StepInterpolant<'_>, calls: &mut u32| -> Vec<f64> {
        let mut yi = vec![0.0; n];
        interp.interpolate(t, &mut yi);
        *calls += 1;
        yi
    };

    if lo_f.signum() == hi_f.signum() {
        for k in 1..=16 {
            let theta = k as f64 / 16.0;
            let t_mid = t0 + theta * (t1 - t0);
            let y_mid = sample(t_mid, interp, &mut interp_calls);
            let f_mid = event(t_mid, &y_mid);
            if lo_f.signum() != f_mid.signum() {
                hi_t = t_mid;
                hi_f = f_mid;
                break;
            }
            lo_t = t_mid;
            lo_f = f_mid;
        }
    }

    let mut root_t = 0.5 * (lo_t + hi_t);
    for _ in 0..64 {
        root_t = 0.5 * (lo_t + hi_t);
        let y_root = sample(root_t, interp, &mut interp_calls);
        let f_root = event(root_t, &y_root);
        if lo_f.signum() != f_root.signum() {
            hi_t = root_t;
            hi_f = f_root;
        } else {
            lo_t = root_t;
            lo_f = f_root;
        }
        if (hi_t - lo_t).abs() < 1e-12 {
            break;
        }
    }

    let y_event = sample(root_t, interp, &mut interp_calls);
    let root_residual = event(root_t, &y_event).abs();
    let time_error = (root_t - event_time_analytic).abs();
    let state_error = y_event
        .iter()
        .zip(analytic_at_event.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);

    RootLocalizationEvidence {
        event_time_analytic,
        event_time_found: root_t,
        time_error,
        root_residual,
        state_error,
        interpolation_calls: interp_calls,
        localized_state: y_event,
        shallow_crossing_tested: shallow,
        shallow_sign_change_only_insufficient: shallow && lo_f.signum() == hi_f.signum(),
    }
}

pub struct ShoEventSolOut {
    pub cap: EventLoopCapture,
}

impl SolOut for ShoEventSolOut {
    fn solout(
        &mut self,
        xold: f64,
        x: &mut f64,
        y: &mut [f64],
        interpolant: Option<&StepInterpolant<'_>>,
    ) -> ControlFlag {
        *self.cap.callback_count.borrow_mut() += 1;
        if *self.cap.interrupted.borrow() {
            *self.cap.steps_after_interrupt.borrow_mut() += 1;
            return ControlFlag::Continue;
        }

        let y0 = self.cap.last_y.borrow().clone();
        let event = |_t: f64, state: &[f64]| state[0];

        if let Some(interp) = interpolant {
            let f0 = event(xold, &y0);
            let f1 = event(*x, y);
            if f0.signum() != f1.signum() {
                let (x_lo, x_hi) = interp.bounds();
                let root = localize_on_ivp_interp(
                    x_lo,
                    x_hi,
                    &y0,
                    y,
                    interp,
                    &event,
                    SHO_EVENT_X,
                    &[sho_analytic_q(SHO_EVENT_X), sho_analytic_p(SHO_EVENT_X)],
                    false,
                );
                *self.cap.root.borrow_mut() = Some(root.clone());
                *self.cap.stop.borrow_mut() = Some(SolverStopEvidence {
                    interrupted: true,
                    stop_time: root.event_time_found,
                    stop_state: root.localized_state.clone(),
                    callback_count_at_stop: *self.cap.callback_count.borrow(),
                    accepted_steps_at_stop: 0,
                    rejected_steps_at_stop: 0,
                    rhs_evaluations_at_stop: 0,
                    no_steps_after_stop: true,
                });
                *self.cap.interrupted.borrow_mut() = true;
                return ControlFlag::Interrupt;
            }
        }

        *self.cap.last_y.borrow_mut() = y.to_vec();
        ControlFlag::Continue
    }
}

pub fn run_sho_event_stop<F>(
    sys: &F,
    x0: f64,
    y0: &[f64],
    xend: f64,
    rtol: f64,
    atol: f64,
) -> Result<(IntegrationResult, EventLoopCapture), ivp::error::Error>
where
    F: FirstOrderSystem,
{
    let cap = EventLoopCapture::new(x0, y0.to_vec());
    let mut solout = ShoEventSolOut { cap: cap.clone() };
    let solver = DOP853::builder().dense_output(true).build();
    let res = solver.solve(
        sys,
        x0,
        y0,
        xend,
        Tolerance::Scalar(rtol),
        Tolerance::Scalar(atol),
        Some(&mut solout),
    )?;
    cap.fill_stop_stats(&res);
    Ok((res, cap))
}

pub fn interrupted_ok(res: &IntegrationResult) -> bool {
    matches!(res.status, Status::UserInterrupt)
}

/// Shallow-crossing event loop using `StepInterpolant` in the accepted-step callback.
pub fn run_shallow_event_localize(
    x0: f64,
    y0: &[f64],
    xend: f64,
    rtol: f64,
    atol: f64,
) -> Result<Option<RootLocalizationEvidence>, ivp::error::Error> {
    let cap = EventLoopCapture::new(x0, y0.to_vec());
    let mut solout = ShallowEventSolOut { cap: cap.clone() };
    let solver = DOP853::builder().dense_output(true).build();
    let _ = solver.solve(
        &ShoSys,
        x0,
        y0,
        xend,
        Tolerance::Scalar(rtol),
        Tolerance::Scalar(atol),
        Some(&mut solout),
    )?;
    let root = cap.root.borrow().clone();
    Ok(root)
}

struct ShoSys;

impl FirstOrderSystem for ShoSys {
    fn derivative(&self, _x: f64, y: &[f64], dy: &mut [f64]) {
        dy[0] = y[1];
        dy[1] = -y[0];
    }
}

struct ShallowEventSolOut {
    cap: EventLoopCapture,
}

impl SolOut for ShallowEventSolOut {
    fn solout(
        &mut self,
        xold: f64,
        x: &mut f64,
        y: &mut [f64],
        interpolant: Option<&StepInterpolant<'_>>,
    ) -> ControlFlag {
        *self.cap.callback_count.borrow_mut() += 1;
        let y0 = self.cap.last_y.borrow().clone();
        let event = |t: f64, _y: &[f64]| gate_1b0_contract::shallow_event_fn(t);

        if let Some(interp) = interpolant {
            let f0 = event(xold, &y0);
            let f1 = event(*x, y);
            if f0.signum() != f1.signum() {
                let (x_lo, x_hi) = interp.bounds();
                let root = localize_on_ivp_interp(
                    x_lo,
                    x_hi,
                    &y0,
                    y,
                    interp,
                    &event,
                    gate_1b0_contract::shallow_event_root_analytic(),
                    &[gate_1b0_contract::shallow_event_fn(
                        gate_1b0_contract::shallow_event_root_analytic(),
                    )],
                    true,
                );
                *self.cap.root.borrow_mut() = Some(root);
            }
        }

        *self.cap.last_y.borrow_mut() = y.to_vec();
        ControlFlag::Continue
    }
}
