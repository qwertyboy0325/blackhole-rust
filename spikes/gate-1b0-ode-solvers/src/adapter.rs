//! `ode_solvers::Dop853` spike adapter helpers.

use gate_1b0_contract::{
    CallbackTiming, DenseOutputAssessment, DenseOutputClass, IntegrationStats, SupportLevel,
};
use nalgebra::DVector;
use ode_solvers::dop853::Dop853;
use ode_solvers::dop_shared::{OutputType, Stats};
use ode_solvers::System;
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

pub struct CapturingSystem<F> {
    pub inner: F,
    pub log: CaptureLog,
}

impl<F> CapturingSystem<F> {
    pub fn new(inner: F, x0: f64, y0: Vec<f64>) -> Self {
        Self {
            inner,
            log: CaptureLog::new(x0, y0),
        }
    }
}

impl<F> System<f64, DVector<f64>> for CapturingSystem<F>
where
    F: System<f64, DVector<f64>>,
{
    fn system(&self, x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
        self.inner.system(x, y, dy);
    }

    fn solout(&mut self, x: f64, y: &DVector<f64>, _dy: &DVector<f64>) -> bool {
        *self.log.callback_count.borrow_mut() += 1;
        let y1 = y.as_slice().to_vec();
        let last_x = *self.log.last_x.borrow();
        let h = x - last_x;
        if *self.log.callback_count.borrow() > 1 {
            self.log.steps.borrow_mut().push(StepCapture {
                x0: last_x,
                x1: x,
                y0: self.log.last_y.borrow().clone(),
                y1: y1.clone(),
                h,
            });
        }
        *self.log.last_x.borrow_mut() = x;
        *self.log.last_y.borrow_mut() = y1;
        *self.log.stop_next.borrow()
    }
}

pub fn make_stepper<F>(
    system: F,
    x0: f64,
    x_end: f64,
    y0: DVector<f64>,
    dx: f64,
    h_max: f64,
) -> Dop853<f64, DVector<f64>, F>
where
    F: System<f64, DVector<f64>>,
{
    Dop853::from_param(
        system,
        x0,
        x_end,
        dx,
        y0,
        DEFAULT_RTOL,
        DEFAULT_ATOL,
        0.9,
        0.0,
        0.333,
        6.0,
        h_max,
        0.0,
        100_000,
        1000,
        OutputType::Dense,
    )
}

pub fn stats_to_integration(stats: Stats, min_h: f64, final_h: f64) -> IntegrationStats {
    IntegrationStats {
        accepted_steps: stats.accepted_steps,
        rejected_steps: stats.rejected_steps,
        rhs_evaluations: stats.num_eval,
        final_step_size: final_h,
        min_step_size: min_h,
    }
}

pub fn dense_assessment_ode_solvers() -> DenseOutputAssessment {
    DenseOutputAssessment {
        classes_observed: vec![
            DenseOutputClass::PredeterminedSamples,
            DenseOutputClass::GlobalSolutionQuery,
        ],
        callback_timing: CallbackTiming::AfterAcceptedStep,
        can_stop_from_callback: true,
        stats_at_callback: false,
        notes: "solout receives endpoint state only; rcont coefficients are private. Dense \
                samples emitted on fixed dx grid via OutputType::Dense."
            .into(),
    }
}

pub fn error_scaling_ode_solvers() -> gate_1b0_contract::ErrorScalingAssessment {
    gate_1b0_contract::ErrorScalingAssessment {
        norm_type: "component-scaled RMS: sqrt(sum((y_i/(atol+rtol|y_i|))^2)/N)".into(),
        dimension_dependent: true,
        absolute_relative_formula: "scale_i = atol + rtol * |y_i|".into(),
        zero_component_behavior: "atol dominates when |y_i| ~ 0".into(),
        position_momentum_notes: "single scalar atol/rtol for all components; pos/mom adapter \
                                  requires external state rescaling"
            .into(),
        scaling_visible_or_configurable: true,
        state_rescaling_changes_dense_semantics: true,
        direct_vector_tolerance: SupportLevel::Unsupported,
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

pub fn interpolate_dense_grid(x_out: &[f64], y_out: &[DVector<f64>], x_query: f64) -> Option<f64> {
    if x_out.is_empty() {
        return None;
    }
    if x_query <= x_out[0] {
        return Some(y_out[0][0]);
    }
    let last = x_out.len() - 1;
    if x_query >= x_out[last] {
        return Some(y_out[last][0]);
    }
    for i in 0..last {
        let x0 = x_out[i];
        let x1 = x_out[i + 1];
        if x_query >= x0 && x_query <= x1 {
            let t = (x_query - x0) / (x1 - x0);
            return Some(y_out[i][0] + t * (y_out[i + 1][0] - y_out[i][0]));
        }
    }
    None
}
