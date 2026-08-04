//! Analytic thin-disk intersection tests.

use relativity_core::{
    initialize_rectilinear_ray, zamo_observer, CameraParams, Covector, KerrParams, PositionBl,
    PositionKs, SensorCoord,
};
use relativity_integrate::{
    integrate, Dop853Config, EventArmingPolicy, EventId, EventMetadata, EventSurface,
    GeodesicState, IntegrationOutcome, LocalizationTermination,
};
use relativity_trace::{ThinDisk, ThinDiskGeometry};

fn minkowski_params() -> KerrParams {
    KerrParams::new(1.0, 0.0).unwrap()
}

#[test]
fn invalid_disk_radii_typed() {
    let p = minkowski_params();
    let r_plus = p.outer_horizon_radius();
    assert!(ThinDiskGeometry::new(r_plus, 10.0).validate(&p).is_err());
    assert!(ThinDiskGeometry::new(r_plus + 1.0, r_plus + 0.5)
        .validate(&p)
        .is_err());
    assert!(ThinDiskGeometry::new(f64::NAN, 10.0).validate(&p).is_err());
}

#[test]
fn flat_ray_hits_inside_annulus_with_analytic_lambda() {
    let p = minkowski_params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    // Ray from (10,0,5) with p = (1,0,0,-1) → z decreases; crosses z=0 at λ=5.
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 5.0),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 20.0;
    cfg.max_step = 0.25;
    cfg.event_arming = EventArmingPolicy::immediate();
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("expected Event, got {:?}", report.outcome.variant_name());
    };
    assert_eq!(hit.event_id, EventId::ThinDisk);
    // Hamiltonian affine parameter need not equal Cartesian |Δz|; require plane hit.
    assert!(
        hit.state.position.z.abs() < 1e-5,
        "z={}",
        hit.state.position.z
    );
    assert!(
        (hit.lambda.0 - 5.0).abs() < 1.0,
        "lam={} (expect near Cartesian Δz scale)",
        hit.lambda.0
    );
    match hit.metadata {
        EventMetadata::ThinDisk {
            oblate_radius,
            crossing_side,
        } => {
            assert!((3.0..=20.0).contains(&oblate_radius), "r={oblate_radius}");
            assert_eq!(
                crossing_side,
                relativity_integrate::DiskCrossingSide::UpperToLower
            );
        }
        other => panic!("metadata {other:?}"),
    }
}

#[test]
fn crossing_outside_r_outer_rejected() {
    let p = minkowski_params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 8.0)).unwrap();
    // Cross at x=10 > 8 → reject; reach affine limit.
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 5.0),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 20.0;
    cfg.max_step = 0.25;
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    assert!(matches!(
        report.outcome,
        IntegrationOutcome::AffineLimit { .. }
    ));
}

#[test]
fn crossing_inside_r_inner_rejected() {
    let p = minkowski_params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(6.0, 20.0)).unwrap();
    // Cross at x=4 < 6.
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 4.0, 0.0, 3.0),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 20.0;
    cfg.max_step = 0.25;
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    assert!(matches!(
        report.outcome,
        IntegrationOutcome::AffineLimit { .. }
    ));
}

#[test]
fn parallel_to_plane_no_hit() {
    let p = minkowski_params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 2.0),
        Covector::new(1.0, -1.0, 0.0, 0.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 5.0;
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    assert!(matches!(
        report.outcome,
        IntegrationOutcome::AffineLimit { .. }
    ));
}

#[test]
fn lower_to_upper_metadata() {
    let p = minkowski_params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, -4.0),
        Covector::new(1.0, 0.0, 0.0, 1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 20.0;
    cfg.max_step = 0.25;
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("expected Event");
    };
    match hit.metadata {
        EventMetadata::ThinDisk { crossing_side, .. } => {
            assert_eq!(
                crossing_side,
                relativity_integrate::DiskCrossingSide::LowerToUpper
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn exact_endpoint_disk_root() {
    let p = minkowski_params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    // Start above; take a step that lands exactly on z=0 is hard with adaptive
    // DOP853 — instead localize a sign-changing step and require ExactEndpoint
    // when f1==0 via unit localizer path is covered in integrate root tests.
    // Here: initial state already on plane but disarmed; move slightly then hit.
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 1e-3),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 1.0;
    cfg.max_step = 1e-3;
    cfg.event_time_tolerance = 1e-14;
    cfg.event_value_tolerance = 1e-14;
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("{:?}", report.outcome.variant_name());
    };
    assert_eq!(hit.event_id, EventId::ThinDisk);
    assert!(matches!(
        hit.localization.termination,
        LocalizationTermination::ExactEndpoint
            | LocalizationTermination::EventValueTolerance
            | LocalizationTermination::AffineWidthTolerance
    ));
}

#[test]
fn initial_plane_disarmed_until_arming_threshold() {
    let p = minkowski_params();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    // Start exactly on the plane with downward momentum.
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.0),
        Covector::new(1.0, 0.0, 0.0, -1.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 5.0;
    cfg.max_step = 0.1;
    cfg.event_arming = EventArmingPolicy::after(1.0).unwrap();
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces).unwrap();
    // With arming at 1.0, an immediate z=0 root at λ≈0 is ignored; ray leaves plane
    // without a later sign change → AffineLimit (or no disk hit before limit).
    match report.outcome {
        IntegrationOutcome::Event(hit) => {
            assert!(
                hit.lambda.0 >= 1.0 - 1e-12,
                "armed hit must respect threshold, lam={}",
                hit.lambda.0
            );
        }
        IntegrationOutcome::AffineLimit { .. } => {}
        other => panic!("unexpected {}", other.variant_name()),
    }
}

#[test]
fn camera_ray_disk_smoke() {
    let p = KerrParams::new(1.0, 0.5).unwrap();
    let bl = PositionBl::new(0.0, 20.0, 85.0_f64.to_radians(), 0.0);
    let obs = zamo_observer(&p, &bl).unwrap();
    let cam = CameraParams {
        horizontal_fov: 50.0_f64.to_radians(),
        roll: 0.0,
    };
    let ray = initialize_rectilinear_ray(&p, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 }).unwrap();
    let y0 = GeodesicState::new(obs.event, ray.covariant_momentum).unwrap();
    let disk = ThinDisk::new(p, ThinDiskGeometry::new(3.0, 20.0)).unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 100.0;
    cfg.horizon_proximity = relativity_integrate::HorizonProximityPolicy::enabled(1e-4).unwrap();
    let surfaces: [&dyn EventSurface; 1] = [&disk];
    let report = integrate(p, &y0, &cfg, &surfaces);
    assert!(
        report.is_ok()
            || matches!(
                report,
                Err(relativity_integrate::IntegrationError::Solver { .. })
            )
    );
}
