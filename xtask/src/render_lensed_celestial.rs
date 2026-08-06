//! Gate 2A2: trace once → Gate 2A1 coordinates → procedural celestial → lensed PPM.

use crate::build_meta::{
    require_release_execution, write_build_execution_report, BuildExecutionMetadata,
};
use crate::diagnostic_scene::{build_diagnostic_trace_scene, DiagnosticNumericalProfile};
use crate::preset::load_preset;
use crate::render_tier::{
    resolve_render_plan, DiagnosticRenderTier, RenderAuthorityClass, ResolutionSource,
};
use crate::trace_outcome_map::{
    resolve_execution, write_trace_execution_report, CliExecution, TRACE_EXECUTION_FILENAME,
};
use relativity_core::EquatorialAngularDirection;
use relativity_render::{
    bolometric_debug_display_spec_digest, bolometric_debug_display_v1,
    bolometric_display_range_counts, build_disk_bolometric_frame,
    build_disk_bolometric_map_artifact, build_disk_frequency_shift_frame,
    build_disk_frequency_shift_map_artifact, diagnostic_bolometric_emission_spec_digest,
    diagnostic_bolometric_emission_v1, g_visualization_range_counts, procedural_coordinate_grid_v1,
    procedural_texture_spec_digest, render_bolometric_celestial_composite, render_lensed_celestial,
    shade_emitted_bolometric_debug, shade_g_factor_debug, shade_observed_bolometric_debug,
    validate_disk_emission_provenance, validate_mode_surface_set, verify_disk_bolometric_frame,
    verify_lensed_celestial_frame, verify_observer_unit_frequency, BolometricDebugDisplaySpec,
    BolometricRegressionSample, BolometricRenderError, DiagnosticBolometricEmissionSpec,
    DiskBolometricConvention, DiskFrequencyShiftConvention, DiskVelocityModel, FrequencyShiftError,
    FrequencyShiftRegressionSample, LensedCelestialMode, ProceduralCelestialTextureSpec,
    RankedBolometricPixel, RankedFrequencyShiftPixel, ResolvedDiskBounds,
    CANONICAL_DISK_EMISSION_CLAIM, CANONICAL_DISK_EMISSION_MODEL, DISK_BOUNDS_SOURCE_V1,
    TEXTURE_ID_V1,
};
use relativity_trace::{
    build_celestial_coordinate_frame, build_celestial_coordinate_map_artifact, encode_ppm, hex_sha,
    outcome_class_bytes, shade_celestial_uv_debug, shade_diagnostic, trace_data_digest,
    trace_grid_with_execution_and_surface_set, validate_celestial_seam, write_rhs_pgm,
    CelestialCoordinateConvention, DiagnosticShadeStyle, OutcomeCounts, TraceExecutionMetadata,
    TraceGrid, TraceSurfaceSet, RADIUS_POLICY_GATE_1B2_CAP,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LensedCelestialReport {
    pub schema_version: u32,
    pub width: u32,
    pub height: u32,
    pub render_tier: Option<DiagnosticRenderTier>,
    pub resolution_source: ResolutionSource,
    pub authority_class: RenderAuthorityClass,
    pub build: BuildExecutionMetadata,
    pub execution: TraceExecutionMetadata,
    pub surface_set: TraceSurfaceSet,
    pub mode: LensedCelestialMode,
    pub trace_invocations: u32,
    pub coordinate_passes: u32,
    pub texture_render_passes: u32,
    pub numerical_profile: DiagnosticNumericalProfile,
    pub numerical_profile_digest: String,
    pub trace_data_digest: String,
    pub outcome_class_digest: String,
    pub coordinate_digest: String,
    pub texture_spec: ProceduralCelestialTextureSpec,
    pub texture_spec_digest: String,
    pub texture_sample_count: u64,
    pub non_escaped_count: u64,
    pub outcome_counts: OutcomeCounts,
    pub lensed_ppm_filename: String,
    pub lensed_ppm_digest: String,
    pub categorical_ppm_digest: String,
    pub rhs_pgm_digest: String,
    pub coordinate_json_digest: String,
    pub uv_debug_ppm_digest: String,
    pub convention_id: String,
    pub resolved_boundary_radius: f64,
    pub radius_policy: String,
    pub mapping_failure_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_frequency_shift: Option<DiskFrequencyShiftOutputReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_bolometric_radiance: Option<DiskBolometricOutputReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_wall_clock_seconds: Option<f64>,
    pub content_digest_excluding_digest_field: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskBolometricOutputReport {
    pub bolometric_emission_passes: u32,
    pub bolometric_transport_passes: u32,
    pub bolometric_visualization_passes: u32,
    pub convention: DiskBolometricConvention,
    pub emission_spec: DiagnosticBolometricEmissionSpec,
    pub emission_spec_digest: String,
    pub accepted_emission_model: String,
    pub accepted_emission_claim: String,
    pub display_spec: BolometricDebugDisplaySpec,
    pub display_spec_digest: String,
    pub resolved_disk_bounds: ResolvedDiskBounds,
    pub disk_bounds_source: String,
    pub source_frequency_shift_digest: String,
    pub disk_hit_count: u64,
    pub mapped_count: u64,
    pub mapping_failure_count: u64,
    pub attenuated_count: u64,
    pub boosted_count: u64,
    pub unchanged_count: u64,
    pub minimum_emitted: Option<RankedBolometricPixel>,
    pub maximum_emitted: Option<RankedBolometricPixel>,
    pub minimum_observed: Option<RankedBolometricPixel>,
    pub maximum_observed: Option<RankedBolometricPixel>,
    pub minimum_transport_factor: Option<RankedBolometricPixel>,
    pub maximum_transport_factor: Option<RankedBolometricPixel>,
    pub maximum_abs_transport_residual: f64,
    pub bolometric_digest: String,
    pub bolometric_json_digest: String,
    pub emitted_debug_ppm_digest: String,
    pub observed_debug_ppm_digest: String,
    pub composite_ppm_digest: String,
    pub emitted_below_range_count: u64,
    pub emitted_above_range_count: u64,
    pub observed_below_range_count: u64,
    pub observed_above_range_count: u64,
    pub regression_corpus: Vec<BolometricRegressionSample>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emission_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visualization_wall_clock_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskFrequencyShiftOutputReport {
    pub observer_frequency_verification_passes: u32,
    pub frequency_shift_passes: u32,
    pub convention: DiskFrequencyShiftConvention,
    pub velocity_model: DiskVelocityModel,
    pub resolved_direction: EquatorialAngularDirection,
    pub disk_hit_count: u64,
    pub mapped_count: u64,
    pub mapping_failure_count: u64,
    pub redshifted_count: u64,
    pub blueshifted_count: u64,
    pub exact_unity_count: u64,
    pub minimum_g: Option<RankedFrequencyShiftPixel>,
    pub maximum_g: Option<RankedFrequencyShiftPixel>,
    pub closest_to_unity: Option<RankedFrequencyShiftPixel>,
    pub maximum_abs_disk_radius_residual: f64,
    pub maximum_observer_unit_frequency_residual: f64,
    pub frequency_shift_digest: String,
    pub frequency_shift_json_digest: String,
    pub g_factor_debug_ppm_digest: String,
    pub below_visualization_range_count: u64,
    pub above_visualization_range_count: u64,
    pub regression_corpus: Vec<FrequencyShiftRegressionSample>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping_wall_clock_seconds: Option<f64>,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    preset_path: &str,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
    surface_set: TraceSurfaceSet,
    mode: LensedCelestialMode,
    texture_id: &str,
    output_dir: &str,
    require_release: bool,
    execution: CliExecution,
    threads: Option<usize>,
    emit_disk_frequency_shift: bool,
    emit_disk_bolometric_radiance: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }

    validate_mode_surface_set(mode, surface_set)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    if emit_disk_bolometric_radiance && !emit_disk_frequency_shift {
        return Err(BolometricRenderError::FlagRequiresFrequencyShift
            .to_string()
            .into());
    }
    if (emit_disk_frequency_shift || emit_disk_bolometric_radiance)
        && !(surface_set == TraceSurfaceSet::OpaqueDiskHorizonEscape
            && mode == LensedCelestialMode::OpaqueDiskMask)
    {
        if emit_disk_bolometric_radiance {
            return Err(BolometricRenderError::FlagSurfaceModeMismatch
                .to_string()
                .into());
        }
        return Err(FrequencyShiftError::FlagSurfaceModeMismatch
            .to_string()
            .into());
    }
    if texture_id != TEXTURE_ID_V1 {
        return Err(format!(
            "unsupported --texture `{texture_id}`; only `{TEXTURE_ID_V1}` is accepted"
        )
        .into());
    }
    let texture_spec = procedural_coordinate_grid_v1();
    texture_spec
        .validate()
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    let texture_spec_digest = procedural_texture_spec_digest(&texture_spec);

    let plan = resolve_render_plan(tier, width, height)?;
    let trace_execution = resolve_execution(execution, threads)?;
    let exec_meta = trace_execution.metadata();

    let root = workspace_root()?;
    let out_dir = if Path::new(output_dir).is_absolute() {
        PathBuf::from(output_dir)
    } else {
        root.join(output_dir)
    };

    let preset_full = if Path::new(preset_path).is_absolute() {
        PathBuf::from(preset_path)
    } else {
        root.join(preset_path)
    };
    let _preset_digest = hex::encode(Sha256::digest(std::fs::read(&preset_full)?));
    let preset = load_preset(&preset_full)?;

    if emit_disk_bolometric_radiance {
        validate_disk_emission_provenance(&preset.disk.emission_model, &preset.disk.emission_claim)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    }

    if preset.celestial_sphere.texture != "procedural_coordinate_grid" {
        return Err(format!(
            "preset celestial_sphere.texture must be `procedural_coordinate_grid`, got `{}`",
            preset.celestial_sphere.texture
        )
        .into());
    }
    validate_celestial_seam(&preset.celestial_sphere.seam)?;

    std::fs::create_dir_all(&out_dir)?;

    let (scene, numerical_profile) = build_diagnostic_trace_scene(
        &preset,
        TraceGrid {
            width: plan.width,
            height: plan.height,
        },
    )?;
    let resolved_disk_bounds = ResolvedDiskBounds::new(scene.disk.r_inner, scene.disk.r_outer)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

    // ---- Phase 1: trace exactly once with selected surface set ----
    let t_trace = Instant::now();
    let bundle = trace_grid_with_execution_and_surface_set(&scene, trace_execution, surface_set)?;
    let trace_wall = t_trace.elapsed().as_secs_f64();
    let trace_invocations = 1u32;

    for o in &bundle.outcomes {
        if !matches!(o, relativity_trace::RayOutcome::Failed(_)) && !o.state_finite() {
            return Err("non-finite success state in TraceBundle".into());
        }
    }

    let (counts, _, _, _) = summarize_bundle(&bundle);
    if mode == LensedCelestialMode::DiskOmittedDiagnostic && counts.disk_hit != 0 {
        return Err(format!(
            "disk-omitted diagnostic produced {} DiskHit outcomes",
            counts.disk_hit
        )
        .into());
    }
    if counts.failed != 0 {
        return Err(format!("failed ray count must be zero, got {}", counts.failed).into());
    }

    let data_digest = trace_data_digest(&bundle);
    let class_digest = hex_sha(&outcome_class_bytes(&bundle));
    let pgm = write_rhs_pgm(&bundle);
    let pgm_digest = hex_sha(&pgm);
    std::fs::write(out_dir.join("rhs-evaluations.pgm"), &pgm)?;

    let categorical = shade_diagnostic(&bundle, DiagnosticShadeStyle::Gate1b2Categorical);
    let categorical_ppm = encode_ppm(&categorical);
    let categorical_ppm_digest = hex_sha(&categorical_ppm);
    std::fs::write(out_dir.join("gate1b2-categorical.ppm"), &categorical_ppm)?;

    // ---- Phase 2: one celestial coordinate pass ----
    let t_map = Instant::now();
    let convention = CelestialCoordinateConvention::finite_oblate_ks_boundary_uv_v1();
    let frame = build_celestial_coordinate_frame(&scene.kerr, &bundle)?;
    let art = build_celestial_coordinate_map_artifact(
        &frame,
        &convention,
        preset.celestial_sphere.radius_m,
        scene.escape_radius,
    );
    if art.mapping_failure_count != 0 {
        return Err("celestial mapping_failure_count != 0".into());
    }
    if art.escaped_count != art.mapped_count {
        return Err("celestial mapped_count != escaped_count".into());
    }
    if art.escaped_count != counts.escaped {
        return Err("celestial escaped_count disagrees with outcome_counts.escaped".into());
    }
    let mapping_wall = t_map.elapsed().as_secs_f64();
    let coordinate_passes = 1u32;

    let json_bytes = serde_json::to_vec_pretty(&art)?;
    let coordinate_json_digest = hex_sha(&json_bytes);
    std::fs::write(out_dir.join("celestial-coordinate-map.json"), &json_bytes)?;

    let uv_frame = shade_celestial_uv_debug(&frame);
    let uv_ppm = encode_ppm(&uv_frame);
    let uv_debug_ppm_digest = hex_sha(&uv_ppm);
    std::fs::write(out_dir.join("celestial-uv-debug.ppm"), &uv_ppm)?;

    // ---- Phase 3: one procedural texture render pass ----
    let t_render = Instant::now();
    let lensed = render_lensed_celestial(&frame, &texture_spec, mode)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    verify_lensed_celestial_frame(&frame, &texture_spec, mode, &lensed.frame)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
    if lensed.texture_sample_count != counts.escaped {
        return Err(format!(
            "texture_sample_count {} != escaped {}",
            lensed.texture_sample_count, counts.escaped
        )
        .into());
    }
    let render_wall = t_render.elapsed().as_secs_f64();
    let texture_render_passes = 1u32;

    let lensed_ppm_filename = mode.ppm_filename().to_string();
    let lensed_ppm = encode_ppm(&lensed.frame);
    debug_assert_eq!(hex_sha(&lensed_ppm), lensed.ppm_digest);
    std::fs::write(out_dir.join(&lensed_ppm_filename), &lensed_ppm)?;

    // ---- Phase 4 (optional): observer ν verification + disk frequency-shift ----
    let (disk_frequency_shift, fs_frame_for_bolo) = if emit_disk_frequency_shift {
        let t_ver = Instant::now();
        let verification = verify_observer_unit_frequency(&scene.kerr, &scene)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let verification_wall = t_ver.elapsed().as_secs_f64();

        let t_fs = Instant::now();
        let fs_frame = build_disk_frequency_shift_frame(
            &scene.kerr,
            &bundle,
            DiskVelocityModel::ProgradeCircularGeodesic,
        )
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let fs_convention = DiskFrequencyShiftConvention::v1();
        let fs_art =
            build_disk_frequency_shift_map_artifact(&fs_frame, &fs_convention, verification);
        if fs_art.mapping_failure_count != 0 {
            return Err("frequency-shift mapping_failure_count != 0".into());
        }
        if fs_art.mapped_count != fs_art.disk_hit_count {
            return Err("frequency-shift mapped_count != disk_hit_count".into());
        }
        if fs_art.disk_hit_count != counts.disk_hit {
            return Err("frequency-shift disk_hit_count disagrees with outcome_counts".into());
        }
        let mapping_fs_wall = t_fs.elapsed().as_secs_f64();

        let fs_json = serde_json::to_vec_pretty(&fs_art)?;
        let frequency_shift_json_digest = hex_sha(&fs_json);
        std::fs::write(out_dir.join("disk-frequency-shift-map.json"), &fs_json)?;

        let g_frame = shade_g_factor_debug(&fs_frame);
        let g_ppm = encode_ppm(&g_frame);
        let g_factor_debug_ppm_digest = hex_sha(&g_ppm);
        std::fs::write(out_dir.join("g-factor-debug.ppm"), &g_ppm)?;
        let (below, above) = g_visualization_range_counts(&fs_frame);

        let resolved_direction = fs_frame
            .pixels()
            .iter()
            .find_map(|p| match p {
                relativity_render::DiskFrequencyShiftPixel::DiskHit(s) => {
                    Some(s.resolved_direction)
                }
                _ => None,
            })
            .unwrap_or_else(|| relativity_core::prograde_equatorial_direction(&scene.kerr));

        let report = DiskFrequencyShiftOutputReport {
            observer_frequency_verification_passes: 1,
            frequency_shift_passes: 1,
            convention: fs_convention,
            velocity_model: DiskVelocityModel::ProgradeCircularGeodesic,
            resolved_direction,
            disk_hit_count: fs_art.disk_hit_count,
            mapped_count: fs_art.mapped_count,
            mapping_failure_count: fs_art.mapping_failure_count,
            redshifted_count: fs_art.redshifted_count,
            blueshifted_count: fs_art.blueshifted_count,
            exact_unity_count: fs_art.exact_unity_count,
            minimum_g: fs_art.minimum_g.clone(),
            maximum_g: fs_art.maximum_g.clone(),
            closest_to_unity: fs_art.closest_to_unity.clone(),
            maximum_abs_disk_radius_residual: fs_art.maximum_abs_disk_radius_residual,
            maximum_observer_unit_frequency_residual: verification.maximum_residual,
            frequency_shift_digest: fs_art.frequency_shift_digest.clone(),
            frequency_shift_json_digest,
            g_factor_debug_ppm_digest,
            below_visualization_range_count: below,
            above_visualization_range_count: above,
            regression_corpus: fs_art.regression_corpus.clone(),
            verification_wall_clock_seconds: Some(verification_wall),
            mapping_wall_clock_seconds: Some(mapping_fs_wall),
        };
        (Some(report), Some(fs_frame))
    } else {
        (None, None)
    };

    // ---- Phase 5 (optional): diagnostic bolometric emission + g⁴ transport ----
    let disk_bolometric_radiance = if emit_disk_bolometric_radiance {
        let fs_frame = fs_frame_for_bolo
            .as_ref()
            .ok_or("bolometric requires frequency-shift frame")?;
        let source_frequency_shift_digest = disk_frequency_shift
            .as_ref()
            .map(|f| f.frequency_shift_digest.clone())
            .ok_or("bolometric requires frequency-shift digest")?;

        let emission_spec = diagnostic_bolometric_emission_v1();
        emission_spec
            .validate()
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let display_spec = bolometric_debug_display_v1();
        display_spec
            .validate()
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

        let t_em = Instant::now();
        let bolo_frame =
            build_disk_bolometric_frame(fs_frame, &emission_spec, resolved_disk_bounds)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let emission_wall = t_em.elapsed().as_secs_f64();

        let t_tr = Instant::now();
        verify_disk_bolometric_frame(fs_frame, &bolo_frame, &emission_spec, resolved_disk_bounds)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let bolo_convention = DiskBolometricConvention::v1();
        let bolo_art = build_disk_bolometric_map_artifact(
            &bolo_frame,
            &bolo_convention,
            &emission_spec,
            resolved_disk_bounds,
            &source_frequency_shift_digest,
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        if bolo_art.mapping_failure_count != 0 {
            return Err("bolometric mapping_failure_count != 0".into());
        }
        if bolo_art.mapped_count != bolo_art.disk_hit_count {
            return Err("bolometric mapped_count != disk_hit_count".into());
        }
        if bolo_art.disk_hit_count != counts.disk_hit {
            return Err("bolometric disk_hit_count disagrees with outcome_counts".into());
        }
        let transport_wall = t_tr.elapsed().as_secs_f64();

        let bolo_json = serde_json::to_vec_pretty(&bolo_art)?;
        let bolometric_json_digest = hex_sha(&bolo_json);
        std::fs::write(
            out_dir.join("disk-bolometric-radiance-map.json"),
            &bolo_json,
        )?;

        let t_viz = Instant::now();
        let emitted_frame = shade_emitted_bolometric_debug(&bolo_frame, &display_spec)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let emitted_ppm = encode_ppm(&emitted_frame);
        let emitted_debug_ppm_digest = hex_sha(&emitted_ppm);
        std::fs::write(out_dir.join("emitted-bolometric-debug.ppm"), &emitted_ppm)?;

        let observed_frame = shade_observed_bolometric_debug(&bolo_frame, &display_spec)
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let observed_ppm = encode_ppm(&observed_frame);
        let observed_debug_ppm_digest = hex_sha(&observed_ppm);
        std::fs::write(out_dir.join("observed-bolometric-debug.ppm"), &observed_ppm)?;

        let composite = render_bolometric_celestial_composite(
            &frame,
            &bolo_frame,
            &texture_spec,
            &display_spec,
        )
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let composite_ppm = encode_ppm(&composite);
        let composite_ppm_digest = hex_sha(&composite_ppm);
        std::fs::write(
            out_dir.join("bolometric-disk-celestial-composite.ppm"),
            &composite_ppm,
        )?;
        let visualization_wall = t_viz.elapsed().as_secs_f64();

        let (emitted_below, emitted_above) =
            bolometric_display_range_counts(&bolo_frame, &display_spec, false)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
        let (observed_below, observed_above) =
            bolometric_display_range_counts(&bolo_frame, &display_spec, true)
                .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

        Some(DiskBolometricOutputReport {
            bolometric_emission_passes: 1,
            bolometric_transport_passes: 1,
            bolometric_visualization_passes: 3,
            convention: bolo_convention,
            emission_spec_digest: diagnostic_bolometric_emission_spec_digest(&emission_spec),
            emission_spec,
            accepted_emission_model: CANONICAL_DISK_EMISSION_MODEL.into(),
            accepted_emission_claim: CANONICAL_DISK_EMISSION_CLAIM.into(),
            display_spec_digest: bolometric_debug_display_spec_digest(&display_spec),
            display_spec,
            resolved_disk_bounds,
            disk_bounds_source: DISK_BOUNDS_SOURCE_V1.into(),
            source_frequency_shift_digest,
            disk_hit_count: bolo_art.disk_hit_count,
            mapped_count: bolo_art.mapped_count,
            mapping_failure_count: bolo_art.mapping_failure_count,
            attenuated_count: bolo_art.attenuated_count,
            boosted_count: bolo_art.boosted_count,
            unchanged_count: bolo_art.unchanged_count,
            minimum_emitted: bolo_art.minimum_emitted.clone(),
            maximum_emitted: bolo_art.maximum_emitted.clone(),
            minimum_observed: bolo_art.minimum_observed.clone(),
            maximum_observed: bolo_art.maximum_observed.clone(),
            minimum_transport_factor: bolo_art.minimum_transport_factor.clone(),
            maximum_transport_factor: bolo_art.maximum_transport_factor.clone(),
            maximum_abs_transport_residual: bolo_art.maximum_abs_transport_residual,
            bolometric_digest: bolo_art.bolometric_digest.clone(),
            bolometric_json_digest,
            emitted_debug_ppm_digest,
            observed_debug_ppm_digest,
            composite_ppm_digest,
            emitted_below_range_count: emitted_below,
            emitted_above_range_count: emitted_above,
            observed_below_range_count: observed_below,
            observed_above_range_count: observed_above,
            regression_corpus: bolo_art.regression_corpus.clone(),
            emission_wall_clock_seconds: Some(emission_wall),
            transport_wall_clock_seconds: Some(transport_wall),
            visualization_wall_clock_seconds: Some(visualization_wall),
        })
    } else {
        None
    };

    write_build_execution_report(&out_dir, &build)?;
    write_trace_execution_report(&out_dir, &exec_meta)?;

    let mut report = LensedCelestialReport {
        schema_version: 1,
        width: plan.width,
        height: plan.height,
        render_tier: plan.tier,
        resolution_source: plan.resolution_source,
        authority_class: plan.authority_class,
        build,
        execution: exec_meta,
        surface_set,
        mode,
        trace_invocations,
        coordinate_passes,
        texture_render_passes,
        numerical_profile_digest: numerical_profile.digest.clone(),
        numerical_profile,
        trace_data_digest: data_digest,
        outcome_class_digest: class_digest,
        coordinate_digest: art.coordinate_digest.clone(),
        texture_spec,
        texture_spec_digest,
        texture_sample_count: lensed.texture_sample_count,
        non_escaped_count: lensed.non_escaped_count,
        outcome_counts: counts,
        lensed_ppm_filename,
        lensed_ppm_digest: lensed.ppm_digest.clone(),
        categorical_ppm_digest,
        rhs_pgm_digest: pgm_digest,
        coordinate_json_digest,
        uv_debug_ppm_digest,
        convention_id: convention.convention_id.clone(),
        resolved_boundary_radius: scene.escape_radius,
        radius_policy: RADIUS_POLICY_GATE_1B2_CAP.into(),
        mapping_failure_count: art.mapping_failure_count,
        disk_frequency_shift,
        disk_bolometric_radiance,
        trace_wall_clock_seconds: Some(trace_wall),
        mapping_wall_clock_seconds: Some(mapping_wall),
        render_wall_clock_seconds: Some(render_wall),
        content_digest_excluding_digest_field: String::new(),
    };
    report.content_digest_excluding_digest_field = content_digest(&report);

    std::fs::write(
        out_dir.join("lensed-celestial-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    let _ = TRACE_EXECUTION_FILENAME;

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn summarize_bundle(bundle: &relativity_trace::TraceBundle) -> (OutcomeCounts, u64, u64, u64) {
    let mut counts = OutcomeCounts {
        disk_hit: 0,
        escaped: 0,
        horizon_event: 0,
        horizon_approach: 0,
        affine_limit: 0,
        failed: 0,
    };
    let mut acc = 0u64;
    let mut rej = 0u64;
    let mut rhs = 0u64;
    for o in &bundle.outcomes {
        match o.class() {
            relativity_trace::OutcomeClass::DiskHit => counts.disk_hit += 1,
            relativity_trace::OutcomeClass::Escaped => counts.escaped += 1,
            relativity_trace::OutcomeClass::HorizonEvent => counts.horizon_event += 1,
            relativity_trace::OutcomeClass::HorizonApproach => counts.horizon_approach += 1,
            relativity_trace::OutcomeClass::AffineLimit => counts.affine_limit += 1,
            relativity_trace::OutcomeClass::Failed => counts.failed += 1,
        }
        rhs += o.rhs_evaluations();
        match o {
            relativity_trace::RayOutcome::DiskHit(h) => {
                acc += h.integration.accepted_steps;
                rej += h.integration.rejected_steps;
            }
            relativity_trace::RayOutcome::Escaped(h) => {
                acc += h.integration.accepted_steps;
                rej += h.integration.rejected_steps;
            }
            relativity_trace::RayOutcome::HorizonEvent(h) => {
                acc += h.integration.accepted_steps;
                rej += h.integration.rejected_steps;
            }
            relativity_trace::RayOutcome::HorizonApproach(h) => {
                acc += h.integration.accepted_steps;
                rej += h.integration.rejected_steps;
            }
            relativity_trace::RayOutcome::AffineLimit(h) => {
                acc += h.integration.accepted_steps;
                rej += h.integration.rejected_steps;
            }
            relativity_trace::RayOutcome::Failed(_) => {}
        }
    }
    (counts, acc, rej, rhs)
}

pub fn content_digest(report: &LensedCelestialReport) -> String {
    #[derive(Serialize)]
    struct FreqProj<'a> {
        observer_frequency_verification_passes: u32,
        frequency_shift_passes: u32,
        convention: &'a DiskFrequencyShiftConvention,
        velocity_model: DiskVelocityModel,
        resolved_direction: EquatorialAngularDirection,
        disk_hit_count: u64,
        mapped_count: u64,
        mapping_failure_count: u64,
        redshifted_count: u64,
        blueshifted_count: u64,
        exact_unity_count: u64,
        minimum_g: &'a Option<RankedFrequencyShiftPixel>,
        maximum_g: &'a Option<RankedFrequencyShiftPixel>,
        closest_to_unity: &'a Option<RankedFrequencyShiftPixel>,
        maximum_abs_disk_radius_residual_bits: u64,
        maximum_observer_unit_frequency_residual_bits: u64,
        frequency_shift_digest: &'a str,
        frequency_shift_json_digest: &'a str,
        g_factor_debug_ppm_digest: &'a str,
        below_visualization_range_count: u64,
        above_visualization_range_count: u64,
        regression_corpus: &'a [FrequencyShiftRegressionSample],
    }
    #[derive(Serialize)]
    struct Proj<'a> {
        schema_version: u32,
        width: u32,
        height: u32,
        render_tier: Option<DiagnosticRenderTier>,
        resolution_source: ResolutionSource,
        authority_class: RenderAuthorityClass,
        build: &'a BuildExecutionMetadata,
        execution: &'a TraceExecutionMetadata,
        surface_set: TraceSurfaceSet,
        mode: LensedCelestialMode,
        trace_invocations: u32,
        coordinate_passes: u32,
        texture_render_passes: u32,
        numerical_profile_digest: &'a str,
        trace_data_digest: &'a str,
        outcome_class_digest: &'a str,
        coordinate_digest: &'a str,
        texture_spec: &'a ProceduralCelestialTextureSpec,
        texture_spec_digest: &'a str,
        texture_sample_count: u64,
        non_escaped_count: u64,
        outcome_counts: &'a OutcomeCounts,
        lensed_ppm_filename: &'a str,
        lensed_ppm_digest: &'a str,
        categorical_ppm_digest: &'a str,
        rhs_pgm_digest: &'a str,
        coordinate_json_digest: &'a str,
        uv_debug_ppm_digest: &'a str,
        convention_id: &'a str,
        resolved_boundary_radius_bits: u64,
        radius_policy: &'a str,
        mapping_failure_count: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        disk_frequency_shift: Option<FreqProj<'a>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        disk_bolometric_radiance: Option<BoloProj<'a>>,
        content_digest_excluding_digest_field: &'a str,
    }
    #[derive(Serialize)]
    struct BoloProj<'a> {
        bolometric_emission_passes: u32,
        bolometric_transport_passes: u32,
        bolometric_visualization_passes: u32,
        convention: &'a DiskBolometricConvention,
        emission_spec: &'a DiagnosticBolometricEmissionSpec,
        emission_spec_digest: &'a str,
        accepted_emission_model: &'a str,
        accepted_emission_claim: &'a str,
        display_spec: &'a BolometricDebugDisplaySpec,
        display_spec_digest: &'a str,
        resolved_disk_bounds_inner_bits: u64,
        resolved_disk_bounds_outer_bits: u64,
        disk_bounds_source: &'a str,
        source_frequency_shift_digest: &'a str,
        disk_hit_count: u64,
        mapped_count: u64,
        mapping_failure_count: u64,
        attenuated_count: u64,
        boosted_count: u64,
        unchanged_count: u64,
        minimum_emitted: &'a Option<RankedBolometricPixel>,
        maximum_emitted: &'a Option<RankedBolometricPixel>,
        minimum_observed: &'a Option<RankedBolometricPixel>,
        maximum_observed: &'a Option<RankedBolometricPixel>,
        minimum_transport_factor: &'a Option<RankedBolometricPixel>,
        maximum_transport_factor: &'a Option<RankedBolometricPixel>,
        maximum_abs_transport_residual_bits: u64,
        bolometric_digest: &'a str,
        bolometric_json_digest: &'a str,
        emitted_debug_ppm_digest: &'a str,
        observed_debug_ppm_digest: &'a str,
        composite_ppm_digest: &'a str,
        emitted_below_range_count: u64,
        emitted_above_range_count: u64,
        observed_below_range_count: u64,
        observed_above_range_count: u64,
        regression_corpus: &'a [BolometricRegressionSample],
    }
    let freq = report.disk_frequency_shift.as_ref().map(|f| FreqProj {
        observer_frequency_verification_passes: f.observer_frequency_verification_passes,
        frequency_shift_passes: f.frequency_shift_passes,
        convention: &f.convention,
        velocity_model: f.velocity_model,
        resolved_direction: f.resolved_direction,
        disk_hit_count: f.disk_hit_count,
        mapped_count: f.mapped_count,
        mapping_failure_count: f.mapping_failure_count,
        redshifted_count: f.redshifted_count,
        blueshifted_count: f.blueshifted_count,
        exact_unity_count: f.exact_unity_count,
        minimum_g: &f.minimum_g,
        maximum_g: &f.maximum_g,
        closest_to_unity: &f.closest_to_unity,
        maximum_abs_disk_radius_residual_bits: f.maximum_abs_disk_radius_residual.to_bits(),
        maximum_observer_unit_frequency_residual_bits: f
            .maximum_observer_unit_frequency_residual
            .to_bits(),
        frequency_shift_digest: &f.frequency_shift_digest,
        frequency_shift_json_digest: &f.frequency_shift_json_digest,
        g_factor_debug_ppm_digest: &f.g_factor_debug_ppm_digest,
        below_visualization_range_count: f.below_visualization_range_count,
        above_visualization_range_count: f.above_visualization_range_count,
        regression_corpus: &f.regression_corpus,
    });
    let bolo = report.disk_bolometric_radiance.as_ref().map(|b| BoloProj {
        bolometric_emission_passes: b.bolometric_emission_passes,
        bolometric_transport_passes: b.bolometric_transport_passes,
        bolometric_visualization_passes: b.bolometric_visualization_passes,
        convention: &b.convention,
        emission_spec: &b.emission_spec,
        emission_spec_digest: &b.emission_spec_digest,
        accepted_emission_model: &b.accepted_emission_model,
        accepted_emission_claim: &b.accepted_emission_claim,
        display_spec: &b.display_spec,
        display_spec_digest: &b.display_spec_digest,
        resolved_disk_bounds_inner_bits: b.resolved_disk_bounds.inner_radius().to_bits(),
        resolved_disk_bounds_outer_bits: b.resolved_disk_bounds.outer_radius().to_bits(),
        disk_bounds_source: &b.disk_bounds_source,
        source_frequency_shift_digest: &b.source_frequency_shift_digest,
        disk_hit_count: b.disk_hit_count,
        mapped_count: b.mapped_count,
        mapping_failure_count: b.mapping_failure_count,
        attenuated_count: b.attenuated_count,
        boosted_count: b.boosted_count,
        unchanged_count: b.unchanged_count,
        minimum_emitted: &b.minimum_emitted,
        maximum_emitted: &b.maximum_emitted,
        minimum_observed: &b.minimum_observed,
        maximum_observed: &b.maximum_observed,
        minimum_transport_factor: &b.minimum_transport_factor,
        maximum_transport_factor: &b.maximum_transport_factor,
        maximum_abs_transport_residual_bits: b.maximum_abs_transport_residual.to_bits(),
        bolometric_digest: &b.bolometric_digest,
        bolometric_json_digest: &b.bolometric_json_digest,
        emitted_debug_ppm_digest: &b.emitted_debug_ppm_digest,
        observed_debug_ppm_digest: &b.observed_debug_ppm_digest,
        composite_ppm_digest: &b.composite_ppm_digest,
        emitted_below_range_count: b.emitted_below_range_count,
        emitted_above_range_count: b.emitted_above_range_count,
        observed_below_range_count: b.observed_below_range_count,
        observed_above_range_count: b.observed_above_range_count,
        regression_corpus: &b.regression_corpus,
    });
    let proj = Proj {
        schema_version: report.schema_version,
        width: report.width,
        height: report.height,
        render_tier: report.render_tier,
        resolution_source: report.resolution_source,
        authority_class: report.authority_class,
        build: &report.build,
        execution: &report.execution,
        surface_set: report.surface_set,
        mode: report.mode,
        trace_invocations: report.trace_invocations,
        coordinate_passes: report.coordinate_passes,
        texture_render_passes: report.texture_render_passes,
        numerical_profile_digest: &report.numerical_profile_digest,
        trace_data_digest: &report.trace_data_digest,
        outcome_class_digest: &report.outcome_class_digest,
        coordinate_digest: &report.coordinate_digest,
        texture_spec: &report.texture_spec,
        texture_spec_digest: &report.texture_spec_digest,
        texture_sample_count: report.texture_sample_count,
        non_escaped_count: report.non_escaped_count,
        outcome_counts: &report.outcome_counts,
        lensed_ppm_filename: &report.lensed_ppm_filename,
        lensed_ppm_digest: &report.lensed_ppm_digest,
        categorical_ppm_digest: &report.categorical_ppm_digest,
        rhs_pgm_digest: &report.rhs_pgm_digest,
        coordinate_json_digest: &report.coordinate_json_digest,
        uv_debug_ppm_digest: &report.uv_debug_ppm_digest,
        convention_id: &report.convention_id,
        resolved_boundary_radius_bits: report.resolved_boundary_radius.to_bits(),
        radius_policy: &report.radius_policy,
        mapping_failure_count: report.mapping_failure_count,
        disk_frequency_shift: freq,
        disk_bolometric_radiance: bolo,
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize"))
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic_scene::{
        gate_1b2_diagnostic_integrator, numerical_profile_from_integrator,
    };

    fn sample_report() -> LensedCelestialReport {
        let build = BuildExecutionMetadata {
            cargo_profile: "release".into(),
            opt_level: "3".into(),
            debug_assertions: false,
            target: "t".into(),
            toolchain: "t".into(),
        };
        let exec = TraceExecutionMetadata::serial();
        let numerical_profile =
            numerical_profile_from_integrator(&gate_1b2_diagnostic_integrator().unwrap());
        let texture_spec = procedural_coordinate_grid_v1();
        let texture_spec_digest = procedural_texture_spec_digest(&texture_spec);
        LensedCelestialReport {
            schema_version: 1,
            width: 32,
            height: 32,
            render_tier: Some(DiagnosticRenderTier::Smoke),
            resolution_source: ResolutionSource::NamedTier,
            authority_class: RenderAuthorityClass::NonAuthoritative,
            build,
            execution: exec,
            surface_set: TraceSurfaceSet::HorizonEscapeOnly,
            mode: LensedCelestialMode::DiskOmittedDiagnostic,
            trace_invocations: 1,
            coordinate_passes: 1,
            texture_render_passes: 1,
            numerical_profile_digest: numerical_profile.digest.clone(),
            numerical_profile,
            trace_data_digest: "td".into(),
            outcome_class_digest: "oc".into(),
            coordinate_digest: "cd".into(),
            texture_spec,
            texture_spec_digest,
            texture_sample_count: 10,
            non_escaped_count: 22,
            outcome_counts: OutcomeCounts {
                disk_hit: 0,
                escaped: 10,
                horizon_event: 20,
                horizon_approach: 2,
                affine_limit: 0,
                failed: 0,
            },
            lensed_ppm_filename: LensedCelestialMode::DiskOmittedDiagnostic
                .ppm_filename()
                .into(),
            lensed_ppm_digest: "lp".into(),
            categorical_ppm_digest: "cp".into(),
            rhs_pgm_digest: "pg".into(),
            coordinate_json_digest: "cj".into(),
            uv_debug_ppm_digest: "uv".into(),
            convention_id: "finite-oblate-ks-boundary-uv-v1".into(),
            resolved_boundary_radius: 80.0,
            radius_policy: RADIUS_POLICY_GATE_1B2_CAP.into(),
            mapping_failure_count: 0,
            disk_frequency_shift: None,
            disk_bolometric_radiance: None,
            trace_wall_clock_seconds: Some(1.0),
            mapping_wall_clock_seconds: Some(0.1),
            render_wall_clock_seconds: Some(0.01),
            content_digest_excluding_digest_field: String::new(),
        }
    }

    #[test]
    fn timing_excluded_from_report_digest() {
        let mut a = sample_report();
        let mut b = a.clone();
        b.trace_wall_clock_seconds = Some(99.0);
        b.mapping_wall_clock_seconds = Some(9.0);
        b.render_wall_clock_seconds = Some(1.0);
        assert_eq!(content_digest(&a), content_digest(&b));
        a.mode = LensedCelestialMode::OpaqueDiskMask;
        a.surface_set = TraceSurfaceSet::OpaqueDiskHorizonEscape;
        assert_ne!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn frequency_timing_excluded_from_report_digest() {
        let mut a = sample_report();
        a.disk_frequency_shift = Some(DiskFrequencyShiftOutputReport {
            observer_frequency_verification_passes: 1,
            frequency_shift_passes: 1,
            convention: DiskFrequencyShiftConvention::v1(),
            velocity_model: DiskVelocityModel::ProgradeCircularGeodesic,
            resolved_direction: EquatorialAngularDirection::PositivePhi,
            disk_hit_count: 1,
            mapped_count: 1,
            mapping_failure_count: 0,
            redshifted_count: 0,
            blueshifted_count: 1,
            exact_unity_count: 0,
            minimum_g: None,
            maximum_g: None,
            closest_to_unity: None,
            maximum_abs_disk_radius_residual: 0.0,
            maximum_observer_unit_frequency_residual: 0.0,
            frequency_shift_digest: "fs".into(),
            frequency_shift_json_digest: "fj".into(),
            g_factor_debug_ppm_digest: "gp".into(),
            below_visualization_range_count: 0,
            above_visualization_range_count: 0,
            regression_corpus: vec![],
            verification_wall_clock_seconds: Some(0.01),
            mapping_wall_clock_seconds: Some(0.02),
        });
        let mut b = a.clone();
        if let Some(f) = b.disk_frequency_shift.as_mut() {
            f.verification_wall_clock_seconds = Some(9.0);
            f.mapping_wall_clock_seconds = Some(8.0);
        }
        assert_eq!(content_digest(&a), content_digest(&b));
        if let Some(f) = b.disk_frequency_shift.as_mut() {
            f.frequency_shift_digest = "changed".into();
        }
        assert_ne!(content_digest(&a), content_digest(&b));
    }
}
