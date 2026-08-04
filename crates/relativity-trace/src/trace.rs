//! Single-ray and camera-grid tracing.

use relativity_core::{initialize_rectilinear_ray, zamo_observer, SensorCoord};
use relativity_integrate::{
    integrate, EscapeSphere, EventSurface, GeodesicState, IntegrationError, OuterHorizon,
};

use crate::camera::{pixel_index, sensor_at_pixel_center, TraceGrid};
use crate::disk::ThinDisk;
use crate::outcome::{map_integration_report, RayFailure, RayOutcome};
use crate::scene::TraceScene;

pub fn trace_ray_sensor(
    scene: &TraceScene,
    sensor: SensorCoord,
) -> Result<RayOutcome, IntegrationError> {
    scene.validate()?;
    let obs = zamo_observer(&scene.kerr, &scene.observer).map_err(IntegrationError::from_core)?;
    let ray = initialize_rectilinear_ray(&scene.kerr, &obs, &scene.camera, sensor)
        .map_err(IntegrationError::from_core)?;
    let y0 = GeodesicState::new(obs.event, ray.covariant_momentum)?;

    let disk = ThinDisk::new(scene.kerr, scene.disk)?;
    let hor = OuterHorizon::new(scene.kerr);
    let esc = EscapeSphere::new(scene.kerr, scene.escape_radius)?;
    // Registration order must not affect earliest-λ selection (tested both orders).
    let surfaces: [&dyn EventSurface; 3] = [&disk, &hor, &esc];
    let mut cfg = scene.integrator.clone();
    cfg.event_arming = scene.event_arming.clone();
    if !cfg.horizon_proximity.enabled {
        // Gate 1B2 default: enable OuterHorizon approach capture.
        cfg.horizon_proximity = relativity_integrate::HorizonProximityPolicy::enabled(1e-4)?;
    }

    match integrate(scene.kerr, &y0, &cfg, &surfaces) {
        Ok(report) => Ok(map_integration_report(report)),
        Err(e) => Ok(RayOutcome::Failed(RayFailure { error: e })),
    }
}

pub fn trace_ray_pixel(
    scene: &TraceScene,
    col: u32,
    row: u32,
) -> Result<RayOutcome, IntegrationError> {
    let sensor = sensor_at_pixel_center(scene.grid, col, row);
    trace_ray_sensor(scene, sensor)
}

#[derive(Debug, Clone)]
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
    scene.validate()?;
    let n = scene.grid.pixel_count();
    let mut outcomes = Vec::with_capacity(n);
    for row in 0..scene.grid.height {
        for col in 0..scene.grid.width {
            outcomes.push(trace_ray_pixel(scene, col, row)?);
        }
    }
    Ok(TraceBundle {
        grid: scene.grid,
        outcomes,
    })
}
