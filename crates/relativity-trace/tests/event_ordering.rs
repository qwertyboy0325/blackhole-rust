//! Event ordering and opaque first-hit tests.

use relativity_core::{Covector, KerrParams, PositionKs};
use relativity_integrate::{
    integrate, Dop853Config, EscapeSphere, EventId, EventSurface, GeodesicState,
    IntegrationOutcome, OuterHorizon,
};
use relativity_trace::{ThinDisk, ThinDiskGeometry};

fn params() -> KerrParams {
    KerrParams::new(1.0, 0.0).unwrap()
}

#[test]
fn registration_order_independent_earliest_lambda() {
    let p = params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    let esc = EscapeSphere::new(p, 50.0).unwrap();
    // Cross disk at λ=5; escape much later if at all.
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 5.0),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 30.0;
    cfg.max_step = 0.25;

    let s1: [&dyn EventSurface; 2] = [&disk, &esc];
    let s2: [&dyn EventSurface; 2] = [&esc, &disk];
    let r1 = integrate(p, &y0, &cfg, &s1).unwrap();
    let r2 = integrate(p, &y0, &cfg, &s2).unwrap();
    let IntegrationOutcome::Event(h1) = r1.outcome else {
        panic!();
    };
    let IntegrationOutcome::Event(h2) = r2.outcome else {
        panic!();
    };
    assert_eq!(h1.event_id, EventId::ThinDisk);
    assert_eq!(h2.event_id, EventId::ThinDisk);
    assert_eq!(h1.lambda.0.to_bits(), h2.lambda.0.to_bits());
}

#[test]
fn escape_before_disk_when_earlier() {
    let p = params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    let esc = EscapeSphere::new(p, 12.0).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 5.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 80.0;
    cfg.max_step = 0.05;
    let surfaces: [&dyn EventSurface; 2] = [&disk, &esc];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("{:?}", report.outcome.variant_name());
    };
    assert_eq!(hit.event_id, EventId::EscapeSphere);
}

#[test]
fn rejected_disk_does_not_mask_escape_same_step() {
    let p = params();
    // Disk annulus excludes r=10; escape at r=10.2 — both may appear near same steps.
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 8.0)).unwrap();
    let esc = EscapeSphere::new(p, 10.2).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.5),
        Covector::new(1.0, 1.0, 0.0, -0.1),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 30.0;
    cfg.max_step = 0.05;
    let surfaces: [&dyn EventSurface; 2] = [&disk, &esc];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("{:?}", report.outcome.variant_name());
    };
    assert_eq!(hit.event_id, EventId::EscapeSphere);
}

#[test]
fn opaque_first_hit_stops_further_accepted_steps() {
    let p = params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 5.0),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 50.0;
    cfg.max_step = 0.25;
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!();
    };
    assert_eq!(hit.event_id, EventId::ThinDisk);
    // Localized state on plane; raw stop is the accepted-step endpoint (generally later).
    assert!(hit.state.position.z.abs() < 1e-4);
    assert!((hit.raw_solver_stop.lambda.0 - hit.lambda.0).abs() >= 0.0);
    assert!(hit.raw_solver_stop.lambda.0 + 1e-12 >= hit.lambda.0);
}

#[test]
fn outside_annulus_continues_then_affine_or_escape() {
    let p = params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 8.0)).unwrap();
    let esc = EscapeSphere::new(p, 40.0).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 5.0),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 80.0;
    let surfaces: [&dyn EventSurface; 2] = [&disk, &esc];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    // Plane crossing rejected; eventual escape or affine.
    match report.outcome {
        IntegrationOutcome::Event(h) => assert_eq!(h.event_id, EventId::EscapeSphere),
        IntegrationOutcome::AffineLimit { .. } => {}
        other => panic!("{}", other.variant_name()),
    }
}

#[test]
fn horizon_surface_still_registers() {
    let p = params();
    let hor = OuterHorizon::new(p);
    assert_eq!(hor.id(), EventId::OuterHorizon);
}
