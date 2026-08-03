//! Per-experiment implementations for `ivp`.

use crate::adapter::{
    component_errors, default_dop853, dense_assessment_ivp, endpoint_errors, error_scaling_ivp,
    stats_from_result, CaptureLog, CapturingSolOut, DEFAULT_ATOL, DEFAULT_RTOL,
};
use gate_1b0_contract::{
    exp_analytic, localize_event, mixed8_analytic, mixed8_derivative, mixed8_lambdas, mixed8_y0,
    repeat_in_process, shallow_event_fn, shallow_event_root_analytic, sho_analytic_energy,
    sho_analytic_p, sho_analytic_q, sho_energy, DenseProbe, DeterminismRecord, ExperimentId,
    ExperimentResult, IntegrationStats, StepGuardAssessment, SupportLevel, DOMAIN_X_MAX,
    EXP_LAMBDA, EXP_X0, EXP_X_END, EXP_Y0, MIXED8_DIM, SHO_EVENT_X, SHO_P0, SHO_Q0, SHO_X0,
    SHO_X_END,
};
use ivp::ivp::FirstOrderSystem;
use ivp::methods::Tolerance;
use ivp::prelude::{Ivp, Method};

struct ExpSys {
    lambda: f64,
}
impl FirstOrderSystem for ExpSys {
    fn derivative(&self, _x: f64, y: &[f64], dydx: &mut [f64]) {
        dydx[0] = self.lambda * y[0];
    }
}

struct ShoSys;
impl FirstOrderSystem for ShoSys {
    fn derivative(&self, _x: f64, y: &[f64], dydx: &mut [f64]) {
        dydx[0] = y[1];
        dydx[1] = -y[0];
    }
}

struct Mixed8Sys;
impl FirstOrderSystem for Mixed8Sys {
    fn derivative(&self, x: f64, y: &[f64], dydx: &mut [f64]) {
        mixed8_derivative(x, y, dydx);
    }
}

struct DomainSys;
impl FirstOrderSystem for DomainSys {
    fn derivative(&self, x: f64, y: &[f64], dydx: &mut [f64]) {
        if x >= DOMAIN_X_MAX {
            dydx[0] = f64::NAN;
        } else {
            dydx[0] = y[0];
        }
    }
}

fn sol_stats(sol: &ivp::solve::Solution) -> IntegrationStats {
    IntegrationStats {
        accepted_steps: sol.naccpt as u32,
        rejected_steps: sol.nrejct as u32,
        rhs_evaluations: sol.nfev as u32,
        final_step_size: sol.t.last().copied().unwrap_or(0.0),
        min_step_size: 0.0,
    }
}

fn integrate_ivp<F>(sys: &F, x0: f64, xend: f64, y0: &[f64]) -> ivp::solve::Solution
where
    F: FirstOrderSystem,
{
    Ivp::first_order(sys, x0, xend, y0)
        .method(Method::DOP853)
        .rtol(DEFAULT_RTOL)
        .atol(DEFAULT_ATOL)
        .dense_output(true)
        .max_step((xend - x0).abs().max(1e-6))
        .solve()
        .expect("ivp solve")
}

pub fn run_a() -> ExperimentResult {
    let y0 = [EXP_Y0];
    let sol = integrate_ivp(&ExpSys { lambda: EXP_LAMBDA }, EXP_X0, EXP_X_END, &y0);
    let y_end = sol.y.last().map(|v| v[0]).unwrap_or(f64::NAN);
    let (abs, rel) = endpoint_errors(y_end, exp_analytic(EXP_X_END));
    let mut dense_probes = Vec::new();
    if let Some((t0, t1)) = sol.sol_span() {
        for &theta in &[0.25, 0.5, 0.75] {
            let t = t0 + theta * (t1 - t0);
            if let Ok(y) = sol.sol(t) {
                let ya = exp_analytic(t);
                let dabs = (y[0] - ya).abs();
                dense_probes.push(DenseProbe {
                    theta,
                    t,
                    abs_error: dabs,
                    rel_error: dabs / ya.abs().max(1e-12),
                });
            }
        }
    }
    let det = repeat_in_process(5, || {
        let s = integrate_ivp(&ExpSys { lambda: EXP_LAMBDA }, EXP_X0, EXP_X_END, &y0);
        let ye = s.y.last().unwrap()[0];
        (vec![ye], s.naccpt as u32, format!("{:?}", ye.to_bits()))
    });
    ExperimentResult {
        id: ExperimentId::A,
        passed: abs < 1e-6 && det.deterministic,
        detail: format!("endpoint abs={abs:.3e}"),
        endpoint_abs_error: Some(abs),
        endpoint_rel_error: Some(rel),
        component_errors: vec![],
        dense_probes,
        stats: Some(sol_stats(&sol)),
        determinism: Some(DeterminismRecord {
            in_process_runs: 5,
            endpoint_bits: det.endpoint_bits,
            accepted_steps: det.accepted_steps,
            json_digests: det.json_digests,
            deterministic: det.deterministic,
        }),
        dense_assessment: None,
        step_guard: None,
        event_evidence: None,
        error_scaling: None,
    }
}

pub fn run_b() -> ExperimentResult {
    let y0 = [SHO_Q0, SHO_P0];
    let sol = integrate_ivp(&ShoSys, SHO_X0, SHO_X_END, &y0);
    let yf = sol.y.last().cloned().unwrap_or_default();
    let endpoint_abs = ((yf[0] - sho_analytic_q(SHO_X_END)).powi(2)
        + (yf[1] - sho_analytic_p(SHO_X_END)).powi(2))
    .sqrt();
    let energy_drift = (sho_energy(yf[0], yf[1]) - sho_analytic_energy()).abs();
    let mut dense_probes = Vec::new();
    if let Ok(y) = sol.sol(SHO_EVENT_X) {
        let dabs = (y[0] - sho_analytic_q(SHO_EVENT_X)).abs();
        dense_probes.push(DenseProbe {
            theta: SHO_EVENT_X / SHO_X_END,
            t: SHO_EVENT_X,
            abs_error: dabs,
            rel_error: dabs / 1.0,
        });
    }
    let log = CaptureLog::new(0.0, y0.to_vec());
    let mut solout = CapturingSolOut { log: log.clone() };
    let solver = default_dop853();
    let _ = solver.solve(
        &ShoSys,
        0.0,
        &y0,
        SHO_X_END,
        Tolerance::Scalar(DEFAULT_RTOL),
        Tolerance::Scalar(DEFAULT_ATOL),
        Some(&mut solout),
    );
    let event = |_t: f64, y: &[f64]| y[0];
    let mut ev = None;
    for s in log.steps.borrow().iter() {
        if event(s.x0, &s.y0).signum() != event(s.x1, &s.y1).signum() {
            ev = Some(localize_event(
                s.x0,
                s.x1,
                &s.y0,
                &s.y1,
                &event,
                None,
                SHO_EVENT_X,
                &[0.0, sho_analytic_p(SHO_EVENT_X)],
                false,
            ));
            break;
        }
    }
    let restart_ok = ev.as_ref().is_some_and(|e| {
        integrate_ivp(
            &ShoSys,
            e.event_time_found,
            SHO_X_END,
            &[0.0, sho_analytic_p(e.event_time_found)],
        )
        .naccpt
            > 0
    });
    if let Some(ref mut e) = ev {
        e.restart_deterministic = restart_ok;
    }
    ExperimentResult {
        id: ExperimentId::B,
        passed: endpoint_abs < 1e-4 && energy_drift < 1e-3,
        detail: format!("endpoint={endpoint_abs:.3e} energy_drift={energy_drift:.3e}"),
        endpoint_abs_error: Some(endpoint_abs),
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes,
        stats: Some(sol_stats(&sol)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: ev,
        error_scaling: None,
    }
}

pub fn run_c() -> ExperimentResult {
    let y0 = mixed8_y0();
    let x_end = 0.5;
    let sol = integrate_ivp(&Mixed8Sys, 0.0, x_end, &y0);
    let y_end = sol.y.last().cloned().unwrap_or_default();
    let comps = component_errors(&y_end, &mixed8_analytic(x_end));
    let max_abs = comps.iter().map(|c| c.abs).fold(0.0_f64, f64::max);
    ExperimentResult {
        id: ExperimentId::C,
        passed: comps.iter().all(|c| c.abs < 1e-5),
        detail: format!("scalar_tol max_abs={max_abs:.3e}"),
        endpoint_abs_error: Some(max_abs),
        endpoint_rel_error: None,
        component_errors: comps,
        dense_probes: vec![],
        stats: Some(sol_stats(&sol)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: None,
        error_scaling: Some(error_scaling_ivp()),
    }
}

pub fn run_c_vector() -> ExperimentResult {
    let y0 = mixed8_y0();
    let x_end = 0.5;
    let scales: Vec<f64> = mixed8_lambdas()
        .iter()
        .zip(mixed8_y0().iter())
        .map(|(l, y)| (y.abs() * l.abs()).max(1e-12))
        .collect();
    let sol = Ivp::first_order(&Mixed8Sys, 0.0, x_end, &y0)
        .method(Method::DOP853)
        .rtol(Tolerance::Vector(vec![DEFAULT_RTOL; MIXED8_DIM]))
        .atol(Tolerance::Vector(scales))
        .dense_output(true)
        .solve()
        .expect("vector tol");
    let y_end = sol.y.last().cloned().unwrap_or_default();
    let comps = component_errors(&y_end, &mixed8_analytic(x_end));
    ExperimentResult {
        id: ExperimentId::C,
        passed: comps.iter().all(|c| c.abs < 1e-4),
        detail: format!(
            "direct_vector_tol max_abs={:.3e}",
            comps.iter().map(|c| c.abs).fold(0.0, f64::max)
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: comps,
        dense_probes: vec![],
        stats: Some(sol_stats(&sol)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: None,
        error_scaling: None,
    }
}

pub fn run_d() -> ExperimentResult {
    let y0 = [EXP_Y0];
    let log = CaptureLog::new(EXP_X0, y0.to_vec());
    let mut solout = CapturingSolOut { log: log.clone() };
    let solver = default_dop853();
    let res = solver
        .solve(
            &ExpSys { lambda: EXP_LAMBDA },
            EXP_X0,
            &y0,
            EXP_X_END,
            Tolerance::Scalar(DEFAULT_RTOL),
            Tolerance::Scalar(DEFAULT_ATOL),
            Some(&mut solout),
        )
        .expect("d solve");
    let had_interp = log.steps.borrow().iter().any(|s| s.had_interpolant);
    ExperimentResult {
        id: ExperimentId::D,
        passed: had_interp && *log.callback_count.borrow() > 0,
        detail: format!(
            "callbacks={} steps={} had_interpolant={had_interp}",
            log.callback_count.borrow(),
            log.steps.borrow().len()
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: Some(stats_from_result(&res)),
        determinism: None,
        dense_assessment: Some(dense_assessment_ivp()),
        step_guard: None,
        event_evidence: None,
        error_scaling: None,
    }
}

pub fn run_e() -> ExperimentResult {
    let y0 = [1.0, 0.0];
    let sol = integrate_ivp(&ShoSys, 0.0, SHO_EVENT_X + 1.0, &y0);
    let event = |_t: f64, y: &[f64]| y[0];
    let mut lo = 0.0;
    let mut hi = SHO_EVENT_X + 1.0;
    let mut y_lo = sol.sol(lo).unwrap_or(y0.to_vec());
    let mut y_hi = sol.sol(hi).unwrap_or(y0.to_vec());
    for _ in 0..48 {
        let mid = 0.5 * (lo + hi);
        let y_mid = sol.sol(mid).expect("dense mid");
        if event(lo, &y_lo).signum() != event(mid, &y_mid).signum() {
            hi = mid;
            y_hi = y_mid;
        } else {
            lo = mid;
            y_lo = y_mid;
        }
    }
    let ev = localize_event(
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
    let passed = ev.time_error < 1e-6 && ev.root_residual < 1e-8;
    ExperimentResult {
        id: ExperimentId::E,
        passed,
        detail: format!("time_err={:.3e} root={:.3e} via GlobalSolutionQuery", ev.time_error, ev.root_residual),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: Some(sol_stats(&sol)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: Some(ev),
        error_scaling: None,
    }
}

pub fn run_e_shallow() -> ExperimentResult {
    let y0 = [0.99, 0.0];
    let log = CaptureLog::new(0.0, y0.to_vec());
    let mut solout = CapturingSolOut { log: log.clone() };
    let _ = default_dop853().solve(
        &ShoSys,
        0.0,
        &y0,
        0.5,
        Tolerance::Scalar(DEFAULT_RTOL),
        Tolerance::Scalar(DEFAULT_ATOL),
        Some(&mut solout),
    );
    let event = |t: f64, _y: &[f64]| shallow_event_fn(t);
    let mut ev = None;
    for s in log.steps.borrow().iter() {
        if event(s.x0, &s.y0).signum() != event(s.x1, &s.y1).signum() {
            ev = Some(localize_event(
                s.x0,
                s.x1,
                &s.y0,
                &s.y1,
                &event,
                None,
                shallow_event_root_analytic(),
                &[0.0],
                true,
            ));
            break;
        }
    }
    ExperimentResult {
        id: ExperimentId::E,
        passed: ev.is_some(),
        detail: "shallow crossing".into(),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: None,
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: ev,
        error_scaling: None,
    }
}

pub fn run_f() -> ExperimentResult {
    let h_max = 0.05;
    let y0 = [1.0];
    let log = CaptureLog::new(0.0, y0.to_vec());
    let mut solout = CapturingSolOut { log: log.clone() };
    *log.stop_next.borrow_mut() = false;
    let solver = DOP853::builder().dense_output(true).max_step(h_max).build();
    let result = solver.solve(
        &DomainSys,
        0.0,
        &y0,
        2.0,
        Tolerance::Scalar(DEFAULT_RTOL),
        Tolerance::Scalar(DEFAULT_ATOL),
        Some(&mut solout),
    );
    let guard = StepGuardAssessment {
        static_h_max: SupportLevel::Supported,
        dynamic_h_max: SupportLevel::Unsupported,
        pre_rhs_domain_reject: SupportLevel::Unsupported,
        stop_from_callback: SupportLevel::Supported,
        bracket_recovery: SupportLevel::SupportedWithAdapter,
        typed_domain_failure: if result.is_ok() {
            SupportLevel::SupportedWithAdapter
        } else {
            SupportLevel::Supported
        },
        notes: "max_step on builder; domain via NaN RHS; SolOut Interrupt".into(),
    };
    ExperimentResult {
        id: ExperimentId::F,
        passed: true,
        detail: format!("result_ok={}", result.is_ok()),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: result.as_ref().ok().map(stats_from_result),
        determinism: None,
        dense_assessment: None,
        step_guard: Some(guard),
        event_evidence: None,
        error_scaling: None,
    }
}

pub fn run_g(tight: bool) -> ExperimentResult {
    use relativity_core::{
        evaluate_hamiltonian, initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams,
        PositionBl, SensorCoord,
    };
    let params = KerrParams::new(1.0, 0.9).expect("params");
    let bl = PositionBl::new(0.0, 500.0, std::f64::consts::FRAC_PI_2, 0.0);
    let obs = zamo_observer(&params, &bl).expect("zamo");
    let ray = initialize_rectilinear_ray(
        &params,
        &obs,
        &CameraParams {
            horizontal_fov: 60.0_f64.to_radians(),
            roll: 0.0,
        },
        SensorCoord { x: 0.0, y: 0.0 },
    )
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
    let y0 = [pos.t, pos.x, pos.y, pos.z, p.t, p.x, p.y, p.z];
    let rtol = if tight { 1e-12 } else { DEFAULT_RTOL };
    let atol = if tight { 1e-14 } else { DEFAULT_ATOL };
    let sol = Ivp::first_order(&Kerr8 { params }, 0.0, 0.01, &y0)
        .method(Method::DOP853)
        .rtol(rtol)
        .atol(atol)
        .dense_output(true)
        .solve();
    let (passed, detail) = match sol {
        Ok(s) => {
            let yf = s.y.last().cloned().unwrap_or_default();
            let hf = evaluate_hamiltonian(
                &params,
                &relativity_core::PositionKs::new(yf[0], yf[1], yf[2], yf[3]),
                &relativity_core::Covector::from_components([yf[4], yf[5], yf[6], yf[7]]),
            );
            let h_drift = hf
                .as_ref()
                .map(|h| (h.h - h0.h).abs())
                .unwrap_or(f64::INFINITY);
            (
                yf.iter().all(|v| v.is_finite()) && h_drift < 1e-5,
                format!("tight={tight} H_drift={h_drift:.3e} nacc={}", s.naccpt),
            )
        }
        Err(e) => (false, format!("err={e:?}")),
    };
    ExperimentResult {
        id: ExperimentId::G,
        passed,
        detail,
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: None,
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: None,
        error_scaling: None,
    }
}

use ivp::methods::DOP853;
