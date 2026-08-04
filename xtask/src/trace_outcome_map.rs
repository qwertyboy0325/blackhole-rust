//! Generate Gate 1B2 categorical outcome map (PPM) + cost PGM + JSON.

use crate::build_meta::{
    require_release_execution, write_build_execution_report, BuildExecutionMetadata,
};
use crate::preset::load_preset;
use relativity_core::{CameraParams, KerrParams, PositionBl};
use relativity_integrate::{Dop853Config, EventArmingPolicy, HorizonProximityPolicy};
use relativity_trace::{
    build_outcome_map_report, trace_grid, write_outcome_ppm, write_rhs_pgm, ThinDiskGeometry,
    TraceGrid, TraceScene,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn run(
    preset_path: &str,
    width: u32,
    height: u32,
    output_ppm: &str,
    require_release: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let build = BuildExecutionMetadata::current();
    if require_release {
        // Fail before any tracing or artifact writes.
        require_release_execution(&build)?;
    }

    let root = workspace_root()?;
    let preset_full = if Path::new(preset_path).is_absolute() {
        PathBuf::from(preset_path)
    } else {
        root.join(preset_path)
    };
    let preset_bytes = std::fs::read(&preset_full)?;
    let preset_digest = hex::encode(Sha256::digest(&preset_bytes));
    let preset = load_preset(&preset_full)?;

    let mass = preset.spacetime.mass;
    let spin = preset.spacetime.spin_a_over_m * mass;
    let kerr = KerrParams::new(mass, spin)?;
    let r_plus = kerr.outer_horizon_radius();
    // Geometric scene radii — not an ISCO model (preset inner_edge string ignored).
    let disk = ThinDiskGeometry::new((r_plus + 1.5).max(3.0 * mass), preset.disk.outer_radius_m);
    disk.validate(&kerr)?;

    let mut integrator = Dop853Config::diagnostic_default();
    integrator.relative_tolerance = [1e-8; 8];
    integrator.absolute_tolerance = [1e-9, 1e-9, 1e-9, 1e-9, 1e-10, 1e-10, 1e-10, 1e-10];
    integrator.affine_limit = 120.0;
    integrator.max_accepted_steps = 2_000;
    integrator.max_step = 2.0;
    integrator.horizon_proximity = HorizonProximityPolicy::enabled(1e-4)?;
    integrator.event_arming = EventArmingPolicy::after(1e-12)?;

    let scene = TraceScene {
        kerr,
        observer: PositionBl::new(
            0.0,
            preset.observer.boyer_lindquist_r,
            preset.observer.boyer_lindquist_theta_degrees.to_radians(),
            preset.observer.boyer_lindquist_phi_degrees.to_radians(),
        ),
        camera: CameraParams {
            horizontal_fov: preset.camera.horizontal_field_of_view_degrees.to_radians(),
            roll: preset.camera.roll_degrees.to_radians(),
        },
        disk,
        // Cap for Gate 1B2 preview so outward rays terminate as Escaped.
        escape_radius: preset.celestial_sphere.radius_m.min(80.0),
        event_arming: integrator.event_arming.clone(),
        integrator,
        grid: TraceGrid { width, height },
    };
    scene.validate()?;

    let t0 = Instant::now();
    let bundle = trace_grid(&scene)?;
    let elapsed = t0.elapsed().as_secs_f64();

    for o in &bundle.outcomes {
        if !matches!(o, relativity_trace::RayOutcome::Failed(_)) && !o.state_finite() {
            return Err("non-finite success state in outcome map".into());
        }
    }

    let ppm = write_outcome_ppm(&bundle);
    let pgm = write_rhs_pgm(&bundle);

    let out_ppm = if Path::new(output_ppm).is_absolute() {
        PathBuf::from(output_ppm)
    } else {
        root.join(output_ppm)
    };
    let out_dir = out_ppm.parent().unwrap_or(Path::new(".")).to_path_buf();
    std::fs::create_dir_all(&out_dir)?;
    let out_pgm = out_dir.join("rhs-evaluations.pgm");
    let out_json = out_dir.join("outcome-map.json");

    std::fs::write(&out_ppm, &ppm)?;
    std::fs::write(&out_pgm, &pgm)?;

    let commit = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());
    let toolchain = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let target = std::env::var("TARGET").unwrap_or_else(|_| {
        std::process::Command::new("rustc")
            .args(["--print", "host-tuple"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".into())
    });

    let report = build_outcome_map_report(
        &bundle,
        &ppm,
        &pgm,
        &preset_digest,
        commit.trim(),
        &toolchain,
        &target,
        Some(elapsed),
    );
    std::fs::write(&out_json, serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(
        out_dir.join("outcome-map.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;
    // Adjacent worker report: compile-time metadata of *this* binary (not inferred by parent).
    write_build_execution_report(&out_dir, &build)?;

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err("git failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
