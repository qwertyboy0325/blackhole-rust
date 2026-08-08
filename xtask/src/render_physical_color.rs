//! Gate 2C1: physical colorimetry (Arch B) + raw f64 authority + derived OpenEXR FLOAT.

use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::diagnostic_scene::build_diagnostic_trace_scene;
use crate::preset::load_preset;
use crate::reference_pipeline::summarize_outcomes;
use crate::render_tier::{resolve_render_plan, DiagnosticRenderTier};
use crate::trace_outcome_map::{resolve_execution, CliExecution};
use exr::image::write::WritableImage;
use exr::image::{AnyChannel, AnyChannels, Encoding, FlatSamples, Image, Layer, Levels};
use exr::meta::header::LayerAttributes;
use exr::prelude::{read_all_data_from_file, Compression};
use relativity_render::{
    build_disk_frequency_shift_frame, build_physical_color_frame,
    build_physical_disk_emission_frame, build_physical_spectral_frame,
    compute_colorimetric_metrics, diagnostic_a_vs_b, disk_frequency_shift_digest,
    encode_physical_color_payload, outcome_class_code, parse_physical_spectral_grid_id,
    payload_sha256, physical_color_digest, physical_disk_emission_digest,
    physical_disk_emission_spec_digest, physical_spectral_digest, physical_spectral_grid_digest,
    validate_physical_emission_provenance, verify_observer_unit_frequency,
    verify_payload_matches_frame, Cie1931Table, CieObserverId, DiskFrequencyShiftConvention,
    DiskVelocityModel, IntegrationMeasure, PhysicalColorPixel, PhysicalDiskEmissionConvention,
    PhysicalDiskEmissionSpec, PhysicalSpectralConvention, SceneLinearRgbSpace, XyzToRgbMatrix,
    CIE_OBSERVER_ID_V1, CIE_RELATIVE_ASSET_PATH, OBSERVER_UNIT_FREQUENCY_TOLERANCE,
    PHYSICAL_EMISSION_MODEL_ID, PHYSICAL_GRID_V1_ID, PRODUCTION_BAND_ID, SCENE_LINEAR_RGB_SPACE_ID,
};
use relativity_trace::{trace_grid_with_execution_and_surface_set, TraceGrid, TraceSurfaceSet};
use serde::Serialize;
use smallvec::smallvec;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Serialize)]
struct ColorRenderReport {
    gate: &'static str,
    architecture: &'static str,
    cie_observer_id: String,
    rgb_space_id: String,
    frequency_shift_digest: String,
    physical_emission_spec_digest: String,
    physical_emission_digest: String,
    physical_spectral_grid_digest: Option<String>,
    physical_spectral_digest: Option<String>,
    physical_color_digest: String,
    payload_sha256: String,
    cie_table_sha256: String,
    rgb_matrix_digest: String,
    disk_hit_count: u64,
    color_disk_hit_count: u64,
    metrics: relativity_render::ColorimetricMetrics,
    build: BuildExecutionMetadata,
    trace_wall_clock_seconds: f64,
    color_wall_clock_seconds: f64,
    total_wall_clock_seconds: f64,
    exr_role: &'static str,
    preview_png: &'static str,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    preset_path: &str,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
    cie_observer: &str,
    rgb_space: &str,
    output_dir: &str,
    require_release: bool,
    execution: CliExecution,
    threads: Option<usize>,
    include_spectral_diagnostic: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let t0 = Instant::now();
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }

    let _observer = CieObserverId::parse(cie_observer)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let _rgb = SceneLinearRgbSpace::parse(rgb_space)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

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
        .ok_or("preset missing [physical] section required for render-physical-color")?;
    if physical.emission_model != PHYSICAL_EMISSION_MODEL_ID {
        return Err("preset [physical].emission_model mismatch".into());
    }

    std::fs::create_dir_all(&out_dir)?;

    let cie = Cie1931Table::load_official_v1_from_path(&root.join(CIE_RELATIVE_ASSET_PATH))
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

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

    let (spectral_grid_digest, spectral_digest) = if include_spectral_diagnostic {
        let spectral_grid = parse_physical_spectral_grid_id(PHYSICAL_GRID_V1_ID)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let spectral = build_physical_spectral_frame(&emission, &spectral_grid)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let gdigest = physical_spectral_grid_digest(&spectral_grid)?;
        let sdigest = physical_spectral_digest(
            &spectral,
            &PhysicalSpectralConvention::v1(),
            &emission_digest,
        )?;
        (Some(gdigest), Some((sdigest, spectral)))
    } else {
        (None, None)
    };

    let rgb_matrix = XyzToRgbMatrix::rec709_d65_linear_v1();

    let t_color = Instant::now();
    let spectral_digest_str = spectral_digest.as_ref().map(|(d, _)| d.as_str());
    let color = build_physical_color_frame(
        &emission,
        &cie,
        &rgb_matrix,
        &emission_digest,
        &freq_digest,
        spectral_digest_str,
        IntegrationMeasure::FrequencyNu,
        1,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let color_wall = t_color.elapsed().as_secs_f64();
    let color_digest = physical_color_digest(&color)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let payload = encode_physical_color_payload(&color)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    verify_payload_matches_frame(&payload, &color)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let payload_digest = payload_sha256(&payload);
    let metrics = compute_colorimetric_metrics(&color);

    if let Some((_, spectral)) = spectral_digest.as_ref() {
        let report = diagnostic_a_vs_b(&color, spectral, &cie)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        std::fs::write(
            out_dir.join("diagnostic-a-vs-b.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
    }

    write_colorimetry_meta(&out_dir, &color, &color_digest, &payload_digest, &metrics)?;
    std::fs::write(out_dir.join("physical-xyz-rgb.f64le"), &payload)?;
    write_selected_pixels_csv(&out_dir, &color)?;
    write_physical_color_exr(&out_dir.join("physical-color.exr"), &color)?;
    verify_exr_roundtrip(&out_dir.join("physical-color.exr"), &color)?;

    let color_disk_hit_count = metrics.disk_hit_count;
    let report = ColorRenderReport {
        gate: "gate-2c1-colorimetry",
        architecture: "B-emission-frame-cie-1nm",
        cie_observer_id: CIE_OBSERVER_ID_V1.into(),
        rgb_space_id: SCENE_LINEAR_RGB_SPACE_ID.into(),
        frequency_shift_digest: freq_digest,
        physical_emission_spec_digest: emission_spec_digest,
        physical_emission_digest: emission_digest,
        physical_spectral_grid_digest: spectral_grid_digest,
        physical_spectral_digest: spectral_digest.map(|(d, _)| d),
        physical_color_digest: color_digest,
        payload_sha256: payload_digest,
        cie_table_sha256: color.provenance.cie_table_sha256.clone(),
        rgb_matrix_digest: color.provenance.rgb_matrix_digest.clone(),
        disk_hit_count: counts.disk_hit,
        color_disk_hit_count,
        metrics,
        build,
        trace_wall_clock_seconds: trace_wall,
        color_wall_clock_seconds: color_wall,
        total_wall_clock_seconds: t0.elapsed().as_secs_f64(),
        exr_role: "DERIVED_INTERCHANGE_ARTIFACT",
        preview_png: "DEFER",
    };
    std::fs::write(
        out_dir.join("physical-color-render-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "Gate 2C1 colorimetry render complete: color_digest={} disk_hits={}",
        report.physical_color_digest, report.color_disk_hit_count
    );
    Ok(())
}

fn write_colorimetry_meta(
    out: &Path,
    frame: &relativity_render::PhysicalColorFrame,
    digest: &str,
    payload_digest: &str,
    metrics: &relativity_render::ColorimetricMetrics,
) -> Result<(), Box<dyn std::error::Error>> {
    let meta = serde_json::json!({
        "schema_version": 1,
        "frame": "PhysicalColorFrame",
        "architecture": "B",
        "authority": "raw-f64le",
        "exr_role": "DERIVED_INTERCHANGE_ARTIFACT",
        "width": frame.grid.width,
        "height": frame.grid.height,
        "cie_observer_id": frame.observer.id(),
        "rgb_space_id": frame.rgb_space.id(),
        "convention": frame.convention,
        "provenance": frame.provenance,
        "physical_color_digest": digest,
        "payload_sha256": payload_digest,
        "payload_schema": 2,
        "production_band_id": PRODUCTION_BAND_ID,
        "cie_license": "CC-BY-SA-4.0",
        "cie_load_mode": "runtime-vendored-asset",
        "metrics": metrics,
        "payload": "physical-xyz-rgb.f64le",
        "exr": "physical-color.exr",
        "units": {
            "X": "absolute CIE X (Km-scaled)",
            "Y": "cd/m^2",
            "Z": "absolute CIE Z (Km-scaled)",
            "RGB": "scene-linear Rec.709/D65 unclamped",
        },
        "presentation": "DEFER_NO_TONE_MAP",
    });
    std::fs::write(
        out.join("physical-colorimetry-meta.json"),
        serde_json::to_vec_pretty(&meta)?,
    )?;
    Ok(())
}

fn write_selected_pixels_csv(
    out: &Path,
    frame: &relativity_render::PhysicalColorFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    // Fixed raster samples for review (corners + mid + a few disk-scan picks).
    let mut lines = vec!["col,row,kind,X,Y,Z,R,G,B,g,T_eff,F,r_over_m".into()];
    let w = frame.grid.width;
    let h = frame.grid.height;
    let mut candidates = vec![
        (0u32, 0u32),
        (w / 2, h / 2),
        (w.saturating_sub(1), h.saturating_sub(1)),
        (w / 4, h / 2),
        (3 * w / 4, h / 2),
        (w / 2, h / 4),
        (w / 2, 3 * h / 4),
    ];
    // Add first few disk hits by raster order for stable diagnostics.
    let mut added = 0u32;
    for row in 0..h {
        for col in 0..w {
            if matches!(frame.pixel_at(col, row), PhysicalColorPixel::DiskHit(_)) {
                candidates.push((col, row));
                added += 1;
                if added >= 8 {
                    break;
                }
            }
        }
        if added >= 8 {
            break;
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    for (col, row) in candidates {
        match frame.pixel_at(col, row) {
            PhysicalColorPixel::DiskHit(s) => {
                lines.push(format!(
                    "{col},{row},disk,{},{},{},{},{},{},{},{},{},{}",
                    s.xyz.x,
                    s.xyz.y,
                    s.xyz.z,
                    s.rgb.r,
                    s.rgb.g,
                    s.rgb.b,
                    s.g_factor,
                    s.t_eff_k,
                    s.f_one_face_w_m2,
                    s.radius_over_m
                ));
            }
            PhysicalColorPixel::Absent { outcome_class } => {
                lines.push(format!(
                    "{col},{row},absent-{},,,,,,,,,,",
                    outcome_class.digest_tag()
                ));
            }
        }
    }
    std::fs::write(out.join("selected-pixels.csv"), lines.join("\n") + "\n")?;
    Ok(())
}

fn f64_to_f32_checked(v: f64, ctx: &str) -> Result<f32, Box<dyn std::error::Error>> {
    if !v.is_finite() {
        return Err(format!("non-finite f64 before EXR ({ctx})").into());
    }
    let f = v as f32;
    if !f.is_finite() {
        return Err(format!("f64→f32 overflow ({ctx})").into());
    }
    Ok(f)
}

pub fn write_physical_color_exr(
    path: &Path,
    frame: &relativity_render::PhysicalColorFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let w = frame.grid.width as usize;
    let h = frame.grid.height as usize;
    let n = w * h;
    let mut ch_x = vec![0f32; n];
    let mut ch_y = vec![0f32; n];
    let mut ch_z = vec![0f32; n];
    let mut ch_r = vec![0f32; n];
    let mut ch_g = vec![0f32; n];
    let mut ch_b = vec![0f32; n];
    let mut ch_gf = vec![0f32; n];
    let mut ch_f = vec![0f32; n];
    let mut ch_t = vec![0f32; n];
    let mut ch_rom = vec![0f32; n];
    let mut ch_mask = vec![0u32; n];
    let mut ch_outcome = vec![0u32; n];

    for (i, pixel) in frame.pixels.iter().enumerate() {
        match pixel {
            PhysicalColorPixel::DiskHit(s) => {
                ch_x[i] = f64_to_f32_checked(s.xyz.x, "X")?;
                ch_y[i] = f64_to_f32_checked(s.xyz.y, "Y")?;
                ch_z[i] = f64_to_f32_checked(s.xyz.z, "Z")?;
                ch_r[i] = f64_to_f32_checked(s.rgb.r, "R")?;
                ch_g[i] = f64_to_f32_checked(s.rgb.g, "G")?;
                ch_b[i] = f64_to_f32_checked(s.rgb.b, "B")?;
                ch_gf[i] = f64_to_f32_checked(s.g_factor, "g")?;
                ch_f[i] = f64_to_f32_checked(s.f_one_face_w_m2, "F")?;
                ch_t[i] = f64_to_f32_checked(s.t_eff_k, "T")?;
                ch_rom[i] = f64_to_f32_checked(s.radius_over_m, "r/M")?;
                ch_mask[i] = 1;
                ch_outcome[i] = outcome_class_u32(relativity_trace::OutcomeClass::DiskHit);
            }
            PhysicalColorPixel::Absent { outcome_class } => {
                ch_mask[i] = 0;
                ch_outcome[i] = outcome_class_u32(*outcome_class);
            }
        }
    }

    let channels = AnyChannels::sort(smallvec![
        AnyChannel::new("X", FlatSamples::F32(ch_x)),
        AnyChannel::new("Y", FlatSamples::F32(ch_y)),
        AnyChannel::new("Z", FlatSamples::F32(ch_z)),
        AnyChannel::new("R", FlatSamples::F32(ch_r)),
        AnyChannel::new("G", FlatSamples::F32(ch_g)),
        AnyChannel::new("B", FlatSamples::F32(ch_b)),
        AnyChannel::new("phys.g", FlatSamples::F32(ch_gf)),
        AnyChannel::new("phys.F", FlatSamples::F32(ch_f)),
        AnyChannel::new("phys.T", FlatSamples::F32(ch_t)),
        AnyChannel::new("phys.r_over_m", FlatSamples::F32(ch_rom)),
        AnyChannel::new("disk.mask", FlatSamples::U32(ch_mask)),
        AnyChannel::new("outcome", FlatSamples::U32(ch_outcome)),
    ]);

    let encoding = Encoding {
        compression: Compression::Uncompressed,
        ..Encoding::default()
    };
    let layer = Layer::new(
        (w, h),
        LayerAttributes::named("physical-color-v1"),
        encoding,
        channels,
    );
    let image = Image::from_layer(layer);
    image
        .write()
        .to_file(path)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    Ok(())
}

fn outcome_class_u32(c: relativity_trace::OutcomeClass) -> u32 {
    u32::from(outcome_class_code(c))
}

pub fn verify_exr_roundtrip(
    path: &Path,
    frame: &relativity_render::PhysicalColorFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let image = read_all_data_from_file(path)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let layer = image.layer_data.first().ok_or("EXR missing layer")?;
    let w = frame.grid.width as usize;
    let h = frame.grid.height as usize;
    if layer.size.0 != w || layer.size.1 != h {
        return Err(format!(
            "EXR size mismatch: got {}x{} expected {w}x{h}",
            layer.size.0, layer.size.1
        )
        .into());
    }

    let get_f32 = |name: &str| -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let ch = layer
            .channel_data
            .list
            .iter()
            .find(|c| c.name.eq(name))
            .ok_or_else(|| format!("missing EXR channel {name}"))?;
        match &ch.sample_data {
            Levels::Singular(FlatSamples::F32(v)) => Ok(v.clone()),
            _ => Err(format!("channel {name} not singular FLOAT").into()),
        }
    };
    let get_u32 = |name: &str| -> Result<Vec<u32>, Box<dyn std::error::Error>> {
        let ch = layer
            .channel_data
            .list
            .iter()
            .find(|c| c.name.eq(name))
            .ok_or_else(|| format!("missing EXR channel {name}"))?;
        match &ch.sample_data {
            Levels::Singular(FlatSamples::U32(v)) => Ok(v.clone()),
            _ => Err(format!("channel {name} not singular UINT").into()),
        }
    };

    let x = get_f32("X")?;
    let y = get_f32("Y")?;
    let z = get_f32("Z")?;
    let r = get_f32("R")?;
    let g = get_f32("G")?;
    let b = get_f32("B")?;
    let gf = get_f32("phys.g")?;
    let ff = get_f32("phys.F")?;
    let tt = get_f32("phys.T")?;
    let rom = get_f32("phys.r_over_m")?;
    let mask = get_u32("disk.mask")?;
    let outcome = get_u32("outcome")?;

    let assert_f32 =
        |name: &str, i: usize, expect: f32, got: f32| -> Result<(), Box<dyn std::error::Error>> {
            if expect.to_bits() != got.to_bits() {
                return Err(
                    format!("EXR f32 mismatch {name}@{i}: expect={expect} got={got}").into(),
                );
            }
            Ok(())
        };

    for (i, pixel) in frame.pixels.iter().enumerate() {
        match pixel {
            PhysicalColorPixel::DiskHit(s) => {
                if mask[i] != 1 {
                    return Err(format!("mask mismatch at {i}").into());
                }
                if outcome[i] != outcome_class_u32(relativity_trace::OutcomeClass::DiskHit) {
                    return Err(format!(
                        "outcome mismatch at {i}: expect DiskHit got {}",
                        outcome[i]
                    )
                    .into());
                }
                assert_f32("X", i, s.xyz.x as f32, x[i])?;
                assert_f32("Y", i, s.xyz.y as f32, y[i])?;
                assert_f32("Z", i, s.xyz.z as f32, z[i])?;
                assert_f32("R", i, s.rgb.r as f32, r[i])?;
                assert_f32("G", i, s.rgb.g as f32, g[i])?;
                assert_f32("B", i, s.rgb.b as f32, b[i])?;
                assert_f32("phys.g", i, s.g_factor as f32, gf[i])?;
                assert_f32("phys.F", i, s.f_one_face_w_m2 as f32, ff[i])?;
                assert_f32("phys.T", i, s.t_eff_k as f32, tt[i])?;
                assert_f32("phys.r_over_m", i, s.radius_over_m as f32, rom[i])?;
            }
            PhysicalColorPixel::Absent { outcome_class } => {
                if mask[i] != 0 {
                    return Err(format!("mask expected 0 at {i}").into());
                }
                let expect_outcome = outcome_class_u32(*outcome_class);
                if outcome[i] != expect_outcome {
                    return Err(format!(
                        "outcome mismatch at {i}: expect {expect_outcome} got {}",
                        outcome[i]
                    )
                    .into());
                }
                for (name, got) in [
                    ("X", x[i]),
                    ("Y", y[i]),
                    ("Z", z[i]),
                    ("R", r[i]),
                    ("G", g[i]),
                    ("B", b[i]),
                    ("phys.g", gf[i]),
                    ("phys.F", ff[i]),
                    ("phys.T", tt[i]),
                    ("phys.r_over_m", rom[i]),
                ] {
                    assert_f32(name, i, 0.0, got)?;
                }
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_render::{
        CieObserverId, ColorDiskHit, ColorPixelProvenance, ColorimetricConvention, ColorimetricXyz,
        PhysicalColorFrame, SceneLinearRgb, SceneLinearRgbSpace, CIE_TABLE_SHA256,
        COLORIMETRIC_CONVENTION_ID,
    };
    use relativity_trace::{OutcomeClass, TraceGrid};
    use tempfile::tempdir;

    #[test]
    fn exr_roundtrip_tiny_with_negative() {
        let grid = TraceGrid {
            width: 2,
            height: 2,
        };
        let fake = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let hit = ColorDiskHit {
            xyz: ColorimetricXyz::new(1.0, 2.0, 3.0).unwrap(),
            rgb: SceneLinearRgb::new(-0.25, 1.5, 0.0).unwrap(),
            g_factor: 1.1,
            f_one_face_w_m2: 1e6,
            t_eff_k: 5000.0,
            radius_over_m: 10.0,
        };
        let pixels = vec![
            PhysicalColorPixel::DiskHit(hit.clone()),
            PhysicalColorPixel::Absent {
                outcome_class: OutcomeClass::Escaped,
            },
            PhysicalColorPixel::DiskHit(ColorDiskHit {
                xyz: ColorimetricXyz::new(0.1, 0.2, 0.3).unwrap(),
                rgb: SceneLinearRgb::new(10.0, -1.0, 0.5).unwrap(),
                ..hit
            }),
            PhysicalColorPixel::Absent {
                outcome_class: OutcomeClass::HorizonEvent,
            },
        ];
        let frame = PhysicalColorFrame::try_new(
            grid,
            pixels,
            ColorPixelProvenance {
                source_physical_emission_digest: fake.into(),
                source_frequency_digest: fake.into(),
                cie_table_sha256: CIE_TABLE_SHA256.into(),
                cie_observer_id: CIE_OBSERVER_ID_V1.into(),
                colorimetric_convention_id: COLORIMETRIC_CONVENTION_ID.into(),
                rgb_space_id: SCENE_LINEAR_RGB_SPACE_ID.into(),
                rgb_matrix_digest: fake.into(),
                source_physical_spectral_digest: None,
            },
            ColorimetricConvention::v1(),
            CieObserverId::Cie1931TwoDegV1,
            SceneLinearRgbSpace::Rec709D65LinearV1,
        )
        .unwrap();
        let _ = physical_color_digest(&frame).unwrap();
        let dir = tempdir().unwrap();
        let path = dir.path().join("t.exr");
        write_physical_color_exr(&path, &frame).unwrap();
        verify_exr_roundtrip(&path, &frame).unwrap();
    }
}
