//! Trace once, shade many diagnostic styles (Gate 2A0-3).

use crate::build_meta::{
    require_release_execution, write_build_execution_report, BuildExecutionMetadata,
};
use crate::preset::load_preset;
use crate::trace_outcome_map::{
    resolve_execution, write_trace_execution_report, CliExecution, TRACE_EXECUTION_FILENAME,
};
use relativity_core::{CameraParams, KerrParams, PositionBl};
use relativity_integrate::{Dop853Config, EventArmingPolicy, HorizonProximityPolicy};
use relativity_trace::{
    encode_ppm, hex_sha, outcome_class_bytes, shade_many, trace_data_digest,
    trace_grid_with_execution, write_rhs_pgm, DiagnosticShadeStyle, OutcomeCounts,
    ThinDiskGeometry, TraceExecutionMetadata, TraceGrid, TraceScene,
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
    pub content_digest_excluding_digest_field: String,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    preset_path: &str,
    width: u32,
    height: u32,
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

    let mass = preset.spacetime.mass;
    let spin = preset.spacetime.spin_a_over_m * mass;
    let kerr = KerrParams::new(mass, spin)?;
    let r_plus = kerr.outer_horizon_radius();
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
        escape_radius: preset.celestial_sphere.radius_m.min(80.0),
        event_arming: integrator.event_arming.clone(),
        integrator,
        grid: TraceGrid { width, height },
    };
    scene.validate()?;

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
        width,
        height,
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
        content_digest_excluding_digest_field: String::new(),
    };
    report.content_digest_excluding_digest_field = content_digest(&report);

    std::fs::write(
        out_dir.join("trace-shade-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    // Keep TRACE_EXECUTION_FILENAME symbol referenced for clarity of adjacency.
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

fn content_digest(report: &TraceShadeReport) -> String {
    #[derive(Serialize)]
    struct Proj<'a> {
        gate: &'a str,
        width: u32,
        height: u32,
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

    #[test]
    fn timing_excluded_from_report_digest() {
        let build = BuildExecutionMetadata {
            cargo_profile: "release".into(),
            opt_level: "3".into(),
            debug_assertions: false,
            target: "t".into(),
            toolchain: "t".into(),
        };
        let exec = TraceExecutionMetadata::serial();
        let mut a = TraceShadeReport {
            gate: "gate-2a0-trace-shade".into(),
            width: 32,
            height: 32,
            build: build.clone(),
            execution: exec.clone(),
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
            content_digest_excluding_digest_field: String::new(),
        };
        let mut b = a.clone();
        b.trace_wall_clock_seconds = Some(99.0);
        b.shade_wall_clock_seconds = Some(9.0);
        assert_eq!(content_digest(&a), content_digest(&b));
        a.styles.pop();
        assert_ne!(content_digest(&a), content_digest(&b));
    }
}
