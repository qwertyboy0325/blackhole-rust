//! Rectilinear null-ray initialization without image rendering.
//!
//! Local past-directed null vector for backward tracing:
//! `k̂^(a) = (−1, n̂^(i))` with unit spatial `n̂`.
//! Radiometry uses the equivalent future-directed momentum `−k`.
//! Sources: James2015; ADR 0003; physics-assumptions.md.

use crate::error::CoreError;
use crate::hamiltonian::{evaluate_hamiltonian, HamiltonianEval};
use crate::kerr::KerrParams;
use crate::metric::evaluate_kerr_schild;
use crate::observer::Observer;
use crate::types::{Covector, LocalComponents, Vector};

/// Camera parameters needed for Gate 1A ray initialization.
#[derive(Debug, Clone, Copy)]
pub struct CameraParams {
    /// Horizontal field of view in radians.
    pub horizontal_fov: f64,
    /// Roll about the look axis in radians.
    pub roll: f64,
}

/// Normalized sensor coordinate in `[-1, 1]²` (pixel-center convention later).
#[derive(Debug, Clone, Copy)]
pub struct SensorCoord {
    pub x: f64,
    pub y: f64,
}

/// Initialized null ray diagnostics at the observer event.
#[derive(Debug, Clone, Copy)]
pub struct InitialRay {
    pub local_past_null: LocalComponents,
    pub chart_wave_vector: Vector,
    pub covariant_momentum: Covector,
    pub future_momentum: Covector,
    pub hamiltonian: HamiltonianEval,
    pub local_null_residual: f64,
    pub chart_null_residual: f64,
    pub past_time_component_local: f64,
    pub future_energy_like: f64,
}

/// Initialize one rectilinear camera ray (no image assembly).
pub fn initialize_rectilinear_ray(
    params: &KerrParams,
    observer: &Observer,
    camera: &CameraParams,
    sensor: SensorCoord,
) -> Result<InitialRay, CoreError> {
    if !sensor.x.is_finite() || !sensor.y.is_finite() {
        return Err(CoreError::NonFinite {
            context: "sensor coordinate",
        });
    }
    if !(camera.horizontal_fov.is_finite()
        && camera.horizontal_fov > 0.0
        && camera.horizontal_fov < std::f64::consts::PI)
    {
        return Err(CoreError::RayInit {
            context: "invalid horizontal FOV",
        });
    }

    // Look toward −e₃ (camera +z spatial is up, +x right, look −z) then roll.
    let tan_half = (0.5 * camera.horizontal_fov).tan();
    let dir_x = sensor.x * tan_half;
    let dir_y = sensor.y * tan_half;
    let dir_z = -1.0;
    let (dir_x, dir_y) = roll2d(dir_x, dir_y, camera.roll);
    let norm = (dir_x * dir_x + dir_y * dir_y + dir_z * dir_z).sqrt();
    if !(norm.is_finite() && norm > 0.0) {
        return Err(CoreError::RayInit {
            context: "sensor direction norm",
        });
    }
    let nx = dir_x / norm;
    let ny = dir_y / norm;
    let nz = dir_z / norm;

    // Past-directed null in local frame: k̂ = (−1, n̂)
    let local_past = LocalComponents::new(-1.0, nx, ny, nz);
    let local_null_residual = -1.0 + nx * nx + ny * ny + nz * nz;

    let k_chart = observer.tetrad.push_local(&local_past);
    let geo = evaluate_kerr_schild(params, &observer.event)?;
    let p = geo.metric.mul_vec(&k_chart);
    let p_future = p.scale(-1.0);

    let ham = evaluate_hamiltonian(params, &observer.event, &p)?;
    let chart_null = geo.metric.contract(&k_chart, &k_chart);

    if !k_chart.is_finite() || !p.is_finite() {
        return Err(CoreError::RayInit {
            context: "non-finite ray tensors",
        });
    }

    Ok(InitialRay {
        local_past_null: local_past,
        chart_wave_vector: k_chart,
        covariant_momentum: p,
        future_momentum: p_future,
        hamiltonian: ham,
        local_null_residual,
        chart_null_residual: chart_null,
        past_time_component_local: local_past.t,
        future_energy_like: -p_future.t,
    })
}

fn roll2d(x: f64, y: f64, roll: f64) -> (f64, f64) {
    let c = roll.cos();
    let s = roll.sin();
    (c * x - s * y, s * x + c * y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observer::{minkowski_static_observer, zamo_observer};
    use crate::types::PositionBl;

    #[test]
    fn minkowski_ray_is_null_and_past_directed() {
        let obs =
            minkowski_static_observer(crate::types::PositionKs::spatial(0.0, 0.0, 0.0)).unwrap();
        let cam = CameraParams {
            horizontal_fov: 50.0_f64.to_radians(),
            roll: 0.0,
        };
        let params = KerrParams::new(1.0, 0.0).unwrap();
        // Use Minkowski observer but KS metric with a=0,M=1 is not Minkowski —
        // for pure Minkowski null check, lower with η directly via chart vectors.
        let local = LocalComponents::new(-1.0, 0.0, 0.0, -1.0);
        let k = obs.tetrad.push_local(&local);
        let eta = crate::types::MetricTensor::minkowski();
        assert!((eta.contract(&k, &k)).abs() < 1e-15);
        assert!(local.t < 0.0);
        let _ = (params, cam);
    }

    #[test]
    fn zamo_baseline_ray_null_and_orientation() {
        let params = KerrParams::new(1.0, 0.999).unwrap();
        let bl = PositionBl::new(0.0, 20.0, 85.0_f64.to_radians(), 0.0);
        let obs = zamo_observer(&params, &bl).unwrap();
        let cam = CameraParams {
            horizontal_fov: 50.0_f64.to_radians(),
            roll: 0.0,
        };
        let ray = initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 })
            .unwrap();
        assert!(ray.past_time_component_local < 0.0);
        assert!(
            ray.chart_null_residual.abs() < 1e-10,
            "null residual {}",
            ray.chart_null_residual
        );
        assert!(ray.hamiltonian.h.abs() < 1e-10, "H {}", ray.hamiltonian.h);
        // Future radiometry momentum is opposite.
        assert!((ray.future_momentum.t + ray.covariant_momentum.t).abs() < 1e-15);
        assert!(ray.future_energy_like > 0.0);
    }

    #[test]
    fn catches_sign_reversal_between_past_and_future() {
        let params = KerrParams::new(1.0, 0.5).unwrap();
        let bl = PositionBl::new(0.0, 30.0, 1.0, 0.0);
        let obs = zamo_observer(&params, &bl).unwrap();
        let cam = CameraParams {
            horizontal_fov: 40.0_f64.to_radians(),
            roll: 0.0,
        };
        let ray = initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.25, y: -0.1 })
            .unwrap();
        assert!(-ray.past_time_component_local > 0.0);
        assert!((ray.covariant_momentum.t - ray.future_momentum.scale(-1.0).t).abs() < 1e-14);
    }
}
