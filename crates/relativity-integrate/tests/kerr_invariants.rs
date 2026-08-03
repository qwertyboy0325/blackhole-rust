use relativity_core::{
    initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams, PositionBl, SensorCoord,
};
use relativity_integrate::{
    integrate, run_and_check, CorpusId, Dop853Config, EventId, GeodesicState, IntegrationOutcome,
    SurfaceApproachReason, CORPUS,
};

fn zamo_ray(params: KerrParams, r: f64, theta: f64, sx: f64) -> GeodesicState {
    let bl = PositionBl::new(0.0, r, theta, 0.0);
    let obs = zamo_observer(&params, &bl).unwrap();
    let cam = CameraParams {
        horizontal_fov: 50.0_f64.to_radians(),
        roll: 0.0,
    };
    let ray =
        initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: sx, y: 0.0 }).unwrap();
    GeodesicState::new(obs.event, ray.covariant_momentum).unwrap()
}

fn endpoint_max_abs(a: &GeodesicState, b: &GeodesicState) -> f64 {
    a.to_array()
        .iter()
        .zip(b.to_array().iter())
        .map(|(u, v)| (u - v).abs())
        .fold(0.0, f64::max)
}

#[test]
fn p_t_constant_and_h_reported_without_projection() {
    let params = KerrParams::new(1.0, 0.9).unwrap();
    let y0 = zamo_ray(params, 50.0, std::f64::consts::FRAC_PI_2, 0.0);
    let pt0 = y0.momentum.t;
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 0.5;
    cfg.max_step = 0.05;
    let report = integrate(params, &y0, &cfg, &[]).unwrap();
    assert!(
        report.diagnostics.p_t_max_abs_drift < 1e-10,
        "{}",
        report.diagnostics.p_t_max_abs_drift
    );
    assert!((report.diagnostics.p_t_final - pt0).abs() < 1e-10);
    assert!(report.diagnostics.h_initial.is_finite());
    assert!(report.diagnostics.h_final.is_finite());
}

#[test]
fn three_level_kerr_convergence() {
    let params = KerrParams::new(1.0, 0.5).unwrap();
    let y0 = zamo_ray(params, 80.0, std::f64::consts::FRAC_PI_2, 0.1);

    let mut loose = Dop853Config::diagnostic_default();
    loose.affine_limit = 0.5;
    loose.relative_tolerance = [1e-6; 8];
    loose.absolute_tolerance = [1e-8; 8];
    let medium = loose.clone().with_tighter_tol(1e-2);
    let tight = medium.clone().with_tighter_tol(1e-2);

    let run = |cfg: &Dop853Config| {
        let r = integrate(params, &y0, cfg, &[]).unwrap();
        let IntegrationOutcome::AffineLimit { state, stats, .. } = r.outcome else {
            panic!("affine");
        };
        (state, stats, r.diagnostics)
    };

    let (s_l, st_l, d_l) = run(&loose);
    let (s_m, st_m, d_m) = run(&medium);
    let (s_t, st_t, d_t) = run(&tight);

    let d_loose_medium = endpoint_max_abs(&s_l, &s_m);
    let d_medium_tight = endpoint_max_abs(&s_m, &s_t);

    // Justified slack: successive tightening must not increase endpoint separation.
    assert!(
        d_medium_tight <= d_loose_medium + 1e-15,
        "d_lm={d_loose_medium} d_mt={d_medium_tight}"
    );

    eprintln!(
        "kerr3: d_lm={d_loose_medium:.3e} d_mt={d_medium_tight:.3e} \
         Hmax=({:.3e},{:.3e},{:.3e}) Hfin=({:.3e},{:.3e},{:.3e}) \
         pt_drift=({:.3e},{:.3e},{:.3e}) steps=({}/{},{}/{},{}/{}) rhs=({},{},{})",
        d_l.h_max_abs_residual,
        d_m.h_max_abs_residual,
        d_t.h_max_abs_residual,
        d_l.h_final,
        d_m.h_final,
        d_t.h_final,
        d_l.p_t_max_abs_drift,
        d_m.p_t_max_abs_drift,
        d_t.p_t_max_abs_drift,
        st_l.accepted_steps,
        st_l.rejected_steps,
        st_m.accepted_steps,
        st_m.rejected_steps,
        st_t.accepted_steps,
        st_t.rejected_steps,
        st_l.rhs_evaluations,
        st_m.rhs_evaluations,
        st_t.rhs_evaluations,
    );
}

#[test]
fn horizon_and_escape_corpus_expectations() {
    for case in CORPUS {
        if matches!(
            case.id,
            CorpusId::SchwarzschildInwardHorizon | CorpusId::MinkowskiEscapeSphere
        ) {
            let report = run_and_check(case)
                .unwrap_or_else(|e| panic!("{}: {e}", case.id.as_str()))
                .expect("report");
            match case.id {
                CorpusId::SchwarzschildInwardHorizon => {
                    let IntegrationOutcome::SurfaceApproach(a) = report.outcome else {
                        panic!("expected SurfaceApproach");
                    };
                    assert_eq!(a.event_id, EventId::OuterHorizon);
                    assert_eq!(a.reason, SurfaceApproachReason::SolverStepSizeTooSmall);
                    assert!(a.signed_event_value > 0.0);
                    assert!(a.signed_event_value <= a.approach_tolerance);
                }
                CorpusId::MinkowskiEscapeSphere => {
                    let IntegrationOutcome::Event(hit) = report.outcome else {
                        panic!("expected Event");
                    };
                    assert_eq!(hit.event_id, EventId::EscapeSphere);
                }
                _ => {}
            }
        }
    }
}

#[test]
fn no_non_finite_success() {
    let params = KerrParams::new(1.0, 0.999).unwrap();
    let y0 = zamo_ray(params, 30.0, 85.0_f64.to_radians(), 0.0);
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 0.25;
    let report = integrate(params, &y0, &cfg, &[]).unwrap();
    match report.outcome {
        IntegrationOutcome::Event(hit) => {
            assert!(hit.state.to_array().iter().all(|v| v.is_finite()));
        }
        IntegrationOutcome::SurfaceApproach(a) => {
            assert!(a.state.to_array().iter().all(|v| v.is_finite()));
        }
        IntegrationOutcome::AffineLimit { state, .. } => {
            assert!(state.to_array().iter().all(|v| v.is_finite()));
        }
    }
}
