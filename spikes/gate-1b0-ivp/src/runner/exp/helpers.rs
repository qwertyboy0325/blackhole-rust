//! Per-experiment implementations for `ivp`.

use crate::adapter::{
    component_errors, dense_assessment_ivp, dop853_with_max_step, endpoint_errors,
    error_scaling_ivp, solve_dop853, solve_dop853_solout, stats_from_result, CaptureLog,
    CapturingSolOut, DenseProbeSolOut, DomainLatch, DomainSys, DEFAULT_ATOL, DEFAULT_RTOL,
    DOMAIN_ERROR_CODE,
};
use crate::event_loop::{interrupted_ok, run_shallow_event_localize, run_sho_event_stop};
use gate_1b0_contract::event::EventFn;
use gate_1b0_contract::{
    endpoint_bits, exp_analytic, localize_root, mixed8_analytic, mixed8_derivative, mixed8_y0,
    repeat_in_process, repeat_in_process_sig, shallow_event_fn, sho_analytic_energy,
    sho_analytic_p, sho_analytic_q, sho_energy, signature_join, EXP_LAMBDA, EXP_X0, EXP_X_END,
    EXP_Y0, MIXED8_DIM, SHO_EVENT_X, SHO_P0, SHO_Q0, SHO_X0, SHO_X_END,
};
use gate_1b0_contract::{
    AcceptedStepProbe, CallbackStopEvidence, DenseProbe, DeterminismRecord, DomainErrorEvidence,
    ExperimentId, ExperimentResult, RepeatSummary, RestartEvidence, RootLocalizationEvidence,
    SolverStopEvidence, StepGuardAssessment, SupportLevel,
};
use ivp::methods::Tolerance;
use ivp::prelude::FirstOrderSystem;
use ivp::solve::{Ivp, Method};
use ivp::status::Status;
use std::cell::RefCell;
use std::rc::Rc;

struct ExpSys {
    lambda: f64,
}

impl FirstOrderSystem for ExpSys {
    fn derivative(&self, _x: f64, y: &[f64], dy: &mut [f64]) {
        dy[0] = self.lambda * y[0];
    }
}

struct ShoSys;

impl FirstOrderSystem for ShoSys {
    fn derivative(&self, _x: f64, y: &[f64], dy: &mut [f64]) {
        dy[0] = y[1];
        dy[1] = -y[0];
    }
}

struct Mixed8Sys;

impl FirstOrderSystem for Mixed8Sys {
    fn derivative(&self, _x: f64, y: &[f64], dy: &mut [f64]) {
        let mut buf = [0.0; MIXED8_DIM];
        mixed8_derivative(_x, y, &mut buf);
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

fn solution_stats(sol: &ivp::solve::Solution) -> gate_1b0_contract::IntegrationStats {
    gate_1b0_contract::IntegrationStats {
        accepted_steps: sol.naccpt as u32,
        rejected_steps: sol.nrejct as u32,
        rhs_evaluations: sol.nfev as u32,
        final_step_size: sol.t.last().copied().unwrap_or(0.0),
        min_step_size: 0.0,
    }
}

fn exp_a_determinism() -> RepeatSummary {
    repeat_in_process(5, || {
        let (res, y_end) = solve_dop853(
            &ExpSys { lambda: EXP_LAMBDA },
            EXP_X0,
            &[EXP_Y0],
            EXP_X_END,
            DEFAULT_RTOL,
            DEFAULT_ATOL,
            None,
        )
        .expect("exp A det");
        (
            vec![y_end[0]],
            res.steps.accepted as u32,
            endpoint_bits(&[y_end[0]]),
        )
    })
}

pub fn run_a() -> ExperimentResult {
    let sol = Ivp::first_order(&ExpSys { lambda: EXP_LAMBDA }, EXP_X0, EXP_X_END, &[EXP_Y0])
        .method(Method::DOP853)
        .rtol(DEFAULT_RTOL)
        .atol(DEFAULT_ATOL)
        .dense_output(true)
        .solve()
        .expect("exp A integrate");
    let y_end = sol.y.last().map(|y| y[0]).unwrap_or(f64::NAN);
    let analytic = exp_analytic(EXP_X_END);
    let (abs, rel) = endpoint_errors(y_end, analytic);
    let mut dense_probes = Vec::new();
    for &theta in &[0.25, 0.5, 0.75] {
        let t = EXP_X0 + theta * (EXP_X_END - EXP_X0);
        if let Ok(yq) = sol.sol(t) {
            let ya = exp_analytic(t);
            let dabs = (yq[0] - ya).abs();
            let scale = ya.abs().max(yq[0].abs()).max(1e-12);
            dense_probes.push(DenseProbe {
                theta,
                t,
                abs_error: dabs,
                rel_error: dabs / scale,
            });
        }
    }
    let det = exp_a_determinism();
    ExperimentResult {
        id: ExperimentId::A,
        passed: abs < 1e-6 && rel < 1e-6 && det.deterministic,
        detail: format!("endpoint abs={abs:.3e} rel={rel:.3e} nacc={}", sol.naccpt),
        endpoint_abs_error: Some(abs),
        endpoint_rel_error: Some(rel),
        component_errors: vec![],
        dense_probes,
        accepted_step_probes: vec![],
        stats: Some(solution_stats(&sol)),
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
        let (res, y_end) = solve_dop853(
            &ShoSys,
            SHO_X0,
            &[SHO_Q0, SHO_P0],
            SHO_X_END,
            DEFAULT_RTOL,
            DEFAULT_ATOL,
            None,
        )
        .expect("sho det");
        let endpoint = y_end;
        (
            endpoint.clone(),
            res.steps.accepted as u32,
            endpoint_bits(&endpoint),
        )
    })
}

pub fn run_b() -> ExperimentResult {
    let log = CaptureLog::new(SHO_X0, vec![SHO_Q0, SHO_P0]);
    let mut solout = CapturingSolOut { log: log.clone() };
    let res = solve_dop853_solout(
        &ShoSys,
        SHO_X0,
        &[SHO_Q0, SHO_P0],
        SHO_X_END,
        DEFAULT_RTOL,
        DEFAULT_ATOL,
        &mut solout,
        None,
    )
    .expect("sho integrate");
    let y_end = log.last_y.borrow().clone();
    let q_end = y_end[0];
    let p_end = y_end[1];
    let qa = sho_analytic_q(SHO_X_END);
    let pa = sho_analytic_p(SHO_X_END);
    let endpoint_abs = ((q_end - qa).powi(2) + (p_end - pa).powi(2)).sqrt();
    let energy_drift = (sho_energy(q_end, p_end) - sho_analytic_energy()).abs();

    let sol = Ivp::first_order(&ShoSys, SHO_X0, SHO_X_END, &[SHO_Q0, SHO_P0])
        .method(Method::DOP853)
        .rtol(DEFAULT_RTOL)
        .atol(DEFAULT_ATOL)
        .dense_output(true)
        .solve()
        .expect("sho dense query");
    let mut dense_probes = Vec::new();
    for &theta in &[0.3, 0.6] {
        let t = SHO_X0 + theta * (SHO_X_END - SHO_X0);
        if let Ok(yq) = sol.sol(t) {
            let ya = sho_analytic_q(t);
            let dabs = (yq[0] - ya).abs();
            dense_probes.push(DenseProbe {
                theta,
                t,
                abs_error: dabs,
                rel_error: dabs / ya.abs().max(1e-12),
            });
        }
    }

    let event: &EventFn = &|_t: f64, y: &[f64]| y[0];
    let root_localization = log.steps.borrow().iter().find_map(|s| {
        let f0 = event(s.x0, &s.y0);
        let f1 = event(s.x1, &s.y1);
        if f0.signum() != f1.signum() {
            Some(localize_root(
                s.x0,
                s.x1,
                &s.y0,
                &s.y1,
                event,
                None,
                SHO_EVENT_X,
                &[0.0, sho_analytic_p(SHO_EVENT_X)],
                false,
            ))
        } else {
            None
        }
    });
    let det = sho_determinism();
    ExperimentResult {
        id: ExperimentId::B,
        passed: endpoint_abs < 1e-4 && energy_drift < 1e-3 && det.deterministic,
        detail: format!(
            "endpoint={endpoint_abs:.3e} energy_drift={energy_drift:.3e} root_localized={}",
            root_localization.is_some()
        ),
        endpoint_abs_error: Some(endpoint_abs),
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes,
        accepted_step_probes: vec![],
        stats: Some(stats_from_result(&res)),
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
        let y0 = mixed8_y0();
        let (res, y_end) =
            solve_dop853(&Mixed8Sys, 0.0, &y0, 0.5, DEFAULT_RTOL, DEFAULT_ATOL, None)
                .expect("mixed8 det");
        (
            y_end.clone(),
            res.steps.accepted as u32,
            endpoint_bits(&y_end),
        )
    })
}

pub fn run_c() -> ExperimentResult {
    let y0 = mixed8_y0();
    let x_end = 0.5;
    let (res, y_end) = solve_dop853(
        &Mixed8Sys,
        0.0,
        &y0,
        x_end,
        DEFAULT_RTOL,
        DEFAULT_ATOL,
        None,
    )
    .expect("mixed8 scalar tol");
    let analytic = mixed8_analytic(x_end);
    let comps = component_errors(&y_end, &analytic);
    let max_abs = comps.iter().map(|c| c.abs).fold(0.0_f64, f64::max);
    let det = mixed8_determinism();
    ExperimentResult {
        id: ExperimentId::C,
        passed: comps.iter().all(|c| c.abs < 1e-5) && det.deterministic,
        detail: format!("scalar_tol max_abs={max_abs:.3e}"),
        endpoint_abs_error: Some(max_abs),
        endpoint_rel_error: None,
        component_errors: comps,
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: Some(stats_from_result(&res)),
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: None,
        root_localization: None,
        solver_stop: None,
        restart: None,
        callback_stop: None,
        domain_error: None,
        error_scaling: Some(error_scaling_ivp()),
    }
}

pub fn run_c_vector() -> ExperimentResult {
    let y0 = mixed8_y0();
    let x_end = 0.5;
    let rtol = Tolerance::Vector(vec![DEFAULT_RTOL; MIXED8_DIM]);
    let atol = Tolerance::Vector(
        y0.iter()
            .map(|v| v.abs().max(1e-6) * DEFAULT_ATOL)
            .collect(),
    );
    let sol = Ivp::first_order(&Mixed8Sys, 0.0, x_end, &y0)
        .method(Method::DOP853)
        .rtol(rtol)
        .atol(atol)
        .dense_output(true)
        .solve()
        .expect("mixed8 vector tol");
    let y_end = sol.y.last().cloned().unwrap_or_default();
    let analytic = mixed8_analytic(x_end);
    let comps = component_errors(&y_end, &analytic);
    let max_abs = comps.iter().map(|c| c.abs).fold(0.0_f64, f64::max);
    let det = repeat_in_process_sig(5, || {
        let sol = Ivp::first_order(&Mixed8Sys, 0.0, x_end, &y0)
            .method(Method::DOP853)
            .rtol(Tolerance::Vector(vec![DEFAULT_RTOL; MIXED8_DIM]))
            .atol(Tolerance::Vector(
                y0.iter()
                    .map(|v| v.abs().max(1e-6) * DEFAULT_ATOL)
                    .collect(),
            ))
            .dense_output(true)
            .solve()
            .expect("mixed8 vector det");
        let ep = sol.y.last().cloned().unwrap_or_default();
        let bits = endpoint_bits(&ep);
        (bits.clone(), sol.naccpt as u32, bits)
    });
    ExperimentResult {
        id: ExperimentId::C,
        passed: comps.iter().all(|c| c.abs < 1e-5) && det.deterministic,
        detail: format!("vector_tol max_abs={max_abs:.3e}"),
        endpoint_abs_error: Some(max_abs),
        endpoint_rel_error: None,
        component_errors: comps,
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: Some(solution_stats(&sol)),
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

fn dense_d_determinism() -> RepeatSummary {
    repeat_in_process_sig(5, || {
        let probes = Rc::new(RefCell::new(Vec::<AcceptedStepProbe>::new()));
        let log = CaptureLog::new(EXP_X0, vec![EXP_Y0]);
        let mut solout = DenseProbeSolOut {
            log: log.clone(),
            analytic: |t| vec![exp_analytic(t)],
            probes: probes.clone(),
        };
        let res = solve_dop853_solout(
            &ExpSys { lambda: EXP_LAMBDA },
            EXP_X0,
            &[EXP_Y0],
            EXP_X_END,
            DEFAULT_RTOL,
            DEFAULT_ATOL,
            &mut solout,
            None,
        )
        .expect("dense D det");
        let max_err = probes
            .borrow()
            .iter()
            .map(|p| p.max_abs_error)
            .fold(0.0_f64, f64::max);
        let sig = signature_join(&[
            &probes.borrow().len().to_string(),
            &max_err.to_bits().to_string(),
            &res.steps.accepted.to_string(),
        ]);
        (sig.clone(), res.steps.accepted as u32, sig)
    })
}

pub fn run_d() -> ExperimentResult {
    let probes = Rc::new(RefCell::new(Vec::<AcceptedStepProbe>::new()));
    let log = CaptureLog::new(EXP_X0, vec![EXP_Y0]);
    let mut solout = DenseProbeSolOut {
        log: log.clone(),
        analytic: |t| vec![exp_analytic(t)],
        probes: probes.clone(),
    };
    let res = solve_dop853_solout(
        &ExpSys { lambda: EXP_LAMBDA },
        EXP_X0,
        &[EXP_Y0],
        EXP_X_END,
        DEFAULT_RTOL,
        DEFAULT_ATOL,
        &mut solout,
        None,
    )
    .expect("dense D");
    let accepted_step_probes = probes.borrow().clone();
    let max_probe_err = accepted_step_probes
        .iter()
        .map(|p| p.max_abs_error)
        .fold(0.0_f64, f64::max);
    let had_interpolant = log.steps.borrow().iter().any(|s| s.had_interpolant);
    let assessment = dense_assessment_ivp(had_interpolant);
    let det = dense_d_determinism();
    let passed = !accepted_step_probes.is_empty()
        && accepted_step_probes.iter().all(|p| p.max_abs_error < 1e-6)
        && had_interpolant
        && det.deterministic;
    ExperimentResult {
        id: ExperimentId::D,
        passed,
        detail: format!(
            "probes={} max_err={max_probe_err:.3e} interp_steps={}",
            accepted_step_probes.len(),
            log.steps.borrow().len()
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes,
        stats: Some(stats_from_result(&res)),
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

fn sho_reference_endpoint() -> Vec<f64> {
    let (_, y) = solve_dop853(
        &ShoSys,
        0.0,
        &[1.0, 0.0],
        SHO_X_END,
        DEFAULT_RTOL,
        DEFAULT_ATOL,
        None,
    )
    .expect("sho reference");
    y
}

fn e_stop_restart_once() -> (
    RootLocalizationEvidence,
    SolverStopEvidence,
    RestartEvidence,
    u32,
) {
    let x_end = SHO_EVENT_X + 1.0;
    let (res, cap) =
        run_sho_event_stop(&ShoSys, 0.0, &[1.0, 0.0], x_end, DEFAULT_RTOL, DEFAULT_ATOL)
            .expect("event stop");
    let root = cap.root.borrow().clone().expect("root localized");
    let mut solver_stop = cap.stop.borrow().clone().expect("stop evidence");
    cap.fill_stop_stats(&res);
    if let Some(stop) = cap.stop.borrow().clone() {
        solver_stop = stop;
    }
    solver_stop.interrupted = interrupted_ok(&res);

    let reference_endpoint = sho_reference_endpoint();
    let (_, restart_endpoint) = solve_dop853(
        &ShoSys,
        root.event_time_found,
        &root.localized_state,
        SHO_X_END,
        DEFAULT_RTOL,
        DEFAULT_ATOL,
        None,
    )
    .expect("restart");
    let endpoint_error = restart_endpoint
        .iter()
        .zip(reference_endpoint.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);

    let restart_det = repeat_in_process_sig(5, || {
        let (_, ep) = solve_dop853(
            &ShoSys,
            root.event_time_found,
            &root.localized_state,
            SHO_X_END,
            DEFAULT_RTOL,
            DEFAULT_ATOL,
            None,
        )
        .expect("restart det");
        let bits = endpoint_bits(&ep);
        (bits.clone(), 0, bits)
    });

    let restart = RestartEvidence {
        restart_time: root.event_time_found,
        restart_state: root.localized_state.clone(),
        restart_endpoint: restart_endpoint.clone(),
        reference_endpoint: reference_endpoint.clone(),
        endpoint_error,
        deterministic: restart_det.deterministic,
        endpoint_bits: vec![endpoint_bits(&restart_endpoint)],
        in_process_runs: restart_det.signatures.len() as u32,
    };

    (root, solver_stop, restart, res.steps.accepted as u32)
}

pub fn run_e() -> ExperimentResult {
    let (root, solver_stop, restart, accepted) = e_stop_restart_once();
    let det = repeat_in_process_sig(5, || {
        let (root, _stop, restart, acc) = e_stop_restart_once();
        let sig = signature_join(&[
            &root.event_time_found.to_bits().to_string(),
            &restart.endpoint_bits.first().cloned().unwrap_or_default(),
        ]);
        (sig.clone(), acc, sig)
    });
    let passed = root.time_error < 1e-6
        && solver_stop.interrupted
        && restart.deterministic
        && restart.endpoint_error < 1e-4;
    ExperimentResult {
        id: ExperimentId::E,
        passed,
        detail: format!(
            "time_err={:.3e} interrupted={} restart_det={} endpoint_err={:.3e} nacc={accepted}",
            root.time_error, solver_stop.interrupted, restart.deterministic, restart.endpoint_error
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: None,
        determinism: Some(det_record(det)),
        dense_assessment: None,
        step_guard: None,
        root_localization: Some(root),
        solver_stop: Some(solver_stop),
        restart: Some(restart),
        callback_stop: None,
        domain_error: None,
        error_scaling: None,
    }
}

pub fn run_e_shallow() -> ExperimentResult {
    let root_localization =
        run_shallow_event_localize(0.0, &[0.99, 0.0], 0.5, DEFAULT_RTOL, DEFAULT_ATOL)
            .expect("shallow event")
            .map(|mut root| {
                root.shallow_crossing_tested = true;
                root.shallow_sign_change_only_insufficient =
                    shallow_event_fn(root.event_time_found).abs() > 1e-9;
                root
            });
    ExperimentResult {
        id: ExperimentId::E,
        passed: root_localization.is_some(),
        detail: "shallow crossing accepted-step localization".into(),
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
    let log = CaptureLog::new(EXP_X0, vec![EXP_Y0]);
    *log.stop_next.borrow_mut() = true;
    let mut solout = CapturingSolOut { log: log.clone() };
    let res = solve_dop853_solout(
        &ExpSys { lambda: EXP_LAMBDA },
        EXP_X0,
        &[EXP_Y0],
        EXP_X_END,
        DEFAULT_RTOL,
        DEFAULT_ATOL,
        &mut solout,
        None,
    )
    .expect("callback stop");
    let stop_time = *log.last_x.borrow();
    let stop_state = log.last_y.borrow().clone();
    let accepted_before =
        res.steps
            .accepted
            .saturating_sub(*log.accepted_after_stop.borrow() as usize) as u32;
    let det = repeat_in_process_sig(5, || {
        let inner_log = CaptureLog::new(EXP_X0, vec![EXP_Y0]);
        *inner_log.stop_next.borrow_mut() = true;
        let mut inner = CapturingSolOut {
            log: inner_log.clone(),
        };
        let st = solve_dop853_solout(
            &ExpSys { lambda: EXP_LAMBDA },
            EXP_X0,
            &[EXP_Y0],
            EXP_X_END,
            DEFAULT_RTOL,
            DEFAULT_ATOL,
            &mut inner,
            None,
        )
        .expect("callback stop det");
        let xt = *inner_log.last_x.borrow();
        (
            signature_join(&[&xt.to_bits().to_string(), &st.steps.accepted.to_string()]),
            st.steps.accepted as u32,
            xt.to_bits().to_string(),
        )
    });
    let interrupted = matches!(res.status, Status::UserInterrupt);
    let evidence = CallbackStopEvidence {
        callback_invoked: *log.callback_count.borrow() > 0,
        interrupt_requested: *log.stop_requested.borrow(),
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

fn domain_error_evidence() -> (DomainErrorEvidence, bool) {
    let latch = DomainLatch::new();
    let sys = DomainSys {
        latch: latch.clone(),
    };
    let solver = dop853_with_max_step(0.2);
    let result = solver.solve(
        &sys,
        0.0,
        &[1.0],
        2.0,
        Tolerance::Scalar(DEFAULT_RTOL),
        Tolerance::Scalar(DEFAULT_ATOL),
        None::<&mut CapturingSolOut>,
    );
    let code = latch.0.borrow().clone().unwrap_or_default();
    let nan_presented_as_error = result.is_err();
    let evidence = DomainErrorEvidence {
        typed_error_code: code.clone(),
        typed_error_recovered: false,
        solver_panicked: false,
        nan_presented_as_error,
    };
    let ok = code == DOMAIN_ERROR_CODE && !nan_presented_as_error && !evidence.solver_panicked;
    (evidence, ok)
}

pub fn run_f() -> ExperimentResult {
    let h_max = 0.05;
    let (callback_stop, cb_ok) = callback_stop_evidence();
    let (domain_error, domain_ok) = domain_error_evidence();
    let det = repeat_in_process_sig(5, || {
        let inner_log = CaptureLog::new(EXP_X0, vec![EXP_Y0]);
        *inner_log.stop_next.borrow_mut() = true;
        let mut inner = CapturingSolOut {
            log: inner_log.clone(),
        };
        let st = solve_dop853_solout(
            &ExpSys { lambda: EXP_LAMBDA },
            EXP_X0,
            &[EXP_Y0],
            EXP_X_END,
            DEFAULT_RTOL,
            DEFAULT_ATOL,
            &mut inner,
            Some(h_max),
        )
        .expect("F det callback");
        let latch = DomainLatch::new();
        let dom_sys = DomainSys {
            latch: latch.clone(),
        };
        let dom_solver = dop853_with_max_step(h_max);
        let dom = dom_solver
            .solve(
                &dom_sys,
                0.0,
                &[1.0],
                2.0,
                Tolerance::Scalar(DEFAULT_RTOL),
                Tolerance::Scalar(DEFAULT_ATOL),
                None::<&mut CapturingSolOut>,
            )
            .is_ok();
        let sig = signature_join(&[
            &st.steps.accepted.to_string(),
            &dom.to_string(),
            &latch.0.borrow().clone().unwrap_or_default(),
        ]);
        (sig.clone(), st.steps.accepted as u32, sig)
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
            SupportLevel::SupportedWithAdapter
        },
        notes:
            "SolOut Interrupt halts after accepted step; DomainLatch typed code on x>=DOMAIN_X_MAX"
                .into(),
    };
    ExperimentResult {
        id: ExperimentId::F,
        passed: cb_ok && domain_ok && det.deterministic,
        detail: format!(
            "callback_stop interrupted={} domain_code={} h_max={h_max}",
            callback_stop.interrupted, domain_error.typed_error_code
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
    impl FirstOrderSystem for Kerr8 {
        fn derivative(&self, _lam: f64, y: &[f64], dy: &mut [f64]) {
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
    let y0 = vec![pos.t, pos.x, pos.y, pos.z, p.t, p.x, p.y, p.z];
    let lam_end = 0.01;
    let rtol = if tight { 1e-12 } else { DEFAULT_RTOL };
    let atol = if tight { 1e-14 } else { DEFAULT_ATOL };
    let (res, yf) =
        solve_dop853(&Kerr8 { params }, 0.0, &y0, lam_end, rtol, atol, None).expect("kerr sig");
    let hf = evaluate_hamiltonian(
        &KerrParams::new(mass, spin).unwrap(),
        &relativity_core::PositionKs::new(yf[0], yf[1], yf[2], yf[3]),
        &relativity_core::Covector::from_components([yf[4], yf[5], yf[6], yf[7]]),
    )
    .ok();
    let h_drift = hf.map(|h| (h.h - h0.h).abs()).unwrap_or(f64::INFINITY);
    signature_join(&[
        &h_drift.to_bits().to_string(),
        &res.steps.accepted.to_string(),
        &endpoint_bits(&yf),
    ])
}

fn kerr_determinism(tight: bool) -> RepeatSummary {
    repeat_in_process_sig(5, || {
        let sig = kerr_signature(tight);
        (sig.clone(), 0, sig)
    })
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
    impl FirstOrderSystem for Kerr8 {
        fn derivative(&self, _lam: f64, y: &[f64], dy: &mut [f64]) {
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
    let y0 = vec![pos.t, pos.x, pos.y, pos.z, p.t, p.x, p.y, p.z];
    let lam_end = 0.01;
    let rtol = if tight { 1e-12 } else { DEFAULT_RTOL };
    let atol = if tight { 1e-14 } else { DEFAULT_ATOL };
    let (res, yf) = solve_dop853(&Kerr8 { params }, 0.0, &y0, lam_end, rtol, atol, None)
        .expect("kerr integrate");
    let hf = evaluate_hamiltonian(
        &params,
        &relativity_core::PositionKs::new(yf[0], yf[1], yf[2], yf[3]),
        &relativity_core::Covector::from_components([yf[4], yf[5], yf[6], yf[7]]),
    )
    .ok();
    let h_drift = hf.map(|h| (h.h - h0.h).abs()).unwrap_or(f64::INFINITY);
    let e_drift = hf
        .map(|h| (h.energy_like - h0.energy_like).abs())
        .unwrap_or(f64::INFINITY);
    let finite = yf.iter().all(|v| v.is_finite());
    let det = kerr_determinism(tight);
    ExperimentResult {
        id: ExperimentId::G,
        passed: res.status.is_success() && finite && h_drift < 1e-6 && det.deterministic,
        detail: format!(
            "tight={tight} H_drift={h_drift:.3e} E_drift={e_drift:.3e} nacc={} finite={finite}",
            res.steps.accepted
        ),
        endpoint_abs_error: Some(h_drift),
        endpoint_rel_error: Some(e_drift),
        component_errors: vec![],
        dense_probes: vec![],
        accepted_step_probes: vec![],
        stats: Some(stats_from_result(&res)),
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
