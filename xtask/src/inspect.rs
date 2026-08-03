//! Deterministic inspect-point and inspect-initial-ray commands.

use crate::preset::load_preset;
use relativity_core::{
    evaluate_hamiltonian, evaluate_kerr_schild, identity_residual, initialize_rectilinear_ray,
    inverse_metric_spatial_derivatives, matrix_inverse_oracle, zamo_observer, CameraParams,
    KerrParams, PositionBl, PositionKs, SensorCoord,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct PointReport {
    mass: f64,
    spin: f64,
    position: [f64; 3],
    oblate_radius: f64,
    used_direct_radius_branch: bool,
    outer_horizon_radius: f64,
    h_scalar: f64,
    metric: [[f64; 4]; 4],
    inverse_metric: [[f64; 4]; 4],
    inverse_identity_residual: f64,
    matrix_oracle_max_abs_diff: f64,
    derivative_fd_max_abs_diff: f64,
    domain_status: String,
    conditioning_notes: Vec<String>,
}

pub fn inspect_point(
    mass: f64,
    spin: f64,
    x: f64,
    y: f64,
    z: f64,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = KerrParams::new(mass, spin)?;
    let pos = PositionKs::spatial(x, y, z);
    let mut notes = Vec::new();
    let geo = evaluate_kerr_schild(&params, &pos)?;
    let id = identity_residual(&geo.metric, &geo.inverse_metric);
    let oracle = matrix_inverse_oracle(&geo.metric)?;
    let mut oracle_diff = 0.0_f64;
    for i in 0..4 {
        for j in 0..4 {
            oracle_diff =
                oracle_diff.max((geo.inverse_metric.get(i, j) - oracle.inverse.get(i, j)).abs());
        }
    }
    notes.push(format!(
        "raw inverse asymmetry {:.3e}",
        oracle.raw_asymmetry
    ));
    let analytic = inverse_metric_spatial_derivatives(&params, &pos)?;
    let fd_diff = fd_max_diff(&params, &pos, &analytic)?;
    if !geo.radius.used_direct_branch {
        notes.push("used stable A<0 oblate-radius branch".into());
    }
    if geo.radius.r < params.outer_horizon_radius() {
        notes.push("inside outer horizon (KS chart)".into());
    }

    let report = PointReport {
        mass,
        spin,
        position: [x, y, z],
        oblate_radius: geo.radius.r,
        used_direct_radius_branch: geo.radius.used_direct_branch,
        outer_horizon_radius: params.outer_horizon_radius(),
        h_scalar: geo.h,
        metric: geo.metric.components(),
        inverse_metric: geo.inverse_metric.components(),
        inverse_identity_residual: id,
        matrix_oracle_max_abs_diff: oracle_diff,
        derivative_fd_max_abs_diff: fd_diff,
        domain_status: "ok".into(),
        conditioning_notes: notes,
    };

    emit(&report, format)
}

pub(crate) fn fd_partial_public(
    params: &KerrParams,
    pos: &PositionKs,
    axis: usize,
) -> Result<[[f64; 4]; 4], Box<dyn std::error::Error>> {
    let r = evaluate_kerr_schild(params, pos)?.radius.r;
    let coord = [pos.x, pos.y, pos.z][axis];
    let scale = coord.abs().max(r).max(1e-16);
    let h = (1e-6 * scale).clamp(1e-14, 1e-4);
    let mut plus = *pos;
    let mut minus = *pos;
    match axis {
        0 => {
            plus.x += h;
            minus.x -= h;
        }
        1 => {
            plus.y += h;
            minus.y -= h;
        }
        2 => {
            plus.z += h;
            minus.z -= h;
        }
        _ => return Err("bad axis".into()),
    }
    let gp = evaluate_kerr_schild(params, &plus)?.inverse_metric;
    let gm = evaluate_kerr_schild(params, &minus)?.inverse_metric;
    let mut out = [[0.0; 4]; 4];
    for a in 0..4 {
        for b in 0..4 {
            out[a][b] = (gp.get(a, b) - gm.get(a, b)) / (2.0 * h);
        }
    }
    Ok(out)
}

fn fd_max_diff(
    params: &KerrParams,
    pos: &PositionKs,
    analytic: &relativity_core::InverseMetricDerivatives,
) -> Result<f64, Box<dyn std::error::Error>> {
    let mut max = 0.0_f64;
    for axis in 0..3 {
        let fd = fd_partial_public(params, pos, axis)?;
        for a in 0..4 {
            for b in 0..4 {
                max = max.max((analytic.spatial[axis][a][b] - fd[a][b]).abs());
            }
        }
    }
    Ok(max)
}

#[derive(Debug, Serialize)]
struct RayReport {
    observer_event: [f64; 4],
    four_velocity: [f64; 4],
    tetrad: [[f64; 4]; 4],
    local_past_null: [f64; 4],
    chart_wave_vector: [f64; 4],
    covariant_momentum: [f64; 4],
    future_momentum: [f64; 4],
    hamiltonian: f64,
    time_orientation_local_past: f64,
    future_energy_like: f64,
    orthonormality_max_abs: f64,
    null_residual_chart: f64,
    null_residual_local: f64,
    dp_t_dlambda: f64,
}

pub fn inspect_initial_ray(
    preset_path: &str,
    sensor_x: f64,
    sensor_y: f64,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let preset = load_preset(Path::new(preset_path))?;
    let mass = preset.spacetime.mass;
    let spin = preset.spacetime.spin_a_over_m * mass;
    let params = KerrParams::new(mass, spin)?;
    if preset.observer.motion != "zamo" {
        return Err("Gate 1A inspect-initial-ray supports observer.motion = \"zamo\" only".into());
    }
    let bl = PositionBl::new(
        0.0,
        preset.observer.boyer_lindquist_r,
        preset.observer.boyer_lindquist_theta_degrees.to_radians(),
        preset.observer.boyer_lindquist_phi_degrees.to_radians(),
    );
    let obs = zamo_observer(&params, &bl)?;
    let g = evaluate_kerr_schild(&params, &obs.event)?.metric;
    let mut ortho = 0.0_f64;
    let eta = [
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for a in 0..4 {
        for b in 0..4 {
            ortho =
                ortho.max((g.contract(&obs.tetrad.legs[a], &obs.tetrad.legs[b]) - eta[a][b]).abs());
        }
    }
    let cam = CameraParams {
        horizontal_fov: preset.camera.horizontal_field_of_view_degrees.to_radians(),
        roll: preset.camera.roll_degrees.to_radians(),
    };
    let ray = initialize_rectilinear_ray(
        &params,
        &obs,
        &cam,
        SensorCoord {
            x: sensor_x,
            y: sensor_y,
        },
    )?;
    let ham = evaluate_hamiltonian(&params, &obs.event, &ray.covariant_momentum)?;

    let report = RayReport {
        observer_event: obs.event.components(),
        four_velocity: obs.four_velocity.components(),
        tetrad: [
            obs.tetrad.legs[0].components(),
            obs.tetrad.legs[1].components(),
            obs.tetrad.legs[2].components(),
            obs.tetrad.legs[3].components(),
        ],
        local_past_null: ray.local_past_null.components(),
        chart_wave_vector: ray.chart_wave_vector.components(),
        covariant_momentum: ray.covariant_momentum.components(),
        future_momentum: ray.future_momentum.components(),
        hamiltonian: ray.hamiltonian.h,
        time_orientation_local_past: ray.past_time_component_local,
        future_energy_like: ray.future_energy_like,
        orthonormality_max_abs: ortho,
        null_residual_chart: ray.chart_null_residual,
        null_residual_local: ray.local_null_residual,
        dp_t_dlambda: ham.dp_dlambda.t,
    };
    emit(&report, format)
}

fn emit<T: Serialize + std::fmt::Debug>(
    report: &T,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(report)?);
        }
        "text" | "human" => {
            println!("{report:#?}");
        }
        other => return Err(format!("unknown format {other}; use text|json").into()),
    }
    Ok(())
}
