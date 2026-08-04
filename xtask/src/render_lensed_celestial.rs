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
use relativity_render::{
    procedural_coordinate_grid_v1, procedural_texture_spec_digest, render_lensed_celestial,
    validate_mode_surface_set, verify_lensed_celestial_frame, LensedCelestialMode,
    ProceduralCelestialTextureSpec, TEXTURE_ID_V1,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_wall_clock_seconds: Option<f64>,
    pub content_digest_excluding_digest_field: String,
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
) -> Result<(), Box<dyn std::error::Error>> {
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }

    validate_mode_surface_set(mode, surface_set)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;

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
    std::fs::create_dir_all(&out_dir)?;

    let preset_full = if Path::new(preset_path).is_absolute() {
        PathBuf::from(preset_path)
    } else {
        root.join(preset_path)
    };
    let _preset_digest = hex::encode(Sha256::digest(std::fs::read(&preset_full)?));
    let preset = load_preset(&preset_full)?;

    if preset.celestial_sphere.texture != "procedural_coordinate_grid" {
        return Err(format!(
            "preset celestial_sphere.texture must be `procedural_coordinate_grid`, got `{}`",
            preset.celestial_sphere.texture
        )
        .into());
    }
    validate_celestial_seam(&preset.celestial_sphere.seam)?;

    let (scene, numerical_profile) = build_diagnostic_trace_scene(
        &preset,
        TraceGrid {
            width: plan.width,
            height: plan.height,
        },
    )?;

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
        content_digest_excluding_digest_field: &'a str,
    }
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
}
