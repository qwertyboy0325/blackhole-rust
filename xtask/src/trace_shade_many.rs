//! Trace once, shade many diagnostic styles (Gate 2A0-3 / 2A0-4).

use crate::build_meta::{
    require_release_execution, write_build_execution_report, BuildExecutionMetadata,
};
use crate::diagnostic_scene::{build_diagnostic_trace_scene, DiagnosticNumericalProfile};
use crate::preset::load_preset;
use crate::render_tier::{
    resolve_render_plan, DiagnosticRenderTier, RenderAuthorityClass, ResolutionSource,
    ResolvedRenderPlan,
};
use crate::trace_outcome_map::{
    resolve_execution, write_trace_execution_report, CliExecution, TRACE_EXECUTION_FILENAME,
};
use relativity_trace::{
    encode_ppm, hex_sha, outcome_class_bytes, shade_many, trace_data_digest,
    trace_grid_with_execution, write_rhs_pgm, DiagnosticShadeStyle, OutcomeCounts,
    TraceExecutionMetadata, TraceGrid,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadeOutputReport {
    pub style: DiagnosticShadeStyle,
    pub filename: String,
    pub ppm_digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceShadeReport {
    pub gate: String,
    pub width: u32,
    pub height: u32,
    pub render_tier: Option<DiagnosticRenderTier>,
    pub resolution_source: ResolutionSource,
    pub authority_class: RenderAuthorityClass,
    pub numerical_profile: DiagnosticNumericalProfile,
    pub build: BuildExecutionMetadata,
    pub execution: TraceExecutionMetadata,
    pub trace_invocations: u32,
    pub shade_passes: u32,
    pub styles: Vec<DiagnosticShadeStyle>,
    pub trace_data_digest: String,
    pub outcome_class_digest: String,
    pub rhs_pgm_digest: String,
    pub shaded_outputs: Vec<ShadeOutputReport>,
    pub outcome_counts: OutcomeCounts,
    pub total_accepted_steps: u64,
    pub total_rejected_steps: u64,
    pub total_rhs_evaluations: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shade_wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rays_per_second: Option<f64>,
    pub content_digest_excluding_digest_field: String,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    preset_path: &str,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
    output_dir: &str,
    require_release: bool,
    execution: CliExecution,
    threads: Option<usize>,
    styles: &[DiagnosticShadeStyle],
) -> Result<(), Box<dyn std::error::Error>> {
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }
    if styles.is_empty() {
        return Err("trace-shade-many requires at least one --style".into());
    }
    let mut seen = BTreeSet::new();
    for s in styles {
        if !seen.insert(*s) {
            return Err(format!("duplicate shade style {} rejected by CLI", s.as_str()).into());
        }
    }

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

    // load preset → resolve plan → one common diagnostic scene → trace once → shade many
    let (scene, numerical_profile) = build_diagnostic_trace_scene(
        &preset,
        TraceGrid {
            width: plan.width,
            height: plan.height,
        },
    )?;

    // ---- Phase 1: trace exactly once ----
    let t_trace = Instant::now();
    let bundle = trace_grid_with_execution(&scene, trace_execution)?;
    let trace_wall = t_trace.elapsed().as_secs_f64();
    let trace_invocations = 1u32;

    for o in &bundle.outcomes {
        if !matches!(o, relativity_trace::RayOutcome::Failed(_)) && !o.state_finite() {
            return Err("non-finite success state in TraceBundle".into());
        }
    }

    let data_digest = trace_data_digest(&bundle);
    let class_digest = hex_sha(&outcome_class_bytes(&bundle));
    let pgm = write_rhs_pgm(&bundle);
    let pgm_digest = hex_sha(&pgm);
    std::fs::write(out_dir.join("rhs-evaluations.pgm"), &pgm)?;

    let (counts, total_acc, total_rej, total_rhs) = summarize_bundle(&bundle);
    let ray_count = (plan.width as u64) * (plan.height as u64);
    let rays_per_second = if trace_wall > 0.0 {
        Some(ray_count as f64 / trace_wall)
    } else {
        None
    };

    // ---- Phase 2: shade many (no tracing) ----
    let t_shade = Instant::now();
    let shaded = shade_many(&bundle, styles);
    let shade_wall = t_shade.elapsed().as_secs_f64();
    let shade_passes = shaded.len() as u32;

    let mut shaded_outputs = Vec::with_capacity(shaded.len());
    for s in &shaded {
        let filename = format!("{}.ppm", s.style.filename_stem());
        let ppm = encode_ppm(&s.frame);
        debug_assert_eq!(hex_sha(&ppm), s.ppm_digest);
        std::fs::write(out_dir.join(&filename), &ppm)?;
        shaded_outputs.push(ShadeOutputReport {
            style: s.style,
            filename,
            ppm_digest: s.ppm_digest.clone(),
        });
    }

    write_build_execution_report(&out_dir, &build)?;
    write_trace_execution_report(&out_dir, &exec_meta)?;

    let mut report = TraceShadeReport {
        gate: "gate-2a0-trace-shade".into(),
        width: plan.width,
        height: plan.height,
        render_tier: plan.tier,
        resolution_source: plan.resolution_source,
        authority_class: plan.authority_class,
        numerical_profile,
        build,
        execution: exec_meta,
        trace_invocations,
        shade_passes,
        styles: styles.to_vec(),
        trace_data_digest: data_digest,
        outcome_class_digest: class_digest,
        rhs_pgm_digest: pgm_digest,
        shaded_outputs,
        outcome_counts: counts,
        total_accepted_steps: total_acc,
        total_rejected_steps: total_rej,
        total_rhs_evaluations: total_rhs,
        trace_wall_clock_seconds: Some(trace_wall),
        shade_wall_clock_seconds: Some(shade_wall),
        rays_per_second,
        content_digest_excluding_digest_field: String::new(),
    };
    report.content_digest_excluding_digest_field = content_digest(&report);

    std::fs::write(
        out_dir.join("trace-shade-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    let _ = TRACE_EXECUTION_FILENAME;
    let _ = plan_summary(&plan);

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn plan_summary(plan: &ResolvedRenderPlan) -> String {
    format!(
        "{}×{} {:?} {:?}",
        plan.width, plan.height, plan.resolution_source, plan.authority_class
    )
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

fn content_digest(report: &TraceShadeReport) -> String {
    #[derive(Serialize)]
    struct Proj<'a> {
        gate: &'a str,
        width: u32,
        height: u32,
        render_tier: Option<DiagnosticRenderTier>,
        resolution_source: ResolutionSource,
        authority_class: RenderAuthorityClass,
        numerical_profile_digest: &'a str,
        build: &'a BuildExecutionMetadata,
        execution: &'a TraceExecutionMetadata,
        trace_invocations: u32,
        shade_passes: u32,
        styles: &'a [DiagnosticShadeStyle],
        trace_data_digest: &'a str,
        outcome_class_digest: &'a str,
        rhs_pgm_digest: &'a str,
        shaded_outputs: &'a [ShadeOutputReport],
        outcome_counts: &'a OutcomeCounts,
        total_accepted_steps: u64,
        total_rejected_steps: u64,
        total_rhs_evaluations: u64,
        content_digest_excluding_digest_field: &'a str,
    }
    let proj = Proj {
        gate: &report.gate,
        width: report.width,
        height: report.height,
        render_tier: report.render_tier,
        resolution_source: report.resolution_source,
        authority_class: report.authority_class,
        numerical_profile_digest: &report.numerical_profile.digest,
        build: &report.build,
        execution: &report.execution,
        trace_invocations: report.trace_invocations,
        shade_passes: report.shade_passes,
        styles: &report.styles,
        trace_data_digest: &report.trace_data_digest,
        outcome_class_digest: &report.outcome_class_digest,
        rhs_pgm_digest: &report.rhs_pgm_digest,
        shaded_outputs: &report.shaded_outputs,
        outcome_counts: &report.outcome_counts,
        total_accepted_steps: report.total_accepted_steps,
        total_rejected_steps: report.total_rejected_steps,
        total_rhs_evaluations: report.total_rhs_evaluations,
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

    fn sample_report() -> TraceShadeReport {
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
        TraceShadeReport {
            gate: "gate-2a0-trace-shade".into(),
            width: 32,
            height: 32,
            render_tier: Some(DiagnosticRenderTier::Smoke),
            resolution_source: ResolutionSource::NamedTier,
            authority_class: RenderAuthorityClass::NonAuthoritative,
            numerical_profile,
            build,
            execution: exec,
            trace_invocations: 1,
            shade_passes: 2,
            styles: vec![
                DiagnosticShadeStyle::Gate1b2Categorical,
                DiagnosticShadeStyle::DiskSuppressed,
            ],
            trace_data_digest: "td".into(),
            outcome_class_digest: "oc".into(),
            rhs_pgm_digest: "pg".into(),
            shaded_outputs: vec![],
            outcome_counts: OutcomeCounts {
                disk_hit: 0,
                escaped: 0,
                horizon_event: 0,
                horizon_approach: 0,
                affine_limit: 0,
                failed: 0,
            },
            total_accepted_steps: 0,
            total_rejected_steps: 0,
            total_rhs_evaluations: 0,
            trace_wall_clock_seconds: Some(1.0),
            shade_wall_clock_seconds: Some(0.01),
            rays_per_second: Some(1024.0),
            content_digest_excluding_digest_field: String::new(),
        }
    }

    #[test]
    fn timing_excluded_from_report_digest() {
        let mut a = sample_report();
        let mut b = a.clone();
        b.trace_wall_clock_seconds = Some(99.0);
        b.shade_wall_clock_seconds = Some(9.0);
        b.rays_per_second = Some(1.0);
        assert_eq!(content_digest(&a), content_digest(&b));
        a.styles.pop();
        assert_ne!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn render_tier_changes_content_digest() {
        let a = sample_report();
        let mut b = a.clone();
        b.render_tier = Some(DiagnosticRenderTier::Preview);
        b.width = 64;
        b.height = 64;
        assert_ne!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn dimensions_change_content_digest() {
        let a = sample_report();
        let mut b = a.clone();
        b.width = 48;
        b.height = 48;
        b.render_tier = None;
        b.resolution_source = ResolutionSource::CustomDimensions;
        assert_ne!(content_digest(&a), content_digest(&b));
    }
}
