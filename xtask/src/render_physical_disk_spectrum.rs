//! Gate 2C0: render physical thin-disk Page–Thorne + Planck spectral I_ν.

use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::diagnostic_scene::build_diagnostic_trace_scene;
use crate::preset::load_preset;
use crate::reference_pipeline::summarize_outcomes;
use crate::render_tier::{resolve_render_plan, DiagnosticRenderTier};
use crate::trace_outcome_map::{resolve_execution, CliExecution};
use relativity_render::DiskVelocityModel;
use relativity_render::{
    build_disk_frequency_shift_frame, build_physical_disk_emission_frame,
    build_physical_spectral_frame, compute_physical_spectral_closure, disk_frequency_shift_digest,
    parse_physical_spectral_grid_id, physical_disk_emission_digest,
    physical_disk_emission_spec_digest, physical_spectral_digest, physical_spectral_grid_digest,
    validate_physical_emission_provenance, verify_observer_unit_frequency,
    DiskFrequencyShiftConvention, PhysicalDiskEmissionConvention, PhysicalDiskEmissionPixel,
    PhysicalDiskEmissionSpec, PhysicalSpectralConvention, PhysicalSpectralPixel,
    OBSERVER_UNIT_FREQUENCY_TOLERANCE, PHYSICAL_EMISSION_MODEL_ID,
};
use relativity_trace::{trace_grid_with_execution_and_surface_set, TraceGrid, TraceSurfaceSet};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAGIC_FTEFF: &[u8; 8] = b"BHRFTEF1";
const MAGIC_INU: &[u8; 8] = b"BHRPHYI1";

#[derive(Serialize)]
struct PhysicalRenderReport {
    gate: &'static str,
    convention_id: String,
    emission_model_id: String,
    physical_spectral_grid_id: String,
    frequency_shift_digest: String,
    physical_emission_spec_digest: String,
    physical_emission_digest: String,
    physical_spectral_grid_digest: String,
    physical_spectral_digest: String,
    disk_hit_count: u64,
    emission_pixel_count: u64,
    closure: relativity_render::PhysicalSpectralClosureMetrics,
    build: BuildExecutionMetadata,
    trace_wall_clock_seconds: f64,
    channel_wall_clock_seconds: f64,
    spectral_wall_clock_seconds: f64,
    total_wall_clock_seconds: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    preset_path: &str,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
    physical_emission: &str,
    physical_spectral_grid: &str,
    output_dir: &str,
    require_release: bool,
    execution: CliExecution,
    threads: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let t0 = Instant::now();
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }

    if physical_emission != PHYSICAL_EMISSION_MODEL_ID {
        return Err(format!(
            "unsupported --physical-emission `{physical_emission}`; only `{PHYSICAL_EMISSION_MODEL_ID}` is authoritative"
        )
        .into());
    }

    let plan = resolve_render_plan(tier, width, height)?;
    let trace_execution = resolve_execution(execution, threads)?;
    let root = workspace_root()?;
    let out_dir = resolve_path(&root, output_dir);
    let preset_full = resolve_path(&root, preset_path);
    let preset = load_preset(&preset_full)?;
    validate_physical_emission_provenance(&preset.disk.emission_model, &preset.disk.emission_claim)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let physical = preset
        .physical
        .as_ref()
        .ok_or("preset missing [physical] section required for render-physical-disk-spectrum")?;
    if physical.emission_model != PHYSICAL_EMISSION_MODEL_ID {
        return Err("preset [physical].emission_model mismatch".into());
    }

    std::fs::create_dir_all(&out_dir)?;

    let (scene, _) = build_diagnostic_trace_scene(
        &preset,
        TraceGrid {
            width: plan.width,
            height: plan.height,
        },
    )?;
    let bounds = relativity_render::ResolvedDiskBounds::new(scene.disk.r_inner, scene.disk.r_outer)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    let emission_spec =
        PhysicalDiskEmissionSpec::page_thorne_blackbody_v1(physical.mass_solar, physical.mdot_kg_s)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    let t_trace = Instant::now();
    let bundle = trace_grid_with_execution_and_surface_set(
        &scene,
        trace_execution,
        TraceSurfaceSet::OpaqueDiskHorizonEscape,
    )?;
    let trace_wall = t_trace.elapsed().as_secs_f64();
    let counts = summarize_outcomes(&bundle);

    let t_channel = Instant::now();
    let verification = verify_observer_unit_frequency(&scene.kerr, &scene)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    if verification.maximum_residual > OBSERVER_UNIT_FREQUENCY_TOLERANCE {
        return Err("observer unit-frequency verification exceeded tolerance".into());
    }
    let frequency = build_disk_frequency_shift_frame(
        &scene.kerr,
        &bundle,
        DiskVelocityModel::ProgradeCircularGeodesic,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let freq_digest = disk_frequency_shift_digest(&frequency, &DiskFrequencyShiftConvention::v1());

    let emission =
        build_physical_disk_emission_frame(&scene.kerr, &frequency, &emission_spec, bounds)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let emission_spec_digest = physical_disk_emission_spec_digest(&emission_spec);
    let emission_digest = physical_disk_emission_digest(
        &emission,
        &PhysicalDiskEmissionConvention::v1(),
        &emission_spec,
        &freq_digest,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let channel_wall = t_channel.elapsed().as_secs_f64();

    let spectral_grid = parse_physical_spectral_grid_id(physical_spectral_grid)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    let t_spec = Instant::now();
    let spectral = build_physical_spectral_frame(&emission, &spectral_grid)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let spectral_wall = t_spec.elapsed().as_secs_f64();

    let convention = PhysicalSpectralConvention::v1();
    let grid_digest = physical_spectral_grid_digest(&spectral_grid)?;
    let spectral_digest = physical_spectral_digest(&spectral, &convention, &emission_digest)?;
    let closure = compute_physical_spectral_closure(&spectral)?;

    let emission_pixel_count = spectral
        .pixels
        .iter()
        .filter(|p| matches!(p, PhysicalSpectralPixel::DiskHit(_)))
        .count() as u64;

    write_emission_meta(
        &out_dir,
        &emission,
        &freq_digest,
        &emission_spec_digest,
        &emission_digest,
    )?;
    write_f_teff_payload(&out_dir, &emission)?;
    write_spectral_meta(
        &out_dir,
        &spectral,
        &freq_digest,
        &emission_digest,
        &grid_digest,
        &spectral_digest,
        &closure,
    )?;
    write_i_nu_payload(&out_dir, &spectral)?;
    write_diagnostic_pgms(&out_dir, &emission, &spectral)?;

    let report = PhysicalRenderReport {
        gate: "gate-2c0-physical-emission",
        convention_id: convention.convention_id,
        emission_model_id: PHYSICAL_EMISSION_MODEL_ID.into(),
        physical_spectral_grid_id: spectral_grid.grid_id().into(),
        frequency_shift_digest: freq_digest,
        physical_emission_spec_digest: emission_spec_digest,
        physical_emission_digest: emission_digest,
        physical_spectral_grid_digest: grid_digest,
        physical_spectral_digest: spectral_digest,
        disk_hit_count: counts.disk_hit,
        emission_pixel_count,
        closure,
        build,
        trace_wall_clock_seconds: trace_wall,
        channel_wall_clock_seconds: channel_wall,
        spectral_wall_clock_seconds: spectral_wall,
        total_wall_clock_seconds: t0.elapsed().as_secs_f64(),
    };
    std::fs::write(
        out_dir.join("physical-render-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "Gate 2C0 physical spectral render complete: spectral_digest={} emission_pixels={}",
        report.physical_spectral_digest, report.emission_pixel_count
    );
    Ok(())
}

fn write_emission_meta(
    out: &Path,
    frame: &relativity_render::PhysicalDiskEmissionFrame,
    freq: &str,
    spec: &str,
    emission: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let meta = serde_json::json!({
        "schema_version": 1,
        "frame": "PhysicalDiskEmissionFrame",
        "width": frame.grid.width,
        "height": frame.grid.height,
        "r_isco_over_m": frame.r_isco_over_m,
        "gravitational_radius_m": frame.gravitational_radius_m,
        "bounds_inner": frame.bounds.inner_radius(),
        "bounds_outer": frame.bounds.outer_radius(),
        "frequency_shift_digest": freq,
        "physical_emission_spec_digest": spec,
        "physical_emission_digest": emission,
        "payload": "physical-f-teff.f64le",
        "units": {
            "f_one_face": "W_m^-2",
            "t_eff": "K",
        },
        "colorimetry": "absent",
    });
    std::fs::write(
        out.join("physical-emission-meta.json"),
        serde_json::to_vec_pretty(&meta)?,
    )?;
    Ok(())
}

fn write_f_teff_payload(
    out: &Path,
    frame: &relativity_render::PhysicalDiskEmissionFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC_FTEFF);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&frame.grid.width.to_le_bytes());
    bytes.extend_from_slice(&frame.grid.height.to_le_bytes());
    for pixel in &frame.pixels {
        match pixel {
            PhysicalDiskEmissionPixel::DiskHit(s) => {
                bytes.push(1);
                bytes.extend_from_slice(&s.f_one_face_w_m2.to_bits().to_le_bytes());
                bytes.extend_from_slice(&s.t_eff_k.to_bits().to_le_bytes());
                bytes.extend_from_slice(&s.g_factor.to_bits().to_le_bytes());
                bytes.extend_from_slice(&s.radius_over_m.to_bits().to_le_bytes());
            }
            PhysicalDiskEmissionPixel::NotDiskHit { .. } => {
                bytes.push(0);
                for _ in 0..4 {
                    bytes.extend_from_slice(&0f64.to_bits().to_le_bytes());
                }
            }
        }
    }
    std::fs::write(out.join("physical-f-teff.f64le"), bytes)?;
    Ok(())
}

fn write_spectral_meta(
    out: &Path,
    frame: &relativity_render::PhysicalSpectralFrame,
    freq: &str,
    emission: &str,
    grid: &str,
    spectral: &str,
    closure: &relativity_render::PhysicalSpectralClosureMetrics,
) -> Result<(), Box<dyn std::error::Error>> {
    let meta = serde_json::json!({
        "schema_version": 1,
        "frame": "PhysicalSpectralFrame",
        "measure": "frequency-specific-intensity",
        "units": "W_m^-2_Hz^-1_sr^-1",
        "layout": "pixel-major",
        "width": frame.grid.width,
        "height": frame.grid.height,
        "n_bins": frame.spectral_grid.n_bins(),
        "nu_min_hz": frame.spectral_grid.nu_min(),
        "nu_max_hz": frame.spectral_grid.nu_max(),
        "grid_id": frame.spectral_grid.grid_id(),
        "frequency_shift_digest": freq,
        "physical_emission_digest": emission,
        "physical_spectral_grid_digest": grid,
        "physical_spectral_digest": spectral,
        "closure": closure,
        "payload": "physical-i-nu-obs.f64le",
        "colorimetry": "absent-deferred-to-gate-2c1",
        "note": "physical Hz authority; diagnostic spectral-grid-v1 is not Hz",
    });
    std::fs::write(
        out.join("physical-spectral-meta.json"),
        serde_json::to_vec_pretty(&meta)?,
    )?;
    Ok(())
}

fn write_i_nu_payload(
    out: &Path,
    frame: &relativity_render::PhysicalSpectralFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let n_bins = frame.spectral_grid.n_bins() as usize;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC_INU);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&frame.grid.width.to_le_bytes());
    bytes.extend_from_slice(&frame.grid.height.to_le_bytes());
    bytes.extend_from_slice(&(n_bins as u32).to_le_bytes());
    for pixel in &frame.pixels {
        match pixel {
            PhysicalSpectralPixel::DiskHit(s) => {
                bytes.push(1);
                for v in &s.i_nu_obs {
                    bytes.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            PhysicalSpectralPixel::NotDiskHit { .. } => {
                bytes.push(0);
                for _ in 0..n_bins {
                    bytes.extend_from_slice(&0f64.to_bits().to_le_bytes());
                }
            }
        }
    }
    std::fs::write(out.join("physical-i-nu-obs.f64le"), bytes)?;
    Ok(())
}

fn write_diagnostic_pgms(
    out: &Path,
    emission: &relativity_render::PhysicalDiskEmissionFrame,
    spectral: &relativity_render::PhysicalSpectralFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let w = emission.grid.width;
    let h = emission.grid.height;
    let mut teff = vec![0u8; (w * h) as usize];
    let mut flux = vec![0u8; (w * h) as usize];
    let mut integ = vec![0u8; (w * h) as usize];
    for (i, pixel) in emission.pixels.iter().enumerate() {
        if let PhysicalDiskEmissionPixel::DiskHit(s) = pixel {
            teff[i] = log_gray(s.t_eff_k);
            flux[i] = log_gray(s.f_one_face_w_m2);
        }
    }
    for (i, pixel) in spectral.pixels.iter().enumerate() {
        if let PhysicalSpectralPixel::DiskHit(s) = pixel {
            integ[i] = log_gray(s.integrated_observed_i_nu);
        }
    }
    write_pgm(out.join("teff-diagnostic.pgm"), w, h, &teff)?;
    write_pgm(out.join("flux-diagnostic.pgm"), w, h, &flux)?;
    write_pgm(out.join("observed-integral-diagnostic.pgm"), w, h, &integ)?;
    Ok(())
}

fn log_gray(v: f64) -> u8 {
    if !(v > 0.0) {
        return 0;
    }
    let x = (v.ln() + 40.0) / 40.0;
    (x.clamp(0.0, 1.0) * 255.0) as u8
}

fn write_pgm(path: PathBuf, w: u32, h: u32, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = format!("P5\n{w} {h}\n255\n").into_bytes();
    out.extend_from_slice(data);
    std::fs::write(path, out)?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if dir.ends_with("xtask") {
        dir.pop();
    }
    Ok(dir)
}

fn resolve_path(root: &Path, p: &str) -> PathBuf {
    let path = PathBuf::from(p);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
