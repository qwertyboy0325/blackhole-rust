use relativity_core::{Covector, KerrParams, PositionKs};
use relativity_integrate::{
    integrate, is_eligible_crossing, CrossingDirection, Dop853Config, EscapeSphere, EventId,
    EventSurface, GeodesicState, IntegrationOutcome, OuterHorizon,
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
    cfg.max_step = 1.0; // coarse so event is interior
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
    // Adapter outcome state is localized.
    assert!((hit.state.position.x - 20.0).abs() < 1e-5);
    assert!((hit.raw_solver_stop.state.position.x - 20.0).abs() > 1e-6);
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

    // Uninterrupted reference to λ past the event, then compare restart segment.
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
    // Outward from r≈10 toward escape; also register horizon (should not fire first).
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
        -1.0,
        CrossingDirection::Increasing
    ));
    assert!(!is_eligible_crossing(1.0, 2.0, CrossingDirection::Any));
    assert!(!is_eligible_crossing(0.0, 0.0, CrossingDirection::Any));
}

#[test]
fn no_tangent_support_claimed() {
    // Identical signs / zero product → not eligible (no tangent claim).
    assert!(!is_eligible_crossing(1e-16, 1e-16, CrossingDirection::Any));
    assert!(!is_eligible_crossing(1.0, 0.0, CrossingDirection::Any));
}
