//! Gate 2A0-3: shade layer vs encoding and disk-suppressed differential.

use relativity_core::{CameraParams, KerrParams, PositionBl};
use relativity_integrate::{Dop853Config, EventArmingPolicy, HorizonProximityPolicy};
use relativity_trace::{
    encode_ppm, rgb_frame_diff_count, shade_diagnostic, shade_many, trace_data_digest, trace_grid,
    write_outcome_ppm, write_rhs_pgm, DiagnosticShadeStyle, ThinDiskGeometry, TraceGrid,
    TraceScene,
};

fn scene(w: u32, h: u32) -> TraceScene {
    let kerr = KerrParams::new(1.0, 0.999).unwrap();
    let disk = ThinDiskGeometry::new(3.0, 20.0);
    disk.validate(&kerr).unwrap();
    let mut integrator = Dop853Config::diagnostic_default();
    integrator.relative_tolerance = [1e-8; 8];
    integrator.absolute_tolerance = [1e-9, 1e-9, 1e-9, 1e-9, 1e-10, 1e-10, 1e-10, 1e-10];
    integrator.affine_limit = 120.0;
    integrator.max_step = 2.0;
    integrator.max_accepted_steps = 2_000;
    integrator.horizon_proximity = HorizonProximityPolicy::enabled(1e-4).unwrap();
    integrator.event_arming = EventArmingPolicy::after(1e-12).unwrap();
    TraceScene {
        kerr,
        observer: PositionBl::new(0.0, 20.0, 85.0_f64.to_radians(), 0.0),
        camera: CameraParams {
            horizontal_fov: 50.0_f64.to_radians(),
            roll: 0.0,
        },
        disk,
        escape_radius: 80.0,
        event_arming: integrator.event_arming.clone(),
        integrator,
        grid: TraceGrid {
            width: w,
            height: h,
        },
    }
}

#[test]
fn write_outcome_ppm_equals_encode_legacy_shade() {
    let bundle = trace_grid(&scene(8, 8)).unwrap();
    let a = write_outcome_ppm(&bundle);
    let b = encode_ppm(&shade_diagnostic(
        &bundle,
        DiagnosticShadeStyle::Gate1b2Categorical,
    ));
    assert_eq!(a, b);
}

#[test]
fn shade_many_shares_trace_digest_and_differs_in_ppm() {
    let bundle = trace_grid(&scene(8, 8)).unwrap();
    let d0 = trace_data_digest(&bundle);
    let shaded = shade_many(
        &bundle,
        &[
            DiagnosticShadeStyle::Gate1b2Categorical,
            DiagnosticShadeStyle::DiskSuppressed,
        ],
    );
    assert_eq!(shaded.len(), 2);
    assert_ne!(shaded[0].ppm_digest, shaded[1].ppm_digest);
    assert_eq!(d0, trace_data_digest(&bundle));
    let changed = rgb_frame_diff_count(&shaded[0].frame, &shaded[1].frame).unwrap();
    let disk_hits = bundle
        .outcomes
        .iter()
        .filter(|o| matches!(o.class(), relativity_trace::OutcomeClass::DiskHit))
        .count() as u64;
    assert_eq!(changed, disk_hits);
}

#[test]
fn ppm_header_is_p6_row_major() {
    let bundle = trace_grid(&scene(4, 3)).unwrap();
    let ppm = write_outcome_ppm(&bundle);
    assert!(ppm.starts_with(b"P6\n4 3\n255\n"));
    assert_eq!(ppm.len(), b"P6\n4 3\n255\n".len() + 4 * 3 * 3);
}

#[test]
fn rhs_pgm_unchanged_by_shading() {
    let bundle = trace_grid(&scene(6, 6)).unwrap();
    let pgm0 = write_rhs_pgm(&bundle);
    let _ = shade_many(
        &bundle,
        &[
            DiagnosticShadeStyle::Gate1b2Categorical,
            DiagnosticShadeStyle::DiskSuppressed,
        ],
    );
    assert_eq!(pgm0, write_rhs_pgm(&bundle));
}
