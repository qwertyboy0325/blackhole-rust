//! Single-ray integration diagnostic (no image).

use crate::preset::load_preset;
use relativity_core::{
    initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams, PositionBl, SensorCoord,
};
use relativity_integrate::{
    integrate, Dop853Config, EscapeSphere, EventSurface, GeodesicState, HorizonProximityPolicy,
    OuterHorizon,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
struct IntegrateRayReport {
    gate: &'static str,
    commit: String,
    toolchain: String,
    target: String,
    preset_path: String,
    preset_sha256: String,
    sensor: [f64; 2],
    initial_state: [f64; 8],
    config: Dop853Config,
    outcome: relativity_integrate::IntegrationOutcome,
    diagnostics: relativity_integrate::InvariantDiagnostics,
}

pub fn run(
    preset_path: &str,
    sensor_x: f64,
    sensor_y: f64,
    affine_limit: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let preset_full = if Path::new(preset_path).is_absolute() {
        PathBuf::from(preset_path)
    } else {
        root.join(preset_path)
    };
    let preset_bytes = std::fs::read(&preset_full)?;
    let preset_sha = hex::encode(Sha256::digest(&preset_bytes));
    let preset = load_preset(&preset_full)?;

    let mass = preset.spacetime.mass;
    let spin = preset.spacetime.spin_a_over_m * mass;
    let params = KerrParams::new(mass, spin)?;
    let bl = PositionBl::new(
        0.0,
        preset.observer.boyer_lindquist_r,
        preset.observer.boyer_lindquist_theta_degrees.to_radians(),
        preset.observer.boyer_lindquist_phi_degrees.to_radians(),
    );
    let obs = zamo_observer(&params, &bl)?;
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
    let y0 = GeodesicState::new(obs.event, ray.covariant_momentum)?;

    let mut cfg = Dop853Config::diagnostic_default();
    cfg.relative_tolerance = [preset.geodesics.relative_tolerance; 8];
    cfg.absolute_tolerance = [
        preset.geodesics.absolute_tolerance_position,
        preset.geodesics.absolute_tolerance_position,
        preset.geodesics.absolute_tolerance_position,
        preset.geodesics.absolute_tolerance_position,
        preset.geodesics.absolute_tolerance_momentum,
        preset.geodesics.absolute_tolerance_momentum,
        preset.geodesics.absolute_tolerance_momentum,
        preset.geodesics.absolute_tolerance_momentum,
    ];
    cfg.affine_limit = affine_limit;
    cfg.max_accepted_steps = preset.geodesics.maximum_steps;
    cfg.horizon_proximity = HorizonProximityPolicy::enabled(1e-10)?;

    let r_escape = preset.celestial_sphere.radius_m;
    if !(r_escape > bl.r) {
        return Err("celestial_sphere.radius_m must be strictly outside observer r".into());
    }
    let hor = OuterHorizon::new(params);
    let esc = EscapeSphere::new(params, r_escape)?;
    let surfaces: [&dyn EventSurface; 2] = [&hor, &esc];
    let report = integrate(params, &y0, &cfg, &surfaces)?;

    let commit = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());
    let toolchain = Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let target = std::env::var("TARGET").unwrap_or_else(|_| default_target());

    let out = IntegrateRayReport {
        gate: "gate-1b1",
        commit: commit.trim().into(),
        toolchain,
        target,
        preset_path: preset_path.into(),
        preset_sha256: preset_sha,
        sensor: [sensor_x, sensor_y],
        initial_state: y0.to_array(),
        config: cfg,
        outcome: report.outcome,
        diagnostics: report.diagnostics,
    };
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    if !out.status.success() {
        return Err("git failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn default_target() -> String {
    Command::new("rustc")
        .args(["--print", "host-tuple"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_else(|| "unknown".into())
}
