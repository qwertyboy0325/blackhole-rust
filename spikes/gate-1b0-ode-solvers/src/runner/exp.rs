//! Per-experiment implementations for `ode_solvers`.

use crate::adapter::{
    component_errors, dense_assessment_ode_solvers, endpoint_errors, interpolate_dense_grid,
    interpolate_dense_state, make_stepper, stats_to_integration, CapturingSystem, DEFAULT_RTOL,
};
use crate::domain_adapter::domain_error_evidence;
use gate_1b0_contract::{
    endpoint_bits, exp_analytic, mixed8_analytic, mixed8_derivative, mixed8_y0, repeat_in_process,
    repeat_in_process_sig, shallow_event_fn, shallow_event_root_analytic, sho_analytic_energy,
    sho_analytic_p, sho_analytic_q, sho_energy, signature_join, EXP_LAMBDA, EXP_X0, EXP_X_END,
    EXP_Y0, MIXED8_DIM, SHO_EVENT_X, SHO_P0, SHO_Q0, SHO_X0, SHO_X_END,
};
use gate_1b0_contract::{
    localize_root, CallbackStopEvidence, DenseProbe, DeterminismRecord, ExperimentId,
    ExperimentResult, RepeatSummary, RestartEvidence, SolverStopEvidence, StepGuardAssessment,
    SupportLevel,
};
use nalgebra::DVector;
use ode_solvers::dop853::Dop853;
use ode_solvers::System;

struct ExpSys {
    lambda: f64,
}
impl System<f64, DVector<f64>> for ExpSys {
    fn system(&self, _x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
        dy[0] = self.lambda * y[0];
    }
}

struct ShoSys;
impl System<f64, DVector<f64>> for ShoSys {
    fn system(&self, _x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
        dy[0] = y[1];
        dy[1] = -y[0];
    }
}

struct Mixed8Sys;
impl System<f64, DVector<f64>> for Mixed8Sys {
    fn system(&self, x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
        let mut buf = [0.0; MIXED8_DIM];
        mixed8_derivative(x, y.as_slice(), &mut buf);
        dy.copy_from_slice(&buf);
    }
}

fn det_record(det: RepeatSummary) -> DeterminismRecord {
    DeterminismRecord {
        in_process_runs: det.signatures.len() as u32,
        signatures: det.signatures,
        endpoint_bits: det.endpoint_bits,
        accepted_steps: det.accepted_steps,
        json_digests: det.json_digests,
        deterministic: det.deterministic,
    }
}

fn exp_a_determinism() -> RepeatSummary {
    repeat_in_process(5, || {
        let mut s = make_stepper(
            ExpSys { lambda: EXP_LAMBDA },
            EXP_X0,
            EXP_X_END,
            DVector::from_vec(vec![EXP_Y0]),
            0.05,
            EXP_X_END,
        );
        let st = s.integrate().unwrap();
        let ye = s.y_out().last().unwrap()[0];
        (vec![ye], st.accepted_steps, endpoint_bits(&[ye]))
    })
}

pub fn run_a() -> ExperimentResult {
    let inner = ExpSys { lambda: EXP_LAMBDA };
    let y0 = DVector::from_vec(vec![EXP_Y0]);
    let mut stepper = make_stepper(inner, EXP_X0, EXP_X_END, y0, 0.05, EXP_X_END);
    let stats = stepper.integrate().expect("exp integrate");
    let y_end = stepper.y_out().last().map(|v| v[0]).unwrap_or(f64::NAN);
    let analytic = exp_analytic(EXP_X_END);
    let (abs, rel) = endpoint_errors(y_end, analytic);
    let mut dense_probes = Vec::new();
    let x_out = stepper.x_out().clone();
    let y_out = stepper.y_out().clone();
    for &theta in &[0.25, 0.5, 0.75] {
        let xq = EXP_X0 + theta * (EXP_X_END - EXP_X0);
        if let Some(yq) = interpolate_dense_grid(&x_out, &y_out, xq) {
            let ya = exp_analytic(xq);
            let dabs = (yq - ya).abs();
            let scale = ya.abs().max(yq.abs()).max(1e-12);
            dense_probes.push(DenseProbe {
                theta,
                t: xq,
                abs_error: dabs,
                rel_error: dabs / scale,
            });
        }
    }
    let det = exp_a_determinism();
    ExperimentResult {
        id: ExperimentId::A,
        passed: abs < 1e-6 && rel < 1e-6 && det.deterministic,
        detail: format!(
            "endpoint abs={abs:.3e} rel={rel:.3e} nacc={}",
            stats.accepted_steps
        ),
        endpoint_abs_error: Some(abs),
        endpoint_rel_error: Some(rel),
        component_errors: vec![],
        dense_probes,
        accepted_step_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: None,
        root_localization: None,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}

fn sho_determinism() -> RepeatSummary {
    repeat_in_process(5, || {
        let cap = CapturingSystem::new(ShoSys, SHO_X0, vec![SHO_Q0, SHO_P0]);
        let log = cap.log.clone();
        let mut s = make_stepper(
            cap,
            SHO_X0,
            SHO_X_END,
            DVector::from_vec(vec![SHO_Q0, SHO_P0]),
            0.1,
            0.1,
        );
        let st = s.integrate().unwrap();
        let x_out = s.x_out().clone();
        let y_out = s.y_out().clone();
        let _x_final = *x_out.last().unwrap_or(&SHO_X_END);
        let endpoint = y_out
            .last()
            .map(|v| v.as_slice().to_vec())
            .unwrap_or_else(|| log.last_y.borrow().clone());
        (
            endpoint.clone(),
            st.accepted_steps,
            endpoint_bits(&endpoint),
        )
    })
}

pub fn run_b() -> ExperimentResult {
    let inner = ShoSys;
    let cap = CapturingSystem::new(inner, SHO_X0, vec![SHO_Q0, SHO_P0]);
    let log = cap.log.clone();
    let y0 = DVector::from_vec(vec![SHO_Q0, SHO_P0]);
    let mut stepper = make_stepper(cap, SHO_X0, SHO_X_END, y0, 0.1, 0.1);
    let stats = stepper.integrate().expect("sho integrate");
    let x_out = stepper.x_out().clone();
    let y_out = stepper.y_out().clone();
    let x_final = *x_out.last().unwrap_or(&SHO_X_END);
    let yf = y_out
        .last()
        .map(|v| v.as_slice().to_vec())
        .unwrap_or_else(|| log.last_y.borrow().clone());
    let q_end = yf[0];
    let p_end = yf[1];
    let qa = sho_analytic_q(x_final);
    let pa = sho_analytic_p(x_final);
    let endpoint_abs = ((q_end - qa).powi(2) + (p_end - pa).powi(2)).sqrt();
    let energy_drift = (sho_energy(q_end, p_end) - sho_analytic_energy()).abs();
    let mut dense_probes = Vec::new();
    for &theta in &[0.3, 0.6] {
        let xq = SHO_X0 + theta * (SHO_X_END - SHO_X0);
        if x_out.windows(2).any(|w| xq >= w[0] && xq <= w[1]) {
            if let Some(i) = x_out.iter().position(|&x| x >= xq) {
                if i > 0 {
                    let yq = y_out[i][0];
                    let ya = sho_analytic_q(xq);
                    let dabs = (yq - ya).abs();
                    dense_probes.push(DenseProbe {
                        theta,
                        t: xq,
                        abs_error: dabs,
                        rel_error: dabs / ya.abs().max(1e-12),
                    });
                }
            }
        }
    }
    let event = |_t: f64, y: &[f64]| y[0];
    let root_localization = log.steps.borrow().first().map(|s| {
        localize_root(
            s.x0,
            s.x1,
            &s.y0,
            &s.y1,
            &event,
            None,
            SHO_EVENT_X,
            &[0.0, sho_analytic_p(SHO_EVENT_X)],
            false,
        )
    });
    let det = sho_determinism();
    ExperimentResult {
        id: ExperimentId::B,
        passed: endpoint_abs < 1e-4 && energy_drift < 1e-3 && det.deterministic,
        detail: format!(
            "endpoint={endpoint_abs:.3e} energy_drift={energy_drift:.3e} x_final={x_final:.6} root_only"
        ),
        endpoint_abs_error: Some(endpoint_abs),
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes,
        accepted_step_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: None,
        root_localization,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}

fn mixed8_determinism() -> RepeatSummary {
    repeat_in_process(5, || {
        let mut s = make_stepper(
            Mixed8Sys,
            0.0,
            0.5,
            DVector::from_vec(mixed8_y0().to_vec()),
            0.02,
            0.5,
        );
        let st = s.integrate().unwrap();
        let endpoint = s.y_out().last().unwrap().as_slice().to_vec();
        (
            endpoint.clone(),
            st.accepted_steps,
            endpoint_bits(&endpoint),
        )
    })
}

pub fn run_c() -> ExperimentResult {
    let y0 = DVector::from_vec(mixed8_y0().to_vec());
    let x_end = 0.5;
    let mut stepper = make_stepper(Mixed8Sys, 0.0, x_end, y0, 0.02, x_end);
    let stats = stepper.integrate().expect("mixed8");
    let y_end = stepper.y_out().last().unwrap().as_slice().to_vec();
    let analytic = mixed8_analytic(x_end);
    let comps = component_errors(&y_end, &analytic);
    let max_abs = comps.iter().map(|c| c.abs).fold(0.0_f64, f64::max);
    let det = mixed8_determinism();
    ExperimentResult {
        id: ExperimentId::C,
        passed: comps.iter().all(|c| c.abs < 1e-5) && det.deterministic,
        detail: format!(
            "scalar_tol max_abs={max_abs:.3e}; direct_vector=Unsupported; adapter rescale tested separately"
        ),
        endpoint_abs_error: Some(max_abs),
        endpoint_rel_error: None,
        component_errors: comps,
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: None,
        root_localization: None,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: Some(crate::adapter::error_scaling_ode_solvers()),
    }
}

pub fn run_c_adapter() -> ExperimentResult {
    let scales: [f64; MIXED8_DIM] = mixed8_y0().map(|v| v.abs().max(1e-6));
    struct ScaledMixed8 {
        scales: [f64; MIXED8_DIM],
    }
    impl System<f64, DVector<f64>> for ScaledMixed8 {
        fn system(&self, x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
            let mut phys = vec![0.0; MIXED8_DIM];
            let mut dphys = [0.0; MIXED8_DIM];
            for i in 0..MIXED8_DIM {
                phys[i] = y[i] * self.scales[i];
            }
            mixed8_derivative(x, &phys, &mut dphys);
            for i in 0..MIXED8_DIM {
                dy[i] = dphys[i] / self.scales[i];
            }
        }
    }
    let y0 = DVector::from_vec(vec![1.0; MIXED8_DIM]);
    let x_end = 0.5;
    let mut stepper = make_stepper(ScaledMixed8 { scales }, 0.0, x_end, y0, 0.02, x_end);
    let stats = stepper.integrate().expect("scaled mixed8");
    let y_scaled = stepper.y_out().last().unwrap().as_slice();
    let y_end: Vec<f64> = y_scaled
        .iter()
        .zip(scales.iter())
        .map(|(y, s)| y * s)
        .collect();
    let analytic = mixed8_analytic(x_end);
    let comps = component_errors(&y_end, &analytic);
    ExperimentResult {
        id: ExperimentId::C,
        passed: comps.iter().all(|c| c.abs < 1e-4),
        detail: format!(
            "adapter_rescale max_abs={:.3e}",
            comps.iter().map(|c| c.abs).fold(0.0, f64::max)
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: comps,
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        root_localization: None,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}

fn dense_d_determinism() -> RepeatSummary {
    repeat_in_process_sig(5, || {
        let cap = CapturingSystem::new(ExpSys { lambda: EXP_LAMBDA }, EXP_X0, vec![EXP_Y0]);
        let log = cap.log.clone();
        let mut s = make_stepper(
            cap,
            EXP_X0,
            EXP_X_END,
            DVector::from_vec(vec![EXP_Y0]),
            0.01,
            EXP_X_END,
        );
        let st = s.integrate().unwrap();
        let sig = signature_join(&[
            &log.callback_count.borrow().to_string(),
            &log.steps.borrow().len().to_string(),
            &s.x_out().len().to_string(),
            &st.accepted_steps.to_string(),
        ]);
        (sig.clone(), st.accepted_steps, sig)
    })
}

pub fn run_d() -> ExperimentResult {
    let inner = ExpSys { lambda: EXP_LAMBDA };
    let cap = CapturingSystem::new(inner, EXP_X0, vec![EXP_Y0]);
    let log = cap.log.clone();
    let y0 = DVector::from_vec(vec![EXP_Y0]);
    let mut stepper = make_stepper(cap, EXP_X0, EXP_X_END, y0, 0.01, EXP_X_END);
    let stats = stepper.integrate().expect("dense access");
    let assessment = dense_assessment_ode_solvers();
    let callbacks = *log.callback_count.borrow();
    let steps_captured = log.steps.borrow().len();
    let dense_grid = stepper.x_out().len();
    let det = dense_d_determinism();
    let passed = callbacks > 0
        && steps_captured > 0
        && dense_grid > 1
        && det.deterministic
        && assessment
            .classes_observed
            .contains(&gate_1b0_contract::DenseOutputClass::PredeterminedSamples);
    ExperimentResult {
        id: ExperimentId::D,
        passed,
        detail: format!(
            "callbacks={callbacks} steps_captured={steps_captured} dense_grid={dense_grid}; \
             no AcceptedStepInterpolant"
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: Some(det_record(det)),
        dense_assessment: Some(assessment),
        step_guard: None,
        root_localization: None,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}

pub fn run_e() -> ExperimentResult {
    let inner = ShoSys;
    let y0 = DVector::from_vec(vec![1.0, 0.0]);
    let x_end = SHO_EVENT_X + 1.0;
    let mut stepper = make_stepper(inner, 0.0, x_end, y0.clone(), 0.005, 2.0);
    let stats = stepper.integrate().expect("sho grid integrate");
    let x_out = stepper.x_out();
    let y_out = stepper.y_out();
    let event = |_t: f64, y: &[f64]| y[0];
    let mut lo = 0.0;
    let mut hi = x_end;
    let mut y_lo = y0.as_slice().to_vec();
    let mut y_hi = interpolate_dense_state(x_out, y_out, hi).unwrap_or_else(|| y_lo.clone());
    for _ in 0..48 {
        let mid = 0.5 * (lo + hi);
        let y_mid = interpolate_dense_state(x_out, y_out, mid).expect("dense mid");
        if event(lo, &y_lo).signum() != event(mid, &y_mid).signum() {
            hi = mid;
            y_hi = y_mid;
        } else {
            lo = mid;
            y_lo = y_mid;
        }
    }
    let root_localization = localize_root(
        lo,
        hi,
        &y_lo,
        &y_hi,
        &event,
        None,
        SHO_EVENT_X,
        &[0.0, sho_analytic_p(SHO_EVENT_X)],
        false,
    );
    let y_final = interpolate_dense_state(x_out, y_out, x_end).unwrap_or_default();
    // No event adapter: raw = integration endpoint; localized is post-hoc grid only.
    let solver_stop = SolverStopEvidence {
        interrupted: false,
        raw_solver_stop_time: x_end,
        raw_solver_stop_state: y_final.clone(),
        localized_event_time: Some(root_localization.event_time_found),
        localized_event_state: Some(root_localization.localized_state.clone()),
        adapter_returned_time: x_end,
        adapter_returned_state: y_final.clone(),
        adapter_matches_localized: false,
        callback_count_at_stop: 0,
        accepted_steps_at_stop: stats.accepted_steps,
        rejected_steps_at_stop: stats.rejected_steps,
        rhs_evaluations_at_stop: stats.num_eval,
        no_steps_after_stop: true,
    };
    let reference_endpoint = vec![sho_analytic_q(SHO_X_END), sho_analytic_p(SHO_X_END)];
    let (restart_endpoint, endpoint_error) = {
        let y_restart = DVector::from_vec(vec![
            0.0,
            sho_analytic_p(root_localization.event_time_found),
        ]);
        let mut s2 = make_stepper(
            ShoSys,
            root_localization.event_time_found,
            SHO_X_END,
            y_restart,
            0.1,
            SHO_X_END,
        );
        if s2.integrate().is_ok() {
            let ep = s2.y_out().last().unwrap().as_slice().to_vec();
            let err = ep
                .iter()
                .zip(reference_endpoint.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max);
            (ep, err)
        } else {
            (vec![], f64::INFINITY)
        }
    };
    let restart = RestartEvidence {
        restart_time: root_localization.event_time_found,
        restart_state: root_localization.localized_state.clone(),
        restart_endpoint: restart_endpoint.clone(),
        reference_endpoint: reference_endpoint.clone(),
        endpoint_error,
        deterministic: false,
        endpoint_bits: if restart_endpoint.is_empty() {
            vec![]
        } else {
            vec![endpoint_bits(&restart_endpoint)]
        },
        in_process_runs: 0,
    };
    let det = repeat_in_process_sig(5, || {
        let mut s = make_stepper(
            ShoSys,
            0.0,
            x_end,
            DVector::from_vec(vec![1.0, 0.0]),
            0.005,
            2.0,
        );
        let st = s.integrate().unwrap();
        let ye = s.y_out().last().unwrap().as_slice().to_vec();
        (
            endpoint_bits(&ye),
            st.accepted_steps,
            signature_join(&[&ye[0].to_bits().to_string(), &ye[1].to_bits().to_string()]),
        )
    });
    let passed = root_localization.time_error < 1e-4
        && root_localization.root_residual < 1e-6
        && det.deterministic;
    ExperimentResult {
        id: ExperimentId::E,
        passed,
        detail: format!(
            "grid root time_err={:.3e} residual={:.3e}; solver_stop.interrupted=false; \
             restart not demonstrated (deterministic=false)",
            root_localization.time_error, root_localization.root_residual
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: None,
        root_localization: Some(root_localization),
        solver_stop: Some(solver_stop),
        restart: Some(restart),
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}

pub fn run_e_shallow() -> ExperimentResult {
    struct ShallowSys;
    impl System<f64, DVector<f64>> for ShallowSys {
        fn system(&self, _x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
            dy[0] = y[1];
            dy[1] = -y[0];
        }
    }
    let cap = CapturingSystem::new(ShallowSys, 0.0, vec![0.99, 0.0]);
    let log = cap.log.clone();
    let y0 = DVector::from_vec(vec![0.99, 0.0]);
    let x_end = 0.5;
    let mut stepper = make_stepper(cap, 0.0, x_end, y0, 0.01, x_end);
    let _ = stepper.integrate();
    let event = |t: f64, _y: &[f64]| shallow_event_fn(t);
    let root_localization = log.steps.borrow().iter().find_map(|s| {
        let f0 = event(s.x0, &s.y0);
        let f1 = event(s.x1, &s.y1);
        if f0.signum() != f1.signum() {
            Some(localize_root(
                s.x0,
                s.x1,
                &s.y0,
                &s.y1,
                &event,
                None,
                shallow_event_root_analytic(),
                &[shallow_event_fn(shallow_event_root_analytic())],
                true,
            ))
        } else {
            None
        }
    });
    ExperimentResult {
        id: ExperimentId::E,
        passed: root_localization.is_some(),
        detail: "shallow_sign_changing_crossing grid localization (not tangent)".into(),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: None,
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        root_localization,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}

fn callback_stop_evidence() -> (CallbackStopEvidence, bool) {
    let cap = CapturingSystem::new(ExpSys { lambda: EXP_LAMBDA }, EXP_X0, vec![EXP_Y0]);
    let log = cap.log.clone();
    *log.stop_next.borrow_mut() = true;
    let y0 = DVector::from_vec(vec![EXP_Y0]);
    let mut stepper = make_stepper(cap, EXP_X0, EXP_X_END, y0, 0.02, EXP_X_END);
    let stats = stepper.integrate().expect("callback stop integrate");
    let x_out = stepper.x_out();
    let y_out = stepper.y_out();
    let stop_time = *x_out.last().unwrap_or(&EXP_X0);
    let stop_state = y_out
        .last()
        .map(|y| y.as_slice().to_vec())
        .unwrap_or_else(|| vec![EXP_Y0]);
    let accepted_before = stats
        .accepted_steps
        .saturating_sub(*log.accepted_after_stop.borrow());
    let det = repeat_in_process_sig(5, || {
        let cap = CapturingSystem::new(ExpSys { lambda: EXP_LAMBDA }, EXP_X0, vec![EXP_Y0]);
        let inner_log = cap.log.clone();
        *inner_log.stop_next.borrow_mut() = true;
        let mut s = make_stepper(
            cap,
            EXP_X0,
            EXP_X_END,
            DVector::from_vec(vec![EXP_Y0]),
            0.02,
            EXP_X_END,
        );
        let st = s.integrate().unwrap();
        let xt = *s.x_out().last().unwrap();
        (
            signature_join(&[&xt.to_bits().to_string(), &st.accepted_steps.to_string()]),
            st.accepted_steps,
            xt.to_bits().to_string(),
        )
    });
    let interrupted = stop_time < EXP_X_END - 1e-9;
    let evidence = CallbackStopEvidence {
        callback_invoked: *log.callback_count.borrow() > 0,
        interrupt_requested: *log.interrupt_requested.borrow(),
        interrupted,
        stop_time,
        stop_state,
        accepted_steps_before_stop: accepted_before,
        accepted_steps_after_stop: *log.accepted_after_stop.borrow(),
        deterministic: det.deterministic,
    };
    let ok = evidence.callback_invoked
        && evidence.interrupt_requested
        && evidence.interrupted
        && evidence.accepted_steps_after_stop == 0
        && det.deterministic;
    (evidence, ok)
}

pub fn run_f() -> ExperimentResult {
    let h_max = 0.05;
    let (callback_stop, cb_ok) = callback_stop_evidence();
    let (domain_error, domain_ok) = domain_error_evidence();
    let det = repeat_in_process_sig(5, || {
        let cap = CapturingSystem::new(ExpSys { lambda: EXP_LAMBDA }, EXP_X0, vec![EXP_Y0]);
        let inner_log = cap.log.clone();
        *inner_log.stop_next.borrow_mut() = true;
        let mut s = make_stepper(
            cap,
            EXP_X0,
            EXP_X_END,
            DVector::from_vec(vec![EXP_Y0]),
            0.02,
            EXP_X_END,
        );
        let st = s.integrate().unwrap();
        let (ev, ok) = domain_error_evidence();
        let sig = signature_join(&[
            &st.accepted_steps.to_string(),
            &ok.to_string(),
            &ev.caller_error_variant,
            &ev.latched_error_code,
        ]);
        (sig.clone(), st.accepted_steps, sig)
    });
    let guard = StepGuardAssessment {
        static_h_max: SupportLevel::Supported,
        dynamic_h_max: SupportLevel::Unsupported,
        pre_rhs_domain_reject: SupportLevel::Unsupported,
        post_accepted_step_stop: SupportLevel::Supported,
        stop_from_callback: SupportLevel::Supported,
        bracket_recovery: SupportLevel::SupportedWithAdapter,
        typed_domain_failure: if domain_ok {
            SupportLevel::Supported
        } else {
            SupportLevel::Unsupported
        },
        notes: "solout halt; solve_with_domain_adapter returns SpikeAdapterError::Domain".into(),
    };
    ExperimentResult {
        id: ExperimentId::F,
        passed: cb_ok && domain_ok && det.deterministic,
        detail: format!(
            "callback_stop interrupted={} domain_variant={} latched={} non_finite_rejected={} h_max={h_max}",
            callback_stop.interrupted,
            domain_error.caller_error_variant,
            domain_error.latched_error_code,
            domain_error.non_finite_nominal_rejected
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: None,
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: Some(guard),
        root_localization: None,
        solver_stop: None,
        restart: None,
        callback_stop: Some(callback_stop),
        domain_error: Some(domain_error),
        error_scaling: None,
    }
}

fn kerr_determinism(tight: bool) -> RepeatSummary {
    repeat_in_process_sig(5, || {
        let sig = kerr_signature(tight);
        (sig.clone(), 0, sig)
    })
}

fn kerr_signature(tight: bool) -> String {
    use relativity_core::{
        evaluate_hamiltonian, initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams,
        PositionBl, SensorCoord,
    };
    let mass = 1.0;
    let spin = 0.9;
    let params = KerrParams::new(mass, spin).expect("params");
    let bl = PositionBl::new(0.0, 500.0, std::f64::consts::FRAC_PI_2, 0.0);
    let obs = zamo_observer(&params, &bl).expect("zamo");
    let cam = CameraParams {
        horizontal_fov: 60.0_f64.to_radians(),
        roll: 0.0,
    };
    let ray = initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 })
        .expect("ray");
    let pos = obs.event;
    let p = ray.covariant_momentum;
    let h0 = evaluate_hamiltonian(&params, &pos, &p).expect("H0");

    struct Kerr8 {
        params: KerrParams,
    }
    impl System<f64, DVector<f64>> for Kerr8 {
        fn system(&self, _lam: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
            let pos = relativity_core::PositionKs::new(y[0], y[1], y[2], y[3]);
            let pc = relativity_core::Covector::from_components([y[4], y[5], y[6], y[7]]);
            if let Ok(ev) = evaluate_hamiltonian(&self.params, &pos, &pc) {
                dy[0] = ev.dx_dlambda.t;
                dy[1] = ev.dx_dlambda.x;
                dy[2] = ev.dx_dlambda.y;
                dy[3] = ev.dx_dlambda.z;
                dy[4] = ev.dp_dlambda.t;
                dy[5] = ev.dp_dlambda.x;
                dy[6] = ev.dp_dlambda.y;
                dy[7] = ev.dp_dlambda.z;
            } else {
                dy.fill(f64::NAN);
            }
        }
    }
    let y0 = DVector::from_vec(vec![pos.t, pos.x, pos.y, pos.z, p.t, p.x, p.y, p.z]);
    let lam_end = 0.01;
    let rtol = if tight { 1e-12 } else { DEFAULT_RTOL };
    let atol = if tight { 1e-14 } else { 1e-12 };
    let mut stepper = Dop853::from_param(
        Kerr8 { params },
        0.0,
        lam_end,
        0.001,
        y0,
        rtol,
        atol,
        0.9,
        0.0,
        0.333,
        6.0,
        lam_end,
        0.0,
        100_000,
        1000,
        ode_solvers::dop_shared::OutputType::Dense,
    );
    let stats = stepper.integrate();
    let yf = stepper.y_out().last().cloned();
    let hf = yf.as_ref().and_then(|y| {
        evaluate_hamiltonian(
            &KerrParams::new(mass, spin).unwrap(),
            &relativity_core::PositionKs::new(y[0], y[1], y[2], y[3]),
            &relativity_core::Covector::from_components([y[4], y[5], y[6], y[7]]),
        )
        .ok()
    });
    let h_drift = hf.map(|h| (h.h - h0.h).abs()).unwrap_or(f64::INFINITY);
    signature_join(&[
        &h_drift.to_bits().to_string(),
        &stats
            .as_ref()
            .map(|s| s.accepted_steps.to_string())
            .unwrap_or_default(),
        &yf.as_ref()
            .map(|y| endpoint_bits(y.as_slice()))
            .unwrap_or_default(),
    ])
}

pub fn run_g(tight: bool) -> ExperimentResult {
    use relativity_core::{
        evaluate_hamiltonian, initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams,
        PositionBl, SensorCoord,
    };
    let mass = 1.0;
    let spin = 0.9;
    let params = KerrParams::new(mass, spin).expect("params");
    let bl = PositionBl::new(0.0, 500.0, std::f64::consts::FRAC_PI_2, 0.0);
    let obs = zamo_observer(&params, &bl).expect("zamo");
    let cam = CameraParams {
        horizontal_fov: 60.0_f64.to_radians(),
        roll: 0.0,
    };
    let ray = initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 })
        .expect("ray");
    let pos = obs.event;
    let p = ray.covariant_momentum;
    let h0 = evaluate_hamiltonian(&params, &pos, &p).expect("H0");

    struct Kerr8 {
        params: KerrParams,
    }
    impl System<f64, DVector<f64>> for Kerr8 {
        fn system(&self, _lam: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
            let pos = relativity_core::PositionKs::new(y[0], y[1], y[2], y[3]);
            let pc = relativity_core::Covector::from_components([y[4], y[5], y[6], y[7]]);
            if let Ok(ev) = evaluate_hamiltonian(&self.params, &pos, &pc) {
                dy[0] = ev.dx_dlambda.t;
                dy[1] = ev.dx_dlambda.x;
                dy[2] = ev.dx_dlambda.y;
                dy[3] = ev.dx_dlambda.z;
                dy[4] = ev.dp_dlambda.t;
                dy[5] = ev.dp_dlambda.x;
                dy[6] = ev.dp_dlambda.y;
                dy[7] = ev.dp_dlambda.z;
            } else {
                dy.fill(f64::NAN);
            }
        }
    }
    let y0 = DVector::from_vec(vec![pos.t, pos.x, pos.y, pos.z, p.t, p.x, p.y, p.z]);
    let lam_end = 0.01;
    let rtol = if tight { 1e-12 } else { DEFAULT_RTOL };
    let atol = if tight { 1e-14 } else { 1e-12 };
    let mut stepper = Dop853::from_param(
        Kerr8 { params },
        0.0,
        lam_end,
        0.001,
        y0,
        rtol,
        atol,
        0.9,
        0.0,
        0.333,
        6.0,
        lam_end,
        0.0,
        100_000,
        1000,
        ode_solvers::dop_shared::OutputType::Dense,
    );
    let stats = stepper.integrate();
    let yf = stepper.y_out().last().cloned();
    let hf = yf.as_ref().and_then(|y| {
        evaluate_hamiltonian(
            &params,
            &relativity_core::PositionKs::new(y[0], y[1], y[2], y[3]),
            &relativity_core::Covector::from_components([y[4], y[5], y[6], y[7]]),
        )
        .ok()
    });
    let h_drift = hf.map(|h| (h.h - h0.h).abs()).unwrap_or(f64::INFINITY);
    let e_drift = hf
        .map(|h| (h.energy_like - h0.energy_like).abs())
        .unwrap_or(f64::INFINITY);
    let finite = yf.as_ref().is_some_and(|y| y.iter().all(|v| v.is_finite()));
    let det = kerr_determinism(tight);
    ExperimentResult {
        id: ExperimentId::G,
        passed: stats.is_ok() && finite && h_drift < 1e-6 && det.deterministic,
        detail: format!(
            "tight={tight} H_drift={h_drift:.3e} E_drift={e_drift:.3e} nacc={} finite={finite}",
            stats.as_ref().map(|s| s.accepted_steps).unwrap_or(0)
        ),
        endpoint_abs_error: Some(h_drift),
        endpoint_rel_error: Some(e_drift),
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: stats.ok().map(|s| stats_to_integration(s, 0.0, 0.0)),
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: None,
        root_localization: None,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}
