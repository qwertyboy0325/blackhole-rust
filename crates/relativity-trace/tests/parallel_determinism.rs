//! Gate 2A0-2: parallel vs serial byte identity and row-major ordering.

use relativity_core::{CameraParams, KerrParams, PositionBl};
use relativity_integrate::{Dop853Config, EventArmingPolicy, HorizonProximityPolicy};
use relativity_trace::{
    outcome_class_bytes, trace_grid, trace_grid_with_execution, write_outcome_ppm, write_rhs_pgm,
    ThinDiskGeometry, TraceExecution, TraceGrid, TraceScene,
};
use std::num::NonZeroUsize;

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
fn parallel_thread_counts_match_serial_images() {
    let s = scene(8, 8);
    let serial = trace_grid(&s).unwrap();
    let serial_ppm = write_outcome_ppm(&serial);
    let serial_pgm = write_rhs_pgm(&serial);
    let serial_class = outcome_class_bytes(&serial);

    for threads in [1usize, 2, 4] {
        let parallel = trace_grid_with_execution(
            &s,
            TraceExecution::Parallel {
                threads: NonZeroUsize::new(threads).unwrap(),
            },
        )
        .unwrap();
        assert_eq!(outcome_class_bytes(&parallel), serial_class);
        assert_eq!(write_outcome_ppm(&parallel), serial_ppm);
        assert_eq!(write_rhs_pgm(&parallel), serial_pgm);
    }
}

#[test]
fn parallel_repeated_runs_identical() {
    let s = scene(6, 6);
    let a = trace_grid_with_execution(
        &s,
        TraceExecution::Parallel {
            threads: NonZeroUsize::new(2).unwrap(),
        },
    )
    .unwrap();
    let b = trace_grid_with_execution(
        &s,
        TraceExecution::Parallel {
            threads: NonZeroUsize::new(2).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(outcome_class_bytes(&a), outcome_class_bytes(&b));
    assert_eq!(write_outcome_ppm(&a), write_outcome_ppm(&b));
    assert_eq!(write_rhs_pgm(&a), write_rhs_pgm(&b));
}

#[test]
fn no_nonfinite_success_in_parallel() {
    let s = scene(4, 4);
    let bundle = trace_grid_with_execution(
        &s,
        TraceExecution::Parallel {
            threads: NonZeroUsize::new(2).unwrap(),
        },
    )
    .unwrap();
    for o in &bundle.outcomes {
        if !matches!(o, relativity_trace::RayOutcome::Failed(_)) {
            assert!(o.state_finite());
        }
    }
}
