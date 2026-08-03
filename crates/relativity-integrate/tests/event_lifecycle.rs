use relativity_core::{Covector, KerrParams, PositionKs};
use relativity_integrate::{
    integrate, is_eligible_crossing, CrossingDirection, Dop853Config, EscapeSphere, EventId,
    EventSurface, GeodesicState, HorizonProximityPolicy, IntegrationOutcome, OuterHorizon,
    SurfaceApproachReason,
};

fn params_m() -> KerrParams {
    KerrParams::new(1.0e-18, 0.0).unwrap()
}

fn outward() -> GeodesicState {
    GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap()
}

#[test]
fn localized_authoritative_and_raw_retained() {
    let params = params_m();
    let y0 = outward();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 50.0;
    cfg.max_step = 1.0;
    let esc = EscapeSphere::new(params, 20.0).unwrap();
    let surfaces: [&dyn EventSurface; 1] = [&esc];
    let report = integrate(params, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("event");
    };
    assert!(hit.localization.interpolation_calls > 0);
    let sep = report
        .diagnostics
        .raw_vs_localized_lambda_separation
        .unwrap();
    assert!(sep > 0.0, "raw vs localized sep {sep}");
    assert!((hit.state.position.x - 20.0).abs() < 1e-5);
}

#[test]
fn restart_from_event_matches_uninterrupted() {
    let params = params_m();
    let y0 = outward();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 50.0;
    cfg.max_step = 0.5;
    let esc = EscapeSphere::new(params, 15.0).unwrap();
    let surfaces: [&dyn EventSurface; 1] = [&esc];
    let hit_report = integrate(params, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = hit_report.outcome else {
        panic!("event");
    };

    let mut cfg_ref = cfg.clone();
    cfg_ref.affine_limit = hit.lambda.0 + 2.0;
    let reference = integrate(params, &y0, &cfg_ref, &[]).unwrap();
    let IntegrationOutcome::AffineLimit {
        state: ref_state, ..
    } = reference.outcome
    else {
        panic!("affine");
    };

    let mut cfg_restart = cfg.clone();
    cfg_restart.affine_limit = 2.0;
    let restarted = integrate(params, &hit.state, &cfg_restart, &[]).unwrap();
    let IntegrationOutcome::AffineLimit {
        state: rst_state, ..
    } = restarted.outcome
    else {
        panic!("affine");
    };

    for i in 0..8 {
        let a = ref_state.to_array()[i];
        let b = rst_state.to_array()[i];
        assert!((a - b).abs() < 1e-5, "comp {i}: {a} vs {b}");
    }
}

#[test]
fn earliest_of_multiple_events_selected() {
    let params = KerrParams::new(1.0, 0.0).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 100.0;
    cfg.max_step = 0.5;
    let hor = OuterHorizon::new(params);
    let esc = EscapeSphere::new(params, 15.0).unwrap();
    let surfaces: [&dyn EventSurface; 2] = [&hor, &esc];
    let report = integrate(params, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("expected event, got {}", report.outcome.variant_name());
    };
    assert_eq!(hit.event_id, EventId::EscapeSphere);
}

#[test]
fn crossing_direction_filters() {
    assert!(is_eligible_crossing(
        -1.0,
        1.0,
        CrossingDirection::Increasing
    ));
    assert!(!is_eligible_crossing(
        -1.0,
        1.0,
        CrossingDirection::Decreasing
    ));
    assert!(is_eligible_crossing(
        1.0,
        -1.0,
        CrossingDirection::Decreasing
    ));
    assert!(!is_eligible_crossing(
        1.0,
        1e-20,
        CrossingDirection::Decreasing
    ));
}

#[test]
fn no_tangent_or_proximity_as_event() {
    assert!(!is_eligible_crossing(1e-16, 1e-16, CrossingDirection::Any));
    assert!(!is_eligible_crossing(
        1.0,
        0.0,
        CrossingDirection::Increasing
    ));
}

#[test]
fn horizon_proximity_is_surface_approach_not_event() {
    let params = KerrParams::new(1.0, 0.0).unwrap();
    use relativity_core::{
        initialize_rectilinear_ray, zamo_observer, CameraParams, PositionBl, SensorCoord,
    };
    let bl = PositionBl::new(0.0, 20.0, std::f64::consts::FRAC_PI_2, 0.0);
    let obs = zamo_observer(&params, &bl).unwrap();
    let cam = CameraParams {
        horizontal_fov: 50.0_f64.to_radians(),
        roll: 0.0,
    };
    let ray =
        initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 }).unwrap();
    let y0 = GeodesicState::new(obs.event, ray.covariant_momentum).unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 200.0;
    cfg.max_step = 0.5;
    cfg.horizon_proximity = HorizonProximityPolicy::enabled(1e-10).unwrap();
    let hor = OuterHorizon::new(params);
    let surfaces: [&dyn EventSurface; 1] = [&hor];
    let report = integrate(params, &y0, &cfg, &surfaces).unwrap();
    match report.outcome {
        IntegrationOutcome::SurfaceApproach(a) => {
            assert_eq!(a.event_id, EventId::OuterHorizon);
            assert!(a.signed_event_value > 0.0);
            assert!(a.signed_event_value <= a.approach_tolerance);
            assert_eq!(a.reason, SurfaceApproachReason::SolverStepSizeTooSmall);
        }
        IntegrationOutcome::Event(_) => panic!("must not be EventHit"),
        other => panic!("unexpected {}", other.variant_name()),
    }
}

#[test]
fn horizon_proximity_disabled_yields_solver_error_on_stall() {
    let params = KerrParams::new(1.0, 0.0).unwrap();
    use relativity_core::{
        initialize_rectilinear_ray, zamo_observer, CameraParams, PositionBl, SensorCoord,
    };
    let bl = PositionBl::new(0.0, 20.0, std::f64::consts::FRAC_PI_2, 0.0);
    let obs = zamo_observer(&params, &bl).unwrap();
    let cam = CameraParams {
        horizontal_fov: 50.0_f64.to_radians(),
        roll: 0.0,
    };
    let ray =
        initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 }).unwrap();
    let y0 = GeodesicState::new(obs.event, ray.covariant_momentum).unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 200.0;
    cfg.max_step = 0.5;
    // proximity disabled (default)
    let hor = OuterHorizon::new(params);
    let surfaces: [&dyn EventSurface; 1] = [&hor];
    let err = integrate(params, &y0, &cfg, &surfaces).unwrap_err();
    assert!(matches!(
        err,
        relativity_integrate::IntegrationError::Solver { .. }
    ));
}
