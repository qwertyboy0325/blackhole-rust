//! Single-ray and camera-grid tracing.

use rayon::prelude::*;
use relativity_core::{initialize_rectilinear_ray, zamo_observer, SensorCoord};
use relativity_integrate::{
    integrate, EscapeSphere, EventSurface, GeodesicState, IntegrationError, OuterHorizon,
};

use crate::camera::{pixel_index, sensor_at_pixel_center, TraceGrid};
use crate::disk::ThinDisk;
use crate::execution::TraceExecution;
use crate::outcome::{map_integration_report, RayFailure, RayOutcome};
use crate::scene::TraceScene;
use crate::surface_set::TraceSurfaceSet;

pub fn trace_ray_sensor(
    scene: &TraceScene,
    sensor: SensorCoord,
) -> Result<RayOutcome, IntegrationError> {
    trace_ray_sensor_with_surface_set(scene, sensor, TraceSurfaceSet::OpaqueDiskHorizonEscape)
}

pub fn trace_ray_sensor_with_surface_set(
    scene: &TraceScene,
    sensor: SensorCoord,
    surface_set: TraceSurfaceSet,
) -> Result<RayOutcome, IntegrationError> {
    scene.validate()?;
    let obs = zamo_observer(&scene.kerr, &scene.observer).map_err(IntegrationError::from_core)?;
    let ray = initialize_rectilinear_ray(&scene.kerr, &obs, &scene.camera, sensor)
        .map_err(IntegrationError::from_core)?;
    let y0 = GeodesicState::new(obs.event, ray.covariant_momentum)?;

    let disk = ThinDisk::new(scene.kerr, scene.disk)?;
    let hor = OuterHorizon::new(scene.kerr);
    let esc = EscapeSphere::new(scene.kerr, scene.escape_radius)?;
    let mut cfg = scene.integrator.clone();
    cfg.event_arming = scene.event_arming.clone();
    if !cfg.horizon_proximity.enabled {
        cfg.horizon_proximity = relativity_integrate::HorizonProximityPolicy::enabled(1e-4)?;
    }

    // Registration order must not affect earliest-λ selection (tested both orders).
    let report = match surface_set {
        TraceSurfaceSet::OpaqueDiskHorizonEscape => {
            let surfaces: [&dyn EventSurface; 3] = [&disk, &hor, &esc];
            integrate(scene.kerr, &y0, &cfg, &surfaces)
        }
        TraceSurfaceSet::HorizonEscapeOnly => {
            let surfaces: [&dyn EventSurface; 2] = [&hor, &esc];
            integrate(scene.kerr, &y0, &cfg, &surfaces)
        }
    };

    match report {
        Ok(report) => Ok(map_integration_report(report)),
        Err(e) => Ok(RayOutcome::Failed(RayFailure { error: e })),
    }
}

pub fn trace_ray_pixel(
    scene: &TraceScene,
    col: u32,
    row: u32,
) -> Result<RayOutcome, IntegrationError> {
    trace_ray_pixel_with_surface_set(scene, col, row, TraceSurfaceSet::OpaqueDiskHorizonEscape)
}

pub fn trace_ray_pixel_with_surface_set(
    scene: &TraceScene,
    col: u32,
    row: u32,
    surface_set: TraceSurfaceSet,
) -> Result<RayOutcome, IntegrationError> {
    let sensor = sensor_at_pixel_center(scene.grid, col, row);
    trace_ray_sensor_with_surface_set(scene, sensor, surface_set)
}

#[derive(Debug, Clone)]
/// Ordered camera-grid trace frame (row-major).
///
/// Contains traced physical/numerical outcomes.
/// It does not contain display colors.
/// It may be shaded repeatedly without retracing.
pub struct TraceBundle {
    pub grid: TraceGrid,
    pub outcomes: Vec<RayOutcome>,
}

impl TraceBundle {
    pub fn outcome_at(&self, col: u32, row: u32) -> &RayOutcome {
        &self.outcomes[pixel_index(self.grid, col, row)]
    }
}

/// Serial deterministic camera-grid trace (one sample per pixel center).
pub fn trace_grid(scene: &TraceScene) -> Result<TraceBundle, IntegrationError> {
    trace_grid_with_execution(scene, TraceExecution::Serial)
}

/// Camera-grid trace with explicit serial or bounded-parallel execution.
///
/// Delegates to [`TraceSurfaceSet::OpaqueDiskHorizonEscape`].
pub fn trace_grid_with_execution(
    scene: &TraceScene,
    execution: TraceExecution,
) -> Result<TraceBundle, IntegrationError> {
    trace_grid_with_execution_and_surface_set(
        scene,
        execution,
        TraceSurfaceSet::OpaqueDiskHorizonEscape,
    )
}

/// Camera-grid trace with explicit surface-set registration.
pub fn trace_grid_with_execution_and_surface_set(
    scene: &TraceScene,
    execution: TraceExecution,
    surface_set: TraceSurfaceSet,
) -> Result<TraceBundle, IntegrationError> {
    scene.validate()?;
    match execution {
        TraceExecution::Serial => trace_grid_serial(scene, surface_set),
        TraceExecution::Parallel { threads } => {
            trace_grid_parallel_indexed(scene, threads.get(), surface_set)
        }
    }
}

fn trace_grid_serial(
    scene: &TraceScene,
    surface_set: TraceSurfaceSet,
) -> Result<TraceBundle, IntegrationError> {
    let n = scene.grid.pixel_count();
    let mut outcomes = Vec::with_capacity(n);
    for row in 0..scene.grid.height {
        for col in 0..scene.grid.width {
            outcomes.push(trace_ray_pixel_with_surface_set(
                scene,
                col,
                row,
                surface_set,
            )?);
        }
    }
    Ok(TraceBundle {
        grid: scene.grid,
        outcomes,
    })
}

fn trace_grid_parallel_indexed(
    scene: &TraceScene,
    threads: usize,
    surface_set: TraceSurfaceSet,
) -> Result<TraceBundle, IntegrationError> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| IntegrationError::Solver {
            detail: format!("local rayon thread pool build failed: {e}"),
        })?;

    let width = scene.grid.width as usize;
    let n = scene.grid.pixel_count();
    let results: Vec<Result<RayOutcome, IntegrationError>> = pool.install(|| {
        (0..n)
            .into_par_iter()
            .map(|index| {
                let row = (index / width) as u32;
                let col = (index % width) as u32;
                trace_ray_pixel_with_surface_set(scene, col, row, surface_set)
            })
            .collect()
    });
    fold_indexed_results(scene.grid, results)
}

/// Deterministic reduction: first top-level error by ascending pixel index.
pub fn fold_indexed_results(
    grid: TraceGrid,
    results: Vec<Result<RayOutcome, IntegrationError>>,
) -> Result<TraceBundle, IntegrationError> {
    if results.len() != grid.pixel_count() {
        return Err(IntegrationError::InvalidConfig {
            field: "indexed_results_len",
        });
    }
    let mut outcomes = Vec::with_capacity(results.len());
    for item in results {
        match item {
            Ok(o) => outcomes.push(o),
            Err(e) => return Err(e),
        }
    }
    Ok(TraceBundle { grid, outcomes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_core::{CameraParams, KerrParams, PositionBl};
    use relativity_integrate::{Dop853Config, EventArmingPolicy, HorizonProximityPolicy};
    use std::num::NonZeroUsize;

    use crate::disk::ThinDiskGeometry;
    use crate::outcome::OutcomeClass;

    fn tiny_scene(w: u32, h: u32) -> TraceScene {
        let kerr = KerrParams::new(1.0, 0.0).unwrap();
        let disk = ThinDiskGeometry::new(3.0, 20.0);
        let mut integrator = Dop853Config::diagnostic_default();
        integrator.relative_tolerance = [1e-8; 8];
        integrator.absolute_tolerance = [1e-9, 1e-9, 1e-9, 1e-9, 1e-10, 1e-10, 1e-10, 1e-10];
        integrator.affine_limit = 40.0;
        integrator.max_step = 2.0;
        integrator.max_accepted_steps = 500;
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
    fn serial_trace_grid_is_row_major() {
        let scene = tiny_scene(3, 2);
        let bundle = trace_grid(&scene).unwrap();
        assert_eq!(bundle.outcomes.len(), 6);
        for row in 0..2 {
            for col in 0..3 {
                let idx = pixel_index(scene.grid, col, row);
                assert!(std::ptr::eq(
                    &bundle.outcomes[idx],
                    bundle.outcome_at(col, row)
                ));
            }
        }
    }

    #[test]
    fn default_apis_equal_explicit_opaque_surface_set() {
        let scene = tiny_scene(4, 3);
        let a = trace_grid(&scene).unwrap();
        let b = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Serial,
            TraceSurfaceSet::OpaqueDiskHorizonEscape,
        )
        .unwrap();
        assert_eq!(
            crate::diagnostics::outcome_class_bytes(&a),
            crate::diagnostics::outcome_class_bytes(&b)
        );
        let c = trace_grid_with_execution(&scene, TraceExecution::Serial).unwrap();
        assert_eq!(
            crate::diagnostics::outcome_class_bytes(&a),
            crate::diagnostics::outcome_class_bytes(&c)
        );
    }

    #[test]
    fn horizon_escape_only_has_no_disk_hits() {
        let scene = tiny_scene(6, 6);
        let bundle = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Serial,
            TraceSurfaceSet::HorizonEscapeOnly,
        )
        .unwrap();
        assert!(!bundle
            .outcomes
            .iter()
            .any(|o| o.class() == OutcomeClass::DiskHit));
    }

    #[test]
    fn horizon_escape_only_serial_equals_parallel() {
        let scene = tiny_scene(5, 4);
        let serial = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Serial,
            TraceSurfaceSet::HorizonEscapeOnly,
        )
        .unwrap();
        let parallel = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Parallel {
                threads: NonZeroUsize::new(2).unwrap(),
            },
            TraceSurfaceSet::HorizonEscapeOnly,
        )
        .unwrap();
        assert_eq!(
            crate::diagnostics::outcome_class_bytes(&serial),
            crate::diagnostics::outcome_class_bytes(&parallel)
        );
    }

    #[test]
    fn surface_set_change_alters_class_digest() {
        let scene = tiny_scene(6, 6);
        let opaque = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Serial,
            TraceSurfaceSet::OpaqueDiskHorizonEscape,
        )
        .unwrap();
        let omitted = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Serial,
            TraceSurfaceSet::HorizonEscapeOnly,
        )
        .unwrap();
        assert_ne!(
            crate::diagnostics::outcome_class_bytes(&opaque),
            crate::diagnostics::outcome_class_bytes(&omitted)
        );
    }

    #[test]
    fn parallel_matches_serial_for_thread_counts() {
        let scene = tiny_scene(4, 4);
        let serial = trace_grid(&scene).unwrap();
        for threads in [1usize, 2, 4] {
            let parallel = trace_grid_with_execution(
                &scene,
                TraceExecution::Parallel {
                    threads: NonZeroUsize::new(threads).unwrap(),
                },
            )
            .unwrap();
            assert_eq!(
                crate::diagnostics::outcome_class_bytes(&serial),
                crate::diagnostics::outcome_class_bytes(&parallel)
            );
            assert_eq!(serial.outcomes.len(), parallel.outcomes.len());
            for (a, b) in serial.outcomes.iter().zip(parallel.outcomes.iter()) {
                assert_eq!(a.class(), b.class());
            }
        }
    }

    #[test]
    fn fold_indexed_results_selects_lowest_pixel_index_error() {
        let grid = TraceGrid {
            width: 3,
            height: 2,
        };
        let ok = Ok(RayOutcome::Failed(RayFailure {
            error: IntegrationError::MissingEventOutcome,
        }));
        let err_early = Err(IntegrationError::InvalidConfig { field: "early" });
        let err_late = Err(IntegrationError::InvalidConfig { field: "late" });
        let results = vec![
            ok.clone(),
            ok.clone(),
            err_early.clone(),
            ok.clone(),
            err_late,
            ok,
        ];
        let err = fold_indexed_results(grid, results).unwrap_err();
        assert_eq!(err, IntegrationError::InvalidConfig { field: "early" });
    }

    #[test]
    fn parallel_repeated_runs_equal() {
        let scene = tiny_scene(4, 3);
        let a = trace_grid_with_execution(
            &scene,
            TraceExecution::Parallel {
                threads: NonZeroUsize::new(2).unwrap(),
            },
        )
        .unwrap();
        let b = trace_grid_with_execution(
            &scene,
            TraceExecution::Parallel {
                threads: NonZeroUsize::new(2).unwrap(),
            },
        )
        .unwrap();
        assert_eq!(
            crate::diagnostics::outcome_class_bytes(&a),
            crate::diagnostics::outcome_class_bytes(&b)
        );
        assert!(!a
            .outcomes
            .iter()
            .any(|o| { !matches!(o.class(), OutcomeClass::Failed) && !o.state_finite() }));
    }

    #[test]
    fn horizon_escape_only_repeated_runs_exact() {
        let scene = tiny_scene(4, 4);
        let a = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Serial,
            TraceSurfaceSet::HorizonEscapeOnly,
        )
        .unwrap();
        let b = trace_grid_with_execution_and_surface_set(
            &scene,
            TraceExecution::Serial,
            TraceSurfaceSet::HorizonEscapeOnly,
        )
        .unwrap();
        assert_eq!(
            crate::diagnostics::outcome_class_bytes(&a),
            crate::diagnostics::outcome_class_bytes(&b)
        );
        assert_eq!(
            crate::trace_digest::trace_data_digest(&a),
            crate::trace_digest::trace_data_digest(&b)
        );
    }
}
