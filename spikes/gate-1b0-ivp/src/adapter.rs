//! `ivp` DOP853 spike adapter helpers.

use gate_1b0_contract::{
    CallbackTiming, DenseOutputAssessment, DenseOutputClass, IntegrationStats, SupportLevel,
};
use ivp::dense::StepInterpolant;
use ivp::methods::DOP853;
use ivp::solout::{ControlFlag, SolOut};
use std::cell::RefCell;
use std::rc::Rc;

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

#[derive(Clone, Default)]
pub struct CaptureLog {
    pub steps: Rc<RefCell<Vec<StepCapture>>>,
    pub callback_count: Rc<RefCell<u32>>,
    pub last_x: Rc<RefCell<f64>>,
    pub last_y: Rc<RefCell<Vec<f64>>>,
    pub stop_next: Rc<RefCell<bool>>,
}

impl CaptureLog {
    pub fn new(x0: f64, y0: Vec<f64>) -> Self {
        Self {
            steps: Rc::new(RefCell::new(Vec::new())),
            callback_count: Rc::new(RefCell::new(0)),
            last_x: Rc::new(RefCell::new(x0)),
            last_y: Rc::new(RefCell::new(y0)),
            stop_next: Rc::new(RefCell::new(false)),
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
            ControlFlag::Interrupt
        } else {
            ControlFlag::Continue
        }
    }
}

pub fn default_dop853() -> DOP853 {
    DOP853::builder().dense_output(true).build()
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

pub fn dense_assessment_ivp() -> DenseOutputAssessment {
    DenseOutputAssessment {
        classes_observed: vec![
            DenseOutputClass::AcceptedStepInterpolant,
            DenseOutputClass::GlobalSolutionQuery,
        ],
        callback_timing: CallbackTiming::AfterAcceptedStep,
        can_stop_from_callback: true,
        stats_at_callback: false,
        notes: "SolOut receives StepInterpolant; Solution::sol(t) after integrate.".into(),
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
