//! `ivp` DOP853 spike adapter helpers.

use gate_1b0_contract::{
    AcceptedStepProbe, CallbackTiming, DenseOutputAssessment, DenseOutputClass, IntegrationStats,
    SupportLevel, DOMAIN_X_MAX,
};
use ivp::dense::StepInterpolant;
use ivp::methods::{IntegrationResult, Tolerance, DOP853};
use ivp::prelude::FirstOrderSystem;
use ivp::solout::{ControlFlag, SolOut};
use std::cell::RefCell;
use std::rc::Rc;

pub const DOMAIN_ERROR_CODE: &str = "DOMAIN_X_EXCEEDED";

#[derive(Clone, Default)]
pub struct DomainLatch(pub Rc<RefCell<Option<String>>>);

impl DomainLatch {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }
}

pub struct DomainSys {
    pub latch: DomainLatch,
}

impl FirstOrderSystem for DomainSys {
    fn derivative(&self, x: f64, y: &[f64], dy: &mut [f64]) {
        if x >= DOMAIN_X_MAX {
            *self.latch.0.borrow_mut() = Some(DOMAIN_ERROR_CODE.into());
            dy[0] = f64::NAN;
            return;
        }
        dy[0] = y[0];
    }
}

pub const DEFAULT_RTOL: f64 = 1e-9;
pub const DEFAULT_ATOL: f64 = 1e-12;

#[derive(Clone, Default)]
pub struct StepCapture {
    pub x0: f64,
    pub x1: f64,
    pub y0: Vec<f64>,
    pub y1: Vec<f64>,
    #[allow(dead_code)]
    pub h: f64,
    pub had_interpolant: bool,
}

#[derive(Clone)]
pub struct CaptureLog {
    pub steps: Rc<RefCell<Vec<StepCapture>>>,
    pub callback_count: Rc<RefCell<u32>>,
    pub last_x: Rc<RefCell<f64>>,
    pub last_y: Rc<RefCell<Vec<f64>>>,
    pub stop_next: Rc<RefCell<bool>>,
    pub stop_requested: Rc<RefCell<bool>>,
    pub accepted_after_stop: Rc<RefCell<u32>>,
}

impl CaptureLog {
    pub fn new(x0: f64, y0: Vec<f64>) -> Self {
        Self {
            steps: Rc::new(RefCell::new(Vec::new())),
            callback_count: Rc::new(RefCell::new(0)),
            last_x: Rc::new(RefCell::new(x0)),
            last_y: Rc::new(RefCell::new(y0)),
            stop_next: Rc::new(RefCell::new(false)),
            stop_requested: Rc::new(RefCell::new(false)),
            accepted_after_stop: Rc::new(RefCell::new(0)),
        }
    }
}

pub struct CapturingSolOut {
    pub log: CaptureLog,
}

impl SolOut for CapturingSolOut {
    fn solout(
        &mut self,
        xold: f64,
        x: &mut f64,
        y: &mut [f64],
        interpolant: Option<&StepInterpolant<'_>>,
    ) -> ControlFlag {
        *self.log.callback_count.borrow_mut() += 1;
        if *self.log.stop_requested.borrow() {
            *self.log.accepted_after_stop.borrow_mut() += 1;
        }
        if *self.log.callback_count.borrow() > 1 {
            self.log.steps.borrow_mut().push(StepCapture {
                x0: xold,
                x1: *x,
                y0: self.log.last_y.borrow().clone(),
                y1: y.to_vec(),
                h: *x - xold,
                had_interpolant: interpolant.is_some(),
            });
        }
        *self.log.last_x.borrow_mut() = *x;
        *self.log.last_y.borrow_mut() = y.to_vec();
        if *self.log.stop_next.borrow() {
            *self.log.stop_requested.borrow_mut() = true;
            ControlFlag::Interrupt
        } else {
            ControlFlag::Continue
        }
    }
}

pub struct DenseProbeSolOut<F> {
    pub log: CaptureLog,
    pub analytic: F,
    pub probes: Rc<RefCell<Vec<AcceptedStepProbe>>>,
}

impl<F> SolOut for DenseProbeSolOut<F>
where
    F: Fn(f64) -> Vec<f64>,
{
    fn solout(
        &mut self,
        xold: f64,
        x: &mut f64,
        y: &mut [f64],
        interpolant: Option<&StepInterpolant<'_>>,
    ) -> ControlFlag {
        *self.log.callback_count.borrow_mut() += 1;
        if let Some(interp) = interpolant {
            let (x_lo, x_hi) = interp.bounds();
            let n = y.len();
            for &theta in &[0.1, 0.25, 0.5, 0.75, 0.9] {
                let t = x_lo + theta * (x_hi - x_lo);
                let mut yi = vec![0.0; n];
                interp.interpolate(t, &mut yi);
                let ya = (self.analytic)(t);
                let max_abs = yi
                    .iter()
                    .zip(ya.iter())
                    .map(|(c, a)| (c - a).abs())
                    .fold(0.0_f64, f64::max);
                let max_rel = yi
                    .iter()
                    .zip(ya.iter())
                    .map(|(c, a)| {
                        let scale = a.abs().max(c.abs()).max(1e-12);
                        (c - a).abs() / scale
                    })
                    .fold(0.0_f64, f64::max);
                self.probes.borrow_mut().push(AcceptedStepProbe {
                    step_x0: x_lo,
                    step_x1: x_hi,
                    theta,
                    t,
                    computed: yi,
                    analytic: ya,
                    max_abs_error: max_abs,
                    max_rel_error: max_rel,
                });
            }
        }
        if *self.log.callback_count.borrow() > 1 {
            self.log.steps.borrow_mut().push(StepCapture {
                x0: xold,
                x1: *x,
                y0: self.log.last_y.borrow().clone(),
                y1: y.to_vec(),
                h: *x - xold,
                had_interpolant: interpolant.is_some(),
            });
        }
        *self.log.last_y.borrow_mut() = y.to_vec();
        ControlFlag::Continue
    }
}

pub fn default_dop853() -> DOP853 {
    DOP853::builder().dense_output(true).build()
}

pub fn dop853_with_max_step(h_max: f64) -> DOP853 {
    DOP853::builder().dense_output(true).max_step(h_max).build()
}

pub fn solve_dop853<F>(
    sys: &F,
    x0: f64,
    y0: &[f64],
    xend: f64,
    rtol: f64,
    atol: f64,
    h_max: Option<f64>,
) -> Result<(IntegrationResult, Vec<f64>), ivp::error::Error>
where
    F: FirstOrderSystem,
{
    let log = CaptureLog::new(x0, y0.to_vec());
    let mut solout = CapturingSolOut { log: log.clone() };
    let solver = match h_max {
        Some(h) => dop853_with_max_step(h),
        None => default_dop853(),
    };
    let res = solver.solve(
        sys,
        x0,
        y0,
        xend,
        Tolerance::Scalar(rtol),
        Tolerance::Scalar(atol),
        Some(&mut solout),
    )?;
    Ok((res, {
        let final_y = log.last_y.borrow().clone();
        final_y
    }))
}

#[allow(clippy::too_many_arguments)]
pub fn solve_dop853_solout<F, S>(
    sys: &F,
    x0: f64,
    y0: &[f64],
    xend: f64,
    rtol: f64,
    atol: f64,
    solout: &mut S,
    h_max: Option<f64>,
) -> Result<IntegrationResult, ivp::error::Error>
where
    F: FirstOrderSystem,
    S: SolOut,
{
    let solver = match h_max {
        Some(h) => dop853_with_max_step(h),
        None => default_dop853(),
    };
    solver.solve(
        sys,
        x0,
        y0,
        xend,
        Tolerance::Scalar(rtol),
        Tolerance::Scalar(atol),
        Some(solout),
    )
}

pub fn stats_from_result(res: &ivp::methods::IntegrationResult) -> IntegrationStats {
    IntegrationStats {
        accepted_steps: res.steps.accepted as u32,
        rejected_steps: res.steps.rejected as u32,
        rhs_evaluations: res.evals.ode as u32,
        final_step_size: res.h,
        min_step_size: res.h,
    }
}

pub fn dense_assessment_ivp(observed_interp: bool) -> DenseOutputAssessment {
    DenseOutputAssessment {
        classes_observed: if observed_interp {
            vec![
                DenseOutputClass::AcceptedStepInterpolant,
                DenseOutputClass::GlobalSolutionQuery,
            ]
        } else {
            vec![DenseOutputClass::GlobalSolutionQuery]
        },
        callback_timing: CallbackTiming::AfterAcceptedStep,
        can_stop_from_callback: true,
        stats_at_callback: false,
        notes: "SolOut StepInterpolant evaluated at arbitrary theta in callback.".into(),
    }
}

pub fn error_scaling_ivp() -> gate_1b0_contract::ErrorScalingAssessment {
    gate_1b0_contract::ErrorScalingAssessment {
        norm_type: "per-component scale atol[i]+rtol[i]*|y[i|]; RMS over components".into(),
        dimension_dependent: true,
        absolute_relative_formula: "scale_i = atol[i] + rtol[i] * |y_i|".into(),
        zero_component_behavior: "atol[i] dominates".into(),
        position_momentum_notes: "Tolerance::Vector supports per-component scales directly".into(),
        scaling_visible_or_configurable: true,
        state_rescaling_changes_dense_semantics: false,
        direct_vector_tolerance: SupportLevel::Supported,
        adapter_scaled_tolerance: SupportLevel::SupportedWithAdapter,
    }
}

pub fn component_errors(y: &[f64], analytic: &[f64]) -> Vec<gate_1b0_contract::ComponentError> {
    y.iter()
        .zip(analytic.iter())
        .enumerate()
        .map(|(index, (&computed, &analytic))| {
            let abs = (computed - analytic).abs();
            let scale = analytic.abs().max(computed.abs()).max(1e-12);
            gate_1b0_contract::ComponentError {
                index,
                abs,
                rel: abs / scale,
                analytic,
                computed,
            }
        })
        .collect()
}

pub fn endpoint_errors(y: f64, analytic: f64) -> (f64, f64) {
    let abs = (y - analytic).abs();
    let scale = analytic.abs().max(y.abs()).max(1e-12);
    (abs, abs / scale)
}
