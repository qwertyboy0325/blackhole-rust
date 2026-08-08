//! Gate 2D0: presentation pipeline over Gate 2C1 PhysicalColorFrame → RGB16 sRGB PNG.

use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::diagnostic_scene::build_diagnostic_trace_scene;
use crate::preset::load_preset;
use crate::render_tier::{resolve_render_plan, DiagnosticRenderTier};
use crate::trace_outcome_map::{resolve_execution, CliExecution};
use relativity_render::{
    authored_rgb16_bytes, build_disk_frequency_shift_frame, build_physical_color_frame,
    build_physical_disk_emission_frame, build_physical_spectral_frame, disk_frequency_shift_digest,
    encode_physical_color_payload, parse_physical_spectral_grid_id, payload_sha256,
    physical_color_digest, physical_disk_emission_digest, physical_spectral_digest,
    physical_spectral_grid_digest, png_metadata_constants, present_physical_color_frame,
    validate_physical_emission_provenance, verify_observer_unit_frequency,
    verify_payload_matches_frame, Cie1931Table, CieObserverId, DiskFrequencyShiftConvention,
    DiskVelocityModel, IntegrationMeasure, PhysicalDiskEmissionConvention,
    PhysicalDiskEmissionSpec, PhysicalSpectralConvention, PresentationSpec, XyzToRgbMatrix,
    BIT_DEPTH_RGB16, CIE_OBSERVER_ID_V1, CIE_RELATIVE_ASSET_PATH, CIE_TABLE_SHA256,
    OBSERVER_UNIT_FREQUENCY_TOLERANCE, PHYSICAL_EMISSION_MODEL_ID, PHYSICAL_GRID_V1_ID,
    PNG_FORMAT_RGB16_SRGB_V1, PNG_GAMA_SRGB, PNG_SRGB_INTENT_PERCEPTUAL, SCENE_LINEAR_RGB_SPACE_ID,
};
use relativity_trace::{trace_grid_with_execution_and_surface_set, TraceGrid, TraceSurfaceSet};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationPresetFile {
    schema_version: u32,
    model_id: String,
    #[allow(dead_code)]
    description: Option<String>,
    middle_gray_luminance_cd_m2: f64,
    exposure_ev: f64,
    tone_mapper: String,
    gamut_mapper: String,
    display_target: String,
    oetf: String,
    bit_depth: u16,
}

#[derive(Serialize)]
pub struct PresentationMeta {
    pub gate: &'static str,
    pub authority: &'static str,
    pub presentation_role: &'static str,
    pub source_physical_color_digest: String,
    pub source_payload_sha256: String,
    pub source_cie_table_sha256: String,
    pub source_frequency_digest: String,
    pub source_physical_emission_digest: String,
    pub source_physical_spectral_digest: Option<String>,
    pub source_physical_spectral_grid_digest: Option<String>,
    pub presentation_spec_digest: String,
    pub presentation_frame_digest: String,
    pub middle_gray_luminance_cd_m2: f64,
    pub exposure_ev: f64,
    pub tone_mapper: String,
    pub gamut_mapper: String,
    pub display_target: String,
    pub oetf: String,
    pub bit_depth: u16,
    pub png_format: &'static str,
    pub png_srgb_intent: u8,
    pub png_srgb_intent_name: &'static str,
    pub png_gama: u32,
    pub png_chrm: &'static str,
    pub png_icc: &'static str,
    pub width: u32,
    pub height: u32,
    pub beauty_png: &'static str,
    pub metrics: relativity_render::PresentationMetrics,
}

#[derive(Serialize)]
struct PresentationReport {
    gate: &'static str,
    result_hint: &'static str,
    source_physical_color_digest: String,
    source_payload_sha256: String,
    presentation_spec_digest: String,
    presentation_frame_digest: String,
    middle_gray_luminance_cd_m2: f64,
    exposure_ev: f64,
    tone_mapper: String,
    gamut_mapper: String,
    png_srgb_intent: u8,
    png_gama: u32,
    png_bytes: u64,
    png_roundtrip_ok: bool,
    build: BuildExecutionMetadata,
    trace_wall_clock_seconds: f64,
    color_wall_clock_seconds: f64,
    presentation_wall_clock_seconds: f64,
    total_wall_clock_seconds: f64,
    metrics: relativity_render::PresentationMetrics,
    note: &'static str,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    preset_path: &str,
    presentation_path: &str,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
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

    let plan = resolve_render_plan(tier, width, height)?;
    let trace_execution = resolve_execution(execution, threads)?;
    let root = workspace_root()?;
    let out_dir = resolve_path(&root, output_dir);
    let preset_full = resolve_path(&root, preset_path);
    let presentation_full = resolve_path(&root, presentation_path);
    let preset = load_preset(&preset_full)?;
    let presentation_spec = load_presentation_spec(&presentation_full)?;
    validate_physical_emission_provenance(&preset.disk.emission_model, &preset.disk.emission_claim)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let physical = preset
        .physical
        .as_ref()
        .ok_or("preset missing [physical] section required for render-presentation")?;
    if physical.emission_model != PHYSICAL_EMISSION_MODEL_ID {
        return Err("preset [physical].emission_model mismatch".into());
    }

    std::fs::create_dir_all(&out_dir)?;

    let cie = Cie1931Table::load_official_v1_from_path(&root.join(CIE_RELATIVE_ASSET_PATH))
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    if cie.content_sha256 != CIE_TABLE_SHA256 {
        return Err("CIE table SHA-256 mismatch".into());
    }

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
    let emission_digest = physical_disk_emission_digest(
        &emission,
        &PhysicalDiskEmissionConvention::v1(),
        &emission_spec,
        &freq_digest,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    let spectral_grid = parse_physical_spectral_grid_id(PHYSICAL_GRID_V1_ID)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let spectral = build_physical_spectral_frame(&emission, &spectral_grid)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let grid_digest = physical_spectral_grid_digest(&spectral_grid)?;
    let spectral_digest = physical_spectral_digest(
        &spectral,
        &PhysicalSpectralConvention::v1(),
        &emission_digest,
    )?;

    let rgb_matrix = XyzToRgbMatrix::rec709_d65_linear_v1();
    let t_color = Instant::now();
    let color = build_physical_color_frame(
        &emission,
        &cie,
        &rgb_matrix,
        &emission_digest,
        &freq_digest,
        Some(spectral_digest.as_str()),
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

    let t_pres = Instant::now();
    let presented = present_physical_color_frame(&color, &presentation_spec)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let presentation_wall = t_pres.elapsed().as_secs_f64();

    let png_path = out_dir.join("beauty-srgb16.png");
    let png_bytes = write_beauty_png(&png_path, &presented)?;
    let roundtrip = verify_beauty_png(&png_path, &presented)?;

    let (intent, gama) = png_metadata_constants();
    let meta = PresentationMeta {
        gate: "gate-2d0-presentation",
        authority: "PRESENTATION_REPRODUCIBILITY_DIGEST",
        presentation_role: "display-referred beauty; not scientific radiance authority",
        source_physical_color_digest: color_digest.clone(),
        source_payload_sha256: payload_digest.clone(),
        source_cie_table_sha256: CIE_TABLE_SHA256.into(),
        source_frequency_digest: freq_digest.clone(),
        source_physical_emission_digest: emission_digest.clone(),
        source_physical_spectral_digest: Some(spectral_digest.clone()),
        source_physical_spectral_grid_digest: Some(grid_digest.clone()),
        presentation_spec_digest: presented.presentation_spec_digest.clone(),
        presentation_frame_digest: presented.presentation_frame_digest.clone(),
        middle_gray_luminance_cd_m2: presentation_spec.middle_gray_luminance_cd_m2,
        exposure_ev: presentation_spec.exposure_ev,
        tone_mapper: presentation_spec.tone_mapper.clone(),
        gamut_mapper: presentation_spec.gamut_mapper.clone(),
        display_target: presentation_spec.display_target.clone(),
        oetf: presentation_spec.oetf.clone(),
        bit_depth: BIT_DEPTH_RGB16,
        png_format: PNG_FORMAT_RGB16_SRGB_V1,
        png_srgb_intent: intent,
        png_srgb_intent_name: "Perceptual",
        png_gama: gama,
        png_chrm: "OMIT",
        png_icc: "OMIT",
        width: presented.width,
        height: presented.height,
        beauty_png: "beauty-srgb16.png",
        metrics: presented.metrics.clone(),
    };
    std::fs::write(
        out_dir.join("presentation-meta.json"),
        serde_json::to_vec_pretty(&meta)?,
    )?;

    let report = PresentationReport {
        gate: "gate-2d0-presentation",
        result_hint: "presentation artifact written; evaluate separately for PASS",
        source_physical_color_digest: color_digest,
        source_payload_sha256: payload_digest,
        presentation_spec_digest: presented.presentation_spec_digest,
        presentation_frame_digest: presented.presentation_frame_digest,
        middle_gray_luminance_cd_m2: presentation_spec.middle_gray_luminance_cd_m2,
        exposure_ev: presentation_spec.exposure_ev,
        tone_mapper: presentation_spec.tone_mapper,
        gamut_mapper: presentation_spec.gamut_mapper,
        png_srgb_intent: intent,
        png_gama: gama,
        png_bytes,
        png_roundtrip_ok: roundtrip,
        build,
        trace_wall_clock_seconds: trace_wall,
        color_wall_clock_seconds: color_wall,
        presentation_wall_clock_seconds: presentation_wall,
        total_wall_clock_seconds: t0.elapsed().as_secs_f64(),
        metrics: presented.metrics,
        note: "display black for absence is presentation fill, not scientific RGB=0 authority",
    };
    std::fs::write(
        out_dir.join("presentation-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;

    // Silence unused observer ID string constants referenced for provenance clarity.
    let _ = (
        CieObserverId::Cie1931TwoDegV1,
        CIE_OBSERVER_ID_V1,
        SCENE_LINEAR_RGB_SPACE_ID,
    );
    Ok(())
}

pub fn load_presentation_spec(path: &Path) -> Result<PresentationSpec, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let file: PresentationPresetFile = toml::from_str(&text)?;
    let mut spec = PresentationSpec::v1(file.middle_gray_luminance_cd_m2, file.exposure_ev)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    // Enforce file IDs match canonical V1 (reject drift).
    if file.schema_version != 1
        || file.model_id != spec.model_id
        || file.tone_mapper != spec.tone_mapper
        || file.gamut_mapper != spec.gamut_mapper
        || file.display_target != spec.display_target
        || file.oetf != spec.oetf
        || file.bit_depth != spec.bit_depth
    {
        return Err("presentation preset fields do not match presentation-model-v1".into());
    }
    spec.validate()
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let _ = &mut spec;
    Ok(spec)
}

pub fn write_beauty_png(
    path: &Path,
    frame: &relativity_render::PresentationFrame,
) -> Result<u64, Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Sixteen);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder.set_source_gamma(png::ScaledFloat::from_scaled(PNG_GAMA_SRGB));
    // A4: omit cHRM and ICC.
    let mut writer = encoder.write_header()?;
    let bytes = authored_rgb16_bytes(&frame.pixels);
    writer.write_image_data(&bytes)?;
    drop(writer);
    Ok(std::fs::metadata(path)?.len())
}

pub fn verify_beauty_png(
    path: &Path,
    expected: &relativity_render::PresentationFrame,
) -> Result<bool, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    if info.width != expected.width || info.height != expected.height {
        return Err("PNG dimension mismatch".into());
    }
    if info.color_type != png::ColorType::Rgb {
        return Err("PNG color type is not RGB".into());
    }
    if info.bit_depth != png::BitDepth::Sixteen {
        return Err("PNG bit depth is not 16".into());
    }
    match info.srgb {
        Some(png::SrgbRenderingIntent::Perceptual) => {}
        Some(other) => {
            return Err(format!("PNG sRGB intent {:?}, expected Perceptual", other).into())
        }
        None => return Err("PNG missing sRGB chunk".into()),
    }
    let gama = info
        .gama_chunk
        .ok_or("PNG missing gAMA chunk")?
        .into_scaled();
    if gama != PNG_GAMA_SRGB {
        return Err(format!("PNG gAMA={gama}, expected {PNG_GAMA_SRGB}").into());
    }
    if info.chrm_chunk.is_some() {
        return Err("PNG unexpectedly contains cHRM chunk".into());
    }
    if info.icc_profile.is_some() {
        return Err("PNG unexpectedly contains iCCP/ICC profile".into());
    }
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("PNG buffer size")?];
    let frame_info = reader.next_frame(&mut buf)?;
    let data = &buf[..frame_info.buffer_size()];
    let expect_bytes = authored_rgb16_bytes(&expected.pixels);
    if data != expect_bytes.as_slice() {
        return Err("PNG decoded RGB16 raster mismatch".into());
    }
    let _ = PNG_SRGB_INTENT_PERCEPTUAL;
    Ok(true)
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("xtask").is_dir() {
            return Ok(dir);
        }
        dir = dir
            .parent()
            .ok_or("workspace root not found")?
            .to_path_buf();
    }
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
    use relativity_render::{DisplayEncodedRgb16, PresentationFrame, PresentationMetrics};

    #[test]
    fn png_roundtrip_1x1() {
        let frame = PresentationFrame {
            width: 1,
            height: 1,
            pixels: vec![DisplayEncodedRgb16 {
                r: 1000,
                g: 2000,
                b: 3000,
            }],
            source_physical_color_digest: "a".repeat(64),
            presentation_spec_digest: "b".repeat(64),
            presentation_frame_digest: "c".repeat(64),
            metrics: PresentationMetrics {
                pixel_count: 1,
                source_disk_hit_count: 0,
                negative_component_count_before_gamut: 0,
                negative_pixel_count_before_gamut: 0,
                gamut_adjusted_pixel_count: 0,
                max_gamut_correction: 0.0,
                worst_gamut_raster_index: None,
                pre_tone_max_rgb: 0.0,
                pre_tone_min_luma: 0.0,
                pre_tone_max_luma: 0.0,
                pre_tone_median_luma_estimate: 0.0,
                post_tone_min: 0.0,
                post_tone_max: 0.0,
                endpoint_epsilon_canonicalization_count: 0,
                final_code_min: 0,
                final_code_max: 0,
            },
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.png");
        write_beauty_png(&path, &frame).unwrap();
        assert!(verify_beauty_png(&path, &frame).unwrap());
    }
}
