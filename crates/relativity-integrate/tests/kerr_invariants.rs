use relativity_core::{
    initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams, PositionBl, SensorCoord,
};
use relativity_integrate::{
    integrate, run_and_check, CorpusId, Dop853Config, EscapeSphere, EventId, EventSurface,
    GeodesicState, IntegrationOutcome, OuterHorizon, CORPUS,
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
    // H residual reported; no projection ⇒ |H| may be small but diagnostics present.
    assert!(report.diagnostics.h_initial.is_finite());
    assert!(report.diagnostics.h_final.is_finite());
    assert!(report.diagnostics.h_max_abs_residual.is_finite());
}

#[test]
fn tighter_tolerance_runs_converge() {
    let params = KerrParams::new(1.0, 0.5).unwrap();
    let y0 = zamo_ray(params, 80.0, std::f64::consts::FRAC_PI_2, 0.1);
    let mut loose = Dop853Config::diagnostic_default();
    loose.affine_limit = 0.5;
    loose.relative_tolerance = [1e-8; 8];
    loose.absolute_tolerance = [1e-10; 8];
    let tight = loose.clone().with_tighter_tol(1e-2);
    let a = integrate(params, &y0, &loose, &[]).unwrap();
    let b = integrate(params, &y0, &tight, &[]).unwrap();
    let IntegrationOutcome::AffineLimit { state: sa, .. } = a.outcome else {
        panic!();
    };
    let IntegrationOutcome::AffineLimit { state: sb, .. } = b.outcome else {
        panic!();
    };
    let err: f64 = sa
        .to_array()
        .iter()
        .zip(sb.to_array().iter())
        .map(|(u, v)| (u - v).abs())
        .fold(0.0, f64::max);
    assert!(err < 1e-6, "endpoint separation {err}");
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
                    let IntegrationOutcome::Event(hit) = report.outcome else {
                        panic!();
                    };
                    assert_eq!(hit.event_id, EventId::OuterHorizon);
                }
                CorpusId::MinkowskiEscapeSphere => {
                    let IntegrationOutcome::Event(hit) = report.outcome else {
                        panic!();
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
    let hor = OuterHorizon::new(params);
    let esc = EscapeSphere::new(params, 1.0e4).unwrap();
    let surfaces: [&dyn EventSurface; 2] = [&hor, &esc];
    let report = integrate(params, &y0, &cfg, &surfaces).unwrap();
    match report.outcome {
        IntegrationOutcome::Event(hit) => {
            assert!(hit.state.to_array().iter().all(|v| v.is_finite()));
        }
        IntegrationOutcome::AffineLimit { state, .. } => {
            assert!(state.to_array().iter().all(|v| v.is_finite()));
        }
    }
}
