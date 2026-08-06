//! Gate 2B2: render diagnostic disk spectral specific intensity.

use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::diagnostic_scene::build_diagnostic_trace_scene;
use crate::preset::load_preset;
use crate::reference_pipeline::summarize_outcomes;
use crate::render_tier::{resolve_render_plan, DiagnosticRenderTier};
use crate::trace_outcome_map::{resolve_execution, CliExecution};
use relativity_core::SpectralGrid;
use relativity_render::{
    build_disk_bolometric_frame, build_disk_frequency_shift_frame, build_disk_spectral_frame,
    compute_bolometric_closure, diagnostic_bolometric_emission_v1,
    diagnostic_lognormal_continuum_v1, diagnostic_spectrum_spec_digest, disk_bolometric_digest,
    disk_frequency_shift_digest, disk_spectral_digest, spectral_grid_digest,
    validate_disk_emission_provenance, verify_disk_bolometric_frame,
    verify_observer_unit_frequency, DiskBolometricConvention, DiskFrequencyShiftConvention,
    DiskSpectralConvention, DiskVelocityModel, ResolvedDiskBounds, SpectralPixel,
    CANONICAL_DISK_EMISSION_CLAIM, CANONICAL_DISK_EMISSION_MODEL, CONTINUUM_SPECTRUM_ID,
};
use relativity_trace::{trace_grid_with_execution_and_surface_set, TraceGrid, TraceSurfaceSet};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAGIC: &[u8; 8] = b"BHRSPEC1";

#[derive(Serialize)]
struct SpectralRenderReport {
    gate: &'static str,
    convention_id: String,
    continuum_spectrum_id: String,
    spectral_grid_id: String,
    frequency_shift_digest: String,
    bolometric_digest: String,
    continuum_digest: String,
    spectral_grid_digest: String,
    spectral_digest: String,
    disk_hit_count: u64,
    closure: relativity_render::SpectralClosureMetrics,
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
    spectrum_id: &str,
    spectral_grid_id: &str,
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

    if spectrum_id != CONTINUUM_SPECTRUM_ID {
        return Err(format!(
            "unsupported --spectrum `{spectrum_id}`; only `{CONTINUUM_SPECTRUM_ID}` is authoritative"
        )
        .into());
    }
    if spectral_grid_id != SpectralGrid::V1_ID
        && !spectral_grid_id.starts_with("spectral-grid-explore-")
    {
        return Err(format!(
            "unsupported --spectral-grid `{spectral_grid_id}`; use `{}` or `spectral-grid-explore-{{n}}`",
            SpectralGrid::V1_ID
        )
        .into());
    }

    let plan = resolve_render_plan(tier, width, height)?;
    let trace_execution = resolve_execution(execution, threads)?;
    let root = workspace_root()?;
    let out_dir = resolve_path(&root, output_dir);
    let preset_full = resolve_path(&root, preset_path);
    let preset = load_preset(&preset_full)?;
    validate_disk_emission_provenance(&preset.disk.emission_model, &preset.disk.emission_claim)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    std::fs::create_dir_all(&out_dir)?;

    let (scene, _) = build_diagnostic_trace_scene(
        &preset,
        TraceGrid {
            width: plan.width,
            height: plan.height,
        },
    )?;
    let bounds = ResolvedDiskBounds::new(scene.disk.r_inner, scene.disk.r_outer)
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
    if verification.maximum_residual > relativity_render::OBSERVER_UNIT_FREQUENCY_TOLERANCE {
        return Err("observer unit-frequency verification exceeded tolerance".into());
    }
    let frequency = build_disk_frequency_shift_frame(
        &scene.kerr,
        &bundle,
        DiskVelocityModel::ProgradeCircularGeodesic,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let freq_digest = disk_frequency_shift_digest(&frequency, &DiskFrequencyShiftConvention::v1());

    let emission_spec = diagnostic_bolometric_emission_v1();
    let bolometric = build_disk_bolometric_frame(&frequency, &emission_spec, bounds)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    verify_disk_bolometric_frame(&frequency, &bolometric, &emission_spec, bounds)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let bolo_digest = disk_bolometric_digest(
        &bolometric,
        &DiskBolometricConvention::v1(),
        &emission_spec,
        bounds,
        &freq_digest,
        CANONICAL_DISK_EMISSION_MODEL,
        CANONICAL_DISK_EMISSION_CLAIM,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let channel_wall = t_channel.elapsed().as_secs_f64();

    let continuum = diagnostic_lognormal_continuum_v1();
    let spectral_grid = if spectral_grid_id == SpectralGrid::V1_ID {
        SpectralGrid::spectral_grid_v1()?
    } else {
        let n: u32 = spectral_grid_id
            .strip_prefix("spectral-grid-explore-")
            .ok_or("bad explore grid id")?
            .parse()?;
        SpectralGrid::log_spaced(spectral_grid_id, continuum.nu_min, continuum.nu_max, n)?
    };

    let t_spec = Instant::now();
    let spectral =
        build_disk_spectral_frame(&frequency, &bolometric, &continuum, &spectral_grid, bounds)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let spectral_wall = t_spec.elapsed().as_secs_f64();

    let convention = DiskSpectralConvention::v1();
    let continuum_digest = diagnostic_spectrum_spec_digest(&continuum);
    let grid_digest = spectral_grid_digest(&spectral_grid)?;
    let spectral_digest = disk_spectral_digest(
        &spectral,
        &convention,
        &continuum,
        &freq_digest,
        &bolo_digest,
    )?;
    let closure = compute_bolometric_closure(&spectral)?;

    write_meta(
        &out_dir,
        &spectral,
        &freq_digest,
        &bolo_digest,
        &continuum_digest,
        &grid_digest,
        &spectral_digest,
        &closure,
    )?;
    write_i_nu_payload(&out_dir, &spectral)?;
    write_integral_pgms(&out_dir, &spectral)?;
    write_band_pgms(&out_dir, &spectral)?;
    write_selected_csv(&out_dir, &spectral)?;

    let report = SpectralRenderReport {
        gate: "gate-2b2-spectral-transport",
        convention_id: convention.convention_id,
        continuum_spectrum_id: CONTINUUM_SPECTRUM_ID.into(),
        spectral_grid_id: spectral_grid.grid_id().into(),
        frequency_shift_digest: freq_digest,
        bolometric_digest: bolo_digest,
        continuum_digest,
        spectral_grid_digest: grid_digest,
        spectral_digest,
        disk_hit_count: counts.disk_hit,
        closure,
        build,
        trace_wall_clock_seconds: trace_wall,
        channel_wall_clock_seconds: channel_wall,
        spectral_wall_clock_seconds: spectral_wall,
        total_wall_clock_seconds: t0.elapsed().as_secs_f64(),
    };
    std::fs::write(
        out_dir.join("spectral-render-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "Gate 2B2 spectral render complete: spectral_digest={} disk_hits={}",
        report.spectral_digest, report.disk_hit_count
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_meta(
    out: &Path,
    frame: &relativity_render::SpectralFrame,
    freq: &str,
    bolo: &str,
    continuum: &str,
    grid: &str,
    spectral: &str,
    closure: &relativity_render::SpectralClosureMetrics,
) -> Result<(), Box<dyn std::error::Error>> {
    let meta = serde_json::json!({
        "schema_version": 1,
        "measure": "frequency-specific-intensity",
        "layout": "pixel-major",
        "width": frame.grid().width,
        "height": frame.grid().height,
        "n_bins": frame.spectral_grid().n_bins(),
        "nu_min": frame.spectral_grid().nu_min(),
        "nu_max": frame.spectral_grid().nu_max(),
        "grid_id": frame.spectral_grid().grid_id(),
        "frequency_shift_digest": freq,
        "bolometric_digest": bolo,
        "continuum_digest": continuum,
        "spectral_grid_digest": grid,
        "spectral_digest": spectral,
        "closure": closure,
        "payload": "spectral-i-nu-obs.f64le",
        "colorimetry": "absent",
    });
    std::fs::write(
        out.join("spectral-frame-meta.json"),
        serde_json::to_vec_pretty(&meta)?,
    )?;
    Ok(())
}

fn write_i_nu_payload(
    out: &Path,
    frame: &relativity_render::SpectralFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let n_bins = frame.spectral_grid().n_bins() as usize;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&1u32.to_le_bytes()); // schema
    bytes.extend_from_slice(&frame.grid().width.to_le_bytes());
    bytes.extend_from_slice(&frame.grid().height.to_le_bytes());
    bytes.extend_from_slice(&(n_bins as u32).to_le_bytes());
    for pixel in frame.pixels() {
        match pixel {
            SpectralPixel::DiskHit(s) => {
                bytes.push(1);
                for v in &s.i_nu_obs {
                    bytes.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            SpectralPixel::NotDiskHit { .. } => {
                bytes.push(0);
                for _ in 0..n_bins {
                    bytes.extend_from_slice(&0f64.to_bits().to_le_bytes());
                }
            }
        }
    }
    std::fs::write(out.join("spectral-i-nu-obs.f64le"), bytes)?;
    Ok(())
}

fn write_integral_pgms(
    out: &Path,
    frame: &relativity_render::SpectralFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let w = frame.grid().width;
    let h = frame.grid().height;
    let mut emitted = vec![0u8; (w * h) as usize];
    let mut observed = vec![0u8; (w * h) as usize];
    let mut rel_err = vec![0u8; (w * h) as usize];
    for (i, pixel) in frame.pixels().iter().enumerate() {
        if let SpectralPixel::DiskHit(s) = pixel {
            emitted[i] = log_gray(s.integrated_emitted_i_nu);
            observed[i] = log_gray(s.integrated_observed_i_nu);
            let err = if s.observed_bolometric_intensity > 0.0 {
                (s.integrated_observed_i_nu - s.observed_bolometric_intensity).abs()
                    / s.observed_bolometric_intensity
            } else {
                0.0
            };
            rel_err[i] = ((err * 255.0).clamp(0.0, 255.0)) as u8;
        }
    }
    write_pgm(out.join("emitted-integral.pgm"), w, h, &emitted)?;
    write_pgm(out.join("observed-integral.pgm"), w, h, &observed)?;
    write_pgm(out.join("bolometric-relative-error.pgm"), w, h, &rel_err)?;
    Ok(())
}

fn write_band_pgms(
    out: &Path,
    frame: &relativity_render::SpectralFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let n = frame.spectral_grid().n_bins() as usize;
    let low = 0..n / 3;
    let mid = n / 3..(2 * n) / 3;
    let high = (2 * n) / 3..n;
    let w = frame.grid().width;
    let h = frame.grid().height;
    let mut lo = vec![0u8; (w * h) as usize];
    let mut mi = vec![0u8; (w * h) as usize];
    let mut hi = vec![0u8; (w * h) as usize];
    let weights = frame.spectral_grid().weights();
    for (i, pixel) in frame.pixels().iter().enumerate() {
        if let SpectralPixel::DiskHit(s) = pixel {
            lo[i] = log_gray(band_integral(&s.i_nu_obs, weights, low.clone()));
            mi[i] = log_gray(band_integral(&s.i_nu_obs, weights, mid.clone()));
            hi[i] = log_gray(band_integral(&s.i_nu_obs, weights, high.clone()));
        }
    }
    write_pgm(out.join("band-low.pgm"), w, h, &lo)?;
    write_pgm(out.join("band-mid.pgm"), w, h, &mi)?;
    write_pgm(out.join("band-high.pgm"), w, h, &hi)?;
    Ok(())
}

fn band_integral(samples: &[f64], weights: &[f64], range: std::ops::Range<usize>) -> f64 {
    let mut acc = 0.0;
    for i in range {
        acc += samples[i] * weights[i];
    }
    acc
}

fn write_selected_csv(
    out: &Path,
    frame: &relativity_render::SpectralFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    rows.push(
        "source_index,col,row,g,i_em_bol,i_obs_bol,integ_em,integ_obs,trunc_em_frac,trunc_obs_frac"
            .into(),
    );
    let w = frame.grid().width;
    let mut selected = Vec::new();
    // Pick extremes by g and a couple of radii.
    for (idx, pixel) in frame.pixels().iter().enumerate() {
        if let SpectralPixel::DiskHit(s) = pixel {
            selected.push((idx, s));
        }
    }
    selected.sort_by(|a, b| a.1.g_factor.total_cmp(&b.1.g_factor));
    let mut picks = Vec::new();
    if let Some(x) = selected.first() {
        picks.push(*x);
    }
    if let Some(x) = selected.last() {
        picks.push(*x);
    }
    if let Some(x) = selected.get(selected.len() / 2) {
        picks.push(*x);
    }
    selected.sort_by(|a, b| a.1.radius.total_cmp(&b.1.radius));
    if let Some(x) = selected.first() {
        picks.push(*x);
    }
    if let Some(x) = selected.last() {
        picks.push(*x);
    }
    picks.sort_by_key(|(i, _)| *i);
    picks.dedup_by_key(|(i, _)| *i);
    for (idx, s) in picks {
        let col = (idx as u32) % w;
        let row = (idx as u32) / w;
        rows.push(format!(
            "{idx},{col},{row},{},{},{},{},{},{},{}",
            s.g_factor,
            s.emitted_bolometric_intensity,
            s.observed_bolometric_intensity,
            s.integrated_emitted_i_nu,
            s.integrated_observed_i_nu,
            s.truncated_emitted_energy_fraction,
            s.truncated_observed_energy_fraction
        ));
    }
    std::fs::write(
        out.join("selected-pixel-spectra.csv"),
        rows.join("\n") + "\n",
    )?;
    Ok(())
}

fn log_gray(v: f64) -> u8 {
    if !(v > 0.0) {
        return 0;
    }
    let stops = (v.log2() + 16.0).clamp(0.0, 19.0);
    ((stops / 19.0) * 255.0) as u8
}

fn write_pgm(
    path: PathBuf,
    w: u32,
    h: u32,
    pixels: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = format!("P5\n{w} {h}\n255\n").into_bytes();
    out.extend_from_slice(pixels);
    std::fs::write(path, out)?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}

fn resolve_path(root: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}
