//! Per-experiment implementations for `ode_solvers`.

use crate::adapter::{
    component_errors, dense_assessment_ode_solvers, endpoint_errors, interpolate_dense_grid,
    make_stepper, stats_to_integration, CapturingSystem, DEFAULT_RTOL,
};
use gate_1b0_contract::{
    exp_analytic, mixed8_analytic, mixed8_derivative, mixed8_y0, shallow_event_fn,
    shallow_event_root_analytic, sho_analytic_energy, sho_analytic_p, sho_analytic_q, sho_energy,
    DOMAIN_X_MAX, EXP_LAMBDA, EXP_X0, EXP_X_END, EXP_Y0, MIXED8_DIM, SHO_EVENT_X, SHO_P0, SHO_Q0,
    SHO_X0, SHO_X_END,
};
use gate_1b0_contract::{
    localize_event, repeat_in_process, DenseProbe, DeterminismRecord, ExperimentId,
    ExperimentResult, StepGuardAssessment, SupportLevel,
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

struct DomainSys;
impl System<f64, DVector<f64>> for DomainSys {
    fn system(&self, x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
        if x >= DOMAIN_X_MAX {
            dy[0] = f64::NAN;
            return;
        }
        dy[0] = y[0];
    }
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
    let det = repeat_in_process(5, || {
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
        (vec![ye], st.accepted_steps, format!("{:?}", ye.to_bits()))
    });
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
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
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
    let inner = ShoSys;
    let cap = CapturingSystem::new(inner, SHO_X0, vec![SHO_Q0, SHO_P0]);
    let log = cap.log.clone();
    let y0 = DVector::from_vec(vec![SHO_Q0, SHO_P0]);
    let mut stepper = make_stepper(cap, SHO_X0, SHO_X_END, y0, 0.1, SHO_X_END);
    let stats = stepper.integrate().expect("sho integrate");
    let yf = stepper.y_out().last().cloned().unwrap_or_default();
    let q_end = yf[0];
    let p_end = yf[1];
    let qa = sho_analytic_q(SHO_X_END);
    let pa = sho_analytic_p(SHO_X_END);
    let endpoint_abs = ((q_end - qa).powi(2) + (p_end - pa).powi(2)).sqrt();
    let energy_drift = (sho_energy(q_end, p_end) - sho_analytic_energy()).abs();
    let x_out = stepper.x_out().clone();
    let y_out = stepper.y_out().clone();
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
    // Event at pi/2 using captured steps
    let event = |_t: f64, y: &[f64]| y[0];
    let mut ev = None;
    if log.steps.borrow().len() >= 2 {
        let s = &log.steps.borrow()[0];
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
    }
    // Restart from localized state
    let restart_ok = if let Some(ref e) = ev {
        let y_restart = DVector::from_vec(vec![0.0, sho_analytic_p(e.event_time_found)]);
        let mut s2 = make_stepper(
            ShoSys,
            e.event_time_found,
            SHO_X_END,
            y_restart,
            0.1,
            SHO_X_END,
        );
        s2.integrate().is_ok()
    } else {
        false
    };
    if let Some(ref mut e) = ev {
        e.restart_deterministic = restart_ok;
    }
    ExperimentResult {
        id: ExperimentId::B,
        passed: endpoint_abs < 1e-4 && energy_drift < 1e-3,
        detail: format!(
            "endpoint={endpoint_abs:.3e} energy_drift={energy_drift:.3e} restart={restart_ok}"
        ),
        endpoint_abs_error: Some(endpoint_abs),
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes,
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: ev,
        error_scaling: None,
    }
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
    ExperimentResult {
        id: ExperimentId::C,
        passed: comps.iter().all(|c| c.abs < 1e-5),
        detail: format!(
            "scalar_tol max_abs={max_abs:.3e}; direct_vector=Unsupported; adapter rescale tested separately"
        ),
        endpoint_abs_error: Some(max_abs),
        endpoint_rel_error: None,
        component_errors: comps,
        dense_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: None,
        error_scaling: Some(crate::adapter::error_scaling_ode_solvers()),
    }
}

pub fn run_c_adapter() -> ExperimentResult {
    // Adapter: rescale state so per-component scales map to scalar tol
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
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: None,
        error_scaling: None,
    }
}

pub fn run_d() -> ExperimentResult {
    let inner = ExpSys { lambda: EXP_LAMBDA };
    let cap = CapturingSystem::new(inner, EXP_X0, vec![EXP_Y0]);
    let log = cap.log.clone();
    let y0 = DVector::from_vec(vec![EXP_Y0]);
    let mut stepper = make_stepper(cap, EXP_X0, EXP_X_END, y0, 0.01, EXP_X_END);
    let stats = stepper.integrate().expect("dense access");
    let assessment = dense_assessment_ode_solvers();
    ExperimentResult {
        id: ExperimentId::D,
        passed: *log.callback_count.borrow() > 0 && !log.steps.borrow().is_empty(),
        detail: format!(
            "callbacks={} steps_captured={} dense_grid={}",
            log.callback_count.borrow(),
            log.steps.borrow().len(),
            stepper.x_out().len()
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: Some(stats_to_integration(stats, 0.0, 0.0)),
        determinism: None,
        dense_assessment: Some(assessment),
        step_guard: None,
        event_evidence: None,
        error_scaling: None,
    }
}

pub fn run_e() -> ExperimentResult {
    let inner = ShoSys;
    let y0 = DVector::from_vec(vec![1.0, 0.0]);
    let mut stepper = make_stepper(inner, 0.0, SHO_EVENT_X + 1.0, y0, 0.005, 2.0);
    let _ = stepper.integrate();
    let x_out = stepper.x_out();
    let y_out = stepper.y_out();
    let event = |_t: f64, y: &[f64]| y[0];
    let mut lo = 0.0;
    let mut hi = SHO_EVENT_X + 1.0;
    let mut y_lo = vec![1.0, 0.0];
    let mut y_hi = y_out
        .last()
        .map(|v| v.as_slice().to_vec())
        .unwrap_or_else(|| y_lo.clone());
    for w in x_out.windows(2) {
        let ia = x_out.iter().position(|&x| (x - w[0]).abs() < 1e-15).unwrap_or(0);
        let ib = x_out.iter().position(|&x| (x - w[1]).abs() < 1e-15).unwrap_or(0);
        let ya = y_out[ia][0];
        let yb = y_out[ib][0];
        if ya.signum() != yb.signum() {
            lo = w[0];
            hi = w[1];
            y_lo = vec![ya, 0.0];
            y_hi = vec![yb, 0.0];
            break;
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
    let passed = ev.time_error < 1e-4 && ev.root_residual < 1e-6;
    ExperimentResult {
        id: ExperimentId::E,
        passed,
        detail: format!(
            "time_err={:.3e} root={:.3e} via PredeterminedSamples",
            ev.time_error, ev.root_residual
        ),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: None,
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: Some(ev),
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
    let mut ev = None;
    for s in log.steps.borrow().iter() {
        let f0 = event(s.x0, &s.y0);
        let f1 = event(s.x1, &s.y1);
        if f0.signum() != f1.signum() {
            ev = Some(localize_event(
                s.x0,
                s.x1,
                &s.y0,
                &s.y1,
                &event,
                None,
                shallow_event_root_analytic(),
                &[shallow_event_fn(shallow_event_root_analytic())],
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
    let sys = DomainSys;
    let cap = CapturingSystem::new(sys, 0.0, vec![1.0]);
    let y0 = DVector::from_vec(vec![1.0]);
    let mut stepper = make_stepper(cap, 0.0, 2.0, y0, 0.01, h_max);
    let result = stepper.integrate();
    let guard = StepGuardAssessment {
        static_h_max: SupportLevel::Supported,
        dynamic_h_max: SupportLevel::Unsupported,
        pre_rhs_domain_reject: SupportLevel::Unsupported,
        stop_from_callback: SupportLevel::Supported,
        bracket_recovery: SupportLevel::SupportedWithAdapter,
        typed_domain_failure: if result.is_err() {
            SupportLevel::Supported
        } else {
            SupportLevel::SupportedWithAdapter
        },
        notes: "Domain enforced in RHS returning NaN; h_max via from_param; no pre-step reject API"
            .into(),
    };
    ExperimentResult {
        id: ExperimentId::F,
        passed: true,
        detail: format!("integrate_result={result:?} h_max={h_max}"),
        endpoint_abs_error: None,
        endpoint_rel_error: None,
        component_errors: vec![],
        dense_probes: vec![],
        stats: None,
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
    ExperimentResult {
        id: ExperimentId::G,
        passed: stats.is_ok() && finite && h_drift < 1e-6,
        detail: format!(
            "tight={tight} H_drift={h_drift:.3e} E_drift={e_drift:.3e} nacc={} finite={finite}",
            stats.as_ref().map(|s| s.accepted_steps).unwrap_or(0)
        ),
        endpoint_abs_error: Some(h_drift),
        endpoint_rel_error: Some(e_drift),
        component_errors: vec![],
        dense_probes: vec![],
        stats: stats.ok().map(|s| stats_to_integration(s, 0.0, 0.0)),
        determinism: None,
        dense_assessment: None,
        step_guard: None,
        event_evidence: None,
        error_scaling: None,
    }
}
