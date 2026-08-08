//! Gate 2D1: scene appearance (disk modulation + celestial environment) → RGB16 beauty.

use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::diagnostic_scene::build_diagnostic_trace_scene;
use crate::preset::load_preset;
use crate::render_presentation::{load_presentation_spec, verify_beauty_png, write_beauty_png};
use crate::render_tier::{resolve_render_plan, DiagnosticRenderTier};
use crate::trace_outcome_map::{resolve_execution, CliExecution};
use relativity_render::{
    appearance_disk_color_digest, authored_rgb16_bytes, build_appearance_disk_color_frame,
    build_appearance_disk_emission_frame, build_celestial_environment,
    build_disk_frequency_shift_frame, build_physical_color_frame,
    build_physical_disk_emission_frame, build_physical_spectral_frame,
    build_scene_appearance_frame, disk_appearance_spec_digest, disk_frequency_shift_digest,
    encode_physical_color_payload, environment_spec_digest, parse_physical_spectral_grid_id,
    payload_sha256, physical_color_digest, physical_disk_emission_digest, physical_spectral_digest,
    physical_spectral_grid_digest, png_metadata_constants, present_physical_color_frame,
    present_scene_appearance_frame, render_environment_reference,
    validate_physical_emission_provenance, verify_observer_unit_frequency,
    verify_payload_matches_frame, Cie1931Table, DiskAppearanceSpec, DiskFrequencyShiftConvention,
    DiskVelocityModel, EnvironmentSpec, IntegrationMeasure, MilkyWayLikeSpec,
    PhysicalDiskEmissionConvention, PhysicalDiskEmissionSpec, PhysicalSpectralConvention,
    SpiralHarmonicMode, StarsSpec, UnitQuaternion, XyzToRgbMatrix, CIE_RELATIVE_ASSET_PATH,
    CIE_TABLE_SHA256, OBSERVER_UNIT_FREQUENCY_TOLERANCE, PHYSICAL_EMISSION_MODEL_ID,
    PHYSICAL_GRID_V1_ID, PNG_FORMAT_RGB16_SRGB_V1, PNG_GAMA_SRGB, PNG_SRGB_INTENT_PERCEPTUAL,
};
use relativity_trace::{
    build_celestial_coordinate_frame, trace_grid_with_execution_and_surface_set, TraceGrid,
    TraceSurfaceSet,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppearancePresetFile {
    schema_version: u32,
    model_id: String,
    #[allow(dead_code)]
    description: Option<String>,
    disk: DiskSection,
    environment: EnvironmentSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiskSection {
    model_id: String,
    radial_envelope_id: String,
    mean_preservation_claim: String,
    a_max: f64,
    r_ref_over_m: f64,
    identity_modulation: bool,
    modes: Vec<ModeSection>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeSection {
    m: u32,
    weight: f64,
    k_log: f64,
    phase: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentSection {
    model_id: String,
    sky_floor: f64,
    identity_black: bool,
    rotation: RotationSection,
    milky_way: MilkyWaySection,
    stars: StarsSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RotationSection {
    w: f64,
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MilkyWaySection {
    label: String,
    pole: [f64; 3],
    band_sigma_rad: f64,
    band_peak: f64,
    core_sigma_rad: f64,
    core_peak: f64,
    dust_sigma_rad: f64,
    dust_depth: f64,
    longitude_modulation_amp: f64,
    longitude_harmonics: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StarsSection {
    profile_id: String,
    algorithm_id: String,
    seed: u64,
    count: u32,
    angular_sigma_rad: f64,
    peak_scale: f64,
    band_bias: f64,
    t_min_k: f64,
    t_max_k: f64,
}

#[derive(Serialize)]
pub struct SceneAppearanceMeta {
    pub gate: &'static str,
    pub authority: &'static str,
    pub appearance_role: &'static str,
    pub mean_preservation_claim: &'static str,
    pub source_physical_color_digest: String,
    pub source_payload_sha256: String,
    pub source_physical_emission_digest: String,
    pub source_frequency_digest: String,
    pub disk_appearance_spec_digest: String,
    pub appearance_disk_color_digest: String,
    pub environment_spec_digest: String,
    pub scene_appearance_digest: String,
    pub presentation_spec_digest: String,
    pub presentation_frame_digest: String,
    pub identity_scene: bool,
    pub celestial_convention: &'static str,
    pub width: u32,
    pub height: u32,
    pub beauty_png: &'static str,
    pub disk_hit_count: u64,
    pub escaped_count: u64,
    pub horizon_count: u64,
    pub integrated_luma_appearance: f64,
    pub integrated_luma_base_disk: f64,
    pub integrated_luma_relative_change: f64,
    pub metrics: relativity_render::PresentationMetrics,
}

#[derive(Serialize, Deserialize)]
pub struct SceneAppearanceReport {
    pub gate: String,
    pub result_hint: String,
    pub source_physical_color_digest: String,
    pub source_payload_sha256: String,
    pub presentation_spec_digest: String,
    pub presentation_frame_digest: String,
    pub scene_appearance_digest: String,
    pub disk_appearance_spec_digest: String,
    pub environment_spec_digest: String,
    pub identity_scene: bool,
    pub png_bytes: u64,
    pub png_roundtrip_ok: bool,
    pub build: BuildExecutionMetadata,
    pub trace_wall_clock_seconds: f64,
    pub appearance_wall_clock_seconds: f64,
    pub presentation_wall_clock_seconds: f64,
    pub total_wall_clock_seconds: f64,
    pub metrics: relativity_render::PresentationMetrics,
    pub note: String,
}

pub fn load_appearance_specs(
    path: &Path,
) -> Result<(DiskAppearanceSpec, EnvironmentSpec), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let file: AppearancePresetFile = toml::from_str(&text)?;
    if file.schema_version != 1 || file.model_id != "scene-appearance-v1" {
        return Err("appearance preset schema/model mismatch".into());
    }
    let disk = DiskAppearanceSpec {
        model_id: file.disk.model_id,
        radial_envelope_id: file.disk.radial_envelope_id,
        mean_preservation_claim: file.disk.mean_preservation_claim,
        a_max: file.disk.a_max,
        r_ref_over_m: file.disk.r_ref_over_m,
        modes: file
            .disk
            .modes
            .into_iter()
            .map(|m| SpiralHarmonicMode {
                m: m.m,
                weight: m.weight,
                k_log: m.k_log,
                phase: m.phase,
            })
            .collect(),
        identity_modulation: file.disk.identity_modulation,
    };
    disk.validate()
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let env = EnvironmentSpec {
        model_id: file.environment.model_id,
        environment_rotation: UnitQuaternion {
            w: file.environment.rotation.w,
            x: file.environment.rotation.x,
            y: file.environment.rotation.y,
            z: file.environment.rotation.z,
        },
        sky_floor: file.environment.sky_floor,
        milky_way: MilkyWayLikeSpec {
            label: file.environment.milky_way.label,
            pole: file.environment.milky_way.pole,
            band_sigma_rad: file.environment.milky_way.band_sigma_rad,
            band_peak: file.environment.milky_way.band_peak,
            core_sigma_rad: file.environment.milky_way.core_sigma_rad,
            core_peak: file.environment.milky_way.core_peak,
            dust_sigma_rad: file.environment.milky_way.dust_sigma_rad,
            dust_depth: file.environment.milky_way.dust_depth,
            longitude_modulation_amp: file.environment.milky_way.longitude_modulation_amp,
            longitude_harmonics: file.environment.milky_way.longitude_harmonics,
        },
        stars: StarsSpec {
            profile_id: file.environment.stars.profile_id,
            seed: file.environment.stars.seed,
            algorithm_id: file.environment.stars.algorithm_id,
            count: file.environment.stars.count,
            angular_sigma_rad: file.environment.stars.angular_sigma_rad,
            peak_scale: file.environment.stars.peak_scale,
            band_bias: file.environment.stars.band_bias,
            t_min_k: file.environment.stars.t_min_k,
            t_max_k: file.environment.stars.t_max_k,
        },
        identity_black: file.environment.identity_black,
    };
    env.validate()
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    Ok((disk, env))
}

pub struct RenderedSceneAppearance {
    pub report: SceneAppearanceReport,
    pub scene_frame: relativity_render::SceneAppearanceFrame,
    pub presented: relativity_render::PresentationFrame,
    pub source_physical_color_digest: String,
    #[allow(dead_code)]
    pub camera_spec_digest: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    preset_path: &str,
    appearance_path: &str,
    presentation_path: &str,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
    output_dir: &str,
    require_release: bool,
    execution: CliExecution,
    threads: Option<usize>,
    write_env_reference: bool,
    visual_semantic_diagnostics: bool,
    camera_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rendered = render(
        preset_path,
        appearance_path,
        presentation_path,
        tier,
        width,
        height,
        output_dir,
        require_release,
        execution,
        threads,
        write_env_reference,
        visual_semantic_diagnostics,
        camera_path,
        true,
    )?;
    println!("{}", serde_json::to_string_pretty(&rendered.report)?);
    Ok(())
}

/// Core scene-appearance render. When `camera_path` is set, applies C2 overlay (D3A-A1).
#[allow(clippy::too_many_arguments)]
pub fn render(
    preset_path: &str,
    appearance_path: &str,
    presentation_path: &str,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
    output_dir: &str,
    require_release: bool,
    execution: CliExecution,
    threads: Option<usize>,
    write_env_reference: bool,
    visual_semantic_diagnostics: bool,
    camera_path: Option<&str>,
    print_suppressed: bool,
) -> Result<RenderedSceneAppearance, Box<dyn std::error::Error>> {
    let _ = print_suppressed;
    let t0 = Instant::now();
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }

    let plan = resolve_render_plan(tier, width, height)?;
    let trace_execution = resolve_execution(execution, threads)?;
    let root = workspace_root()?;
    let out_dir = resolve_path(&root, output_dir);
    let mut preset = load_preset(&resolve_path(&root, preset_path))?;
    let camera_spec_digest = if let Some(cam_path) = camera_path {
        let cam = crate::camera_composition::load_camera_composition_preset(&resolve_path(
            &root, cam_path,
        ))?;
        let digest = crate::camera_composition::camera_spec_digest(&cam);
        preset = crate::camera_composition::apply_camera_overlay(&preset, &cam)?;
        Some(digest)
    } else {
        None
    };
    let presentation_spec = load_presentation_spec(&resolve_path(&root, presentation_path))?;
    let (disk_spec, env_spec) = load_appearance_specs(&resolve_path(&root, appearance_path))?;
    validate_physical_emission_provenance(&preset.disk.emission_model, &preset.disk.emission_claim)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let physical = preset
        .physical
        .as_ref()
        .ok_or("preset missing [physical] section")?;
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
    let _ = grid_digest;

    let rgb_matrix = XyzToRgbMatrix::rec709_d65_linear_v1();
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
    let color_digest = physical_color_digest(&color)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let payload = encode_physical_color_payload(&color)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    verify_payload_matches_frame(&payload, &color)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let payload_digest = payload_sha256(&payload);

    let t_app = Instant::now();
    let disk_spec_digest = disk_appearance_spec_digest(&disk_spec)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let app_emission =
        build_appearance_disk_emission_frame(&emission, &disk_spec, &emission_digest)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let app_color = build_appearance_disk_color_frame(
        &app_emission,
        &cie,
        &rgb_matrix,
        IntegrationMeasure::FrequencyNu,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let app_color_digest = appearance_disk_color_digest(&app_color);

    let celestial = build_celestial_coordinate_frame(&scene.kerr, &bundle)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let env_digest = environment_spec_digest(&env_spec)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let environment = build_celestial_environment(&env_spec)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    let scene_frame = build_scene_appearance_frame(
        &bundle,
        &color,
        &app_color,
        &celestial,
        &environment,
        &env_digest,
        &presentation_spec,
    )
    .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let appearance_wall = t_app.elapsed().as_secs_f64();

    let identity_scene = disk_spec.identity_modulation && env_spec.identity_black;
    let presentation_source = if identity_scene {
        color_digest.clone()
    } else {
        scene_frame.scene_appearance_digest.clone()
    };

    let t_pres = Instant::now();
    let presented =
        present_scene_appearance_frame(&scene_frame, &presentation_spec, &presentation_source)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let presentation_wall = t_pres.elapsed().as_secs_f64();

    if identity_scene {
        let baseline = present_physical_color_frame(&color, &presentation_spec)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let a = authored_rgb16_bytes(&presented.pixels);
        let b = authored_rgb16_bytes(&baseline.pixels);
        if a != b {
            return Err("A5 identity scene RGB16 raster != Gate 2D0 baseline".into());
        }
        if presented.presentation_frame_digest != baseline.presentation_frame_digest {
            return Err("A5 identity presentation_frame_digest != Gate 2D0 baseline".into());
        }
    }

    let png_path = out_dir.join("beauty-scene-srgb16.png");
    let png_bytes = write_beauty_png(&png_path, &presented)?;
    let roundtrip = verify_beauty_png(&png_path, &presented)?;

    if write_env_reference && !env_spec.identity_black {
        let ref_lin = render_environment_reference(&environment, 256, 128)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        // Encode reference through same post-exposure path at EV=0 using a temporary presentation.
        let mut exposed = Vec::with_capacity(ref_lin.len());
        for rgb in ref_lin {
            exposed.push(relativity_render::ExposedLinearPixel::ExposedLinear {
                rgb,
                count_as_lit: false,
            });
        }
        let ref_pres = relativity_render::present_exposed_linear_rgb(
            256,
            128,
            &exposed,
            presentation_spec.gamut_operator()?,
            presentation_spec.tone_operator()?,
            "environment-reference",
            &presented.presentation_spec_digest,
        )
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        write_beauty_png(&out_dir.join("environment-reference-srgb16.png"), &ref_pres)?;
    }

    if visual_semantic_diagnostics && !identity_scene {
        crate::d1_v1_visual_semantic::write_visual_semantic_diagnostics(
            &out_dir,
            &scene_frame,
            &bundle,
            &app_emission,
            &app_color,
            &presentation_spec,
            None,
        )?;
    }

    let rel_luma = if scene_frame.integrated_luma_base_disk > 0.0 {
        (scene_frame.integrated_luma_appearance - scene_frame.integrated_luma_base_disk)
            / scene_frame.integrated_luma_base_disk
    } else {
        0.0
    };

    let (intent, gama) = png_metadata_constants();
    let _ = (
        intent,
        gama,
        PNG_FORMAT_RGB16_SRGB_V1,
        PNG_SRGB_INTENT_PERCEPTUAL,
        PNG_GAMA_SRGB,
    );

    let meta = SceneAppearanceMeta {
        gate: "gate-2d1-scene-appearance",
        authority: "APPEARANCE_REPRODUCIBILITY_DIGEST",
        appearance_role: "derived appearance + artistic environment; not scientific radiance",
        mean_preservation_claim: "ANNULAR_APPEARANCE_MEAN_PRESERVING",
        source_physical_color_digest: color_digest.clone(),
        source_payload_sha256: payload_digest.clone(),
        source_physical_emission_digest: emission_digest.clone(),
        source_frequency_digest: freq_digest.clone(),
        disk_appearance_spec_digest: disk_spec_digest.clone(),
        appearance_disk_color_digest: app_color_digest,
        environment_spec_digest: env_digest.clone(),
        scene_appearance_digest: scene_frame.scene_appearance_digest.clone(),
        presentation_spec_digest: presented.presentation_spec_digest.clone(),
        presentation_frame_digest: presented.presentation_frame_digest.clone(),
        identity_scene,
        celestial_convention: "finite-oblate-ks-boundary-uv-v1",
        width: presented.width,
        height: presented.height,
        beauty_png: "beauty-scene-srgb16.png",
        disk_hit_count: scene_frame.disk_hit_count,
        escaped_count: scene_frame.escaped_count,
        horizon_count: scene_frame.horizon_count,
        integrated_luma_appearance: scene_frame.integrated_luma_appearance,
        integrated_luma_base_disk: scene_frame.integrated_luma_base_disk,
        integrated_luma_relative_change: rel_luma,
        metrics: presented.metrics.clone(),
    };
    std::fs::write(
        out_dir.join("scene-appearance-meta.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    let report = SceneAppearanceReport {
        gate: "gate-2d1-scene-appearance".into(),
        result_hint: "local-render".into(),
        source_physical_color_digest: color_digest.clone(),
        source_payload_sha256: payload_digest,
        presentation_spec_digest: presented.presentation_spec_digest.clone(),
        presentation_frame_digest: presented.presentation_frame_digest.clone(),
        scene_appearance_digest: scene_frame.scene_appearance_digest.clone(),
        disk_appearance_spec_digest: disk_spec_digest,
        environment_spec_digest: env_digest,
        identity_scene,
        png_bytes,
        png_roundtrip_ok: roundtrip,
        build,
        trace_wall_clock_seconds: trace_wall,
        appearance_wall_clock_seconds: appearance_wall,
        presentation_wall_clock_seconds: presentation_wall,
        total_wall_clock_seconds: t0.elapsed().as_secs_f64(),
        metrics: presented.metrics.clone(),
        note: if camera_spec_digest.is_some() {
            "Gate 2D3A camera-overlaid scene appearance; hero digests are camera-derived production outputs, not new 2C1 scientific authority (D3A-A2)".into()
        } else {
            "Gate 2D1 appearance beauty; scientific channels remain Gate 2C0/2C1 authority".into()
        },
    };
    std::fs::write(
        out_dir.join("appearance-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;

    Ok(RenderedSceneAppearance {
        report,
        scene_frame,
        presented,
        source_physical_color_digest: color_digest,
        camera_spec_digest,
    })
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
