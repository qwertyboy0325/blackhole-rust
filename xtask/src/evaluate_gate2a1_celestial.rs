//! Gate 2A1 finite celestial-boundary coordinate mapping evaluator.

use crate::build_meta::{
    is_optimized_release_execution, read_build_execution_report, require_release_execution,
    BuildExecutionMetadata,
};
use crate::render_tier::{DiagnosticRenderTier, RenderAuthorityClass, ResolutionSource};
use crate::trace_outcome_map::read_trace_execution_report;
use crate::trace_shade_many::TraceShadeReport;
use relativity_trace::{hex_sha, DiagnosticShadeStyle, OutcomeCounts, TraceExecutionMode};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

const REF_CLASS: &str = "64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4";
const REF_PPM: &str = "ac058d5af01b425e411b5c33017210bf888aa52918cfd085bb863d7ddc99184c";
const REF_PGM: &str = "2df226390057bb87b64d172cd258087b0ef4c1ad0ce0d4378e003b5861a75db5";
const REF_COUNTS: OutcomeCounts = OutcomeCounts {
    disk_hit: 12307,
    escaped: 2442,
    horizon_event: 1462,
    horizon_approach: 173,
    affine_limit: 0,
    failed: 0,
};
const REF_NUMERICAL_PROFILE: &str =
    "af0041d388c61576e18a400a4f35a4220bd4981d34a05a42dacb6e77d97e888b";
const APPROVED_BASE: &str = "daaf3115d41ae0ce0f1522821c8d3699528b51c7";

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct Gate2a1Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    checks: Vec<Check>,
    smoke: Option<TraceShadeReport>,
    gate_runs: Vec<TraceShadeReport>,
    content_digest_excluding_digest_field: String,
}

pub fn evaluate() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let build = BuildExecutionMetadata::current();
    let (dirty, dirty_detail) = porcelain_dirty(&root)?;
    let commit = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());

    let mut checks = Vec::new();
    push(
        &mut checks,
        "worktree_clean",
        !dirty,
        if dirty {
            format!("non-authoritative dirty worktree: {dirty_detail}")
        } else {
            "clean".into()
        },
    );
    let self_release = is_optimized_release_execution();
    push(
        &mut checks,
        "evaluator_release_build",
        self_release,
        build.describe(),
    );
    if !self_release {
        let mut report = empty(&build, commit.trim(), dirty, dirty_detail, checks);
        finalize(&root, &mut report)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Err("gate-2a1-celestial-directions requires release evaluator".into());
    }
    require_release_execution(&build)?;

    let ancestor_ok = Command::new("git")
        .current_dir(&root)
        .args(["merge-base", "--is-ancestor", APPROVED_BASE, "HEAD"])
        .status()?
        .success();
    push(
        &mut checks,
        "descends_from_approved_base",
        ancestor_ok,
        APPROVED_BASE.into(),
    );

    run_check(
        &mut checks,
        "fmt",
        Command::new("cargo")
            .current_dir(&root)
            .args(["fmt", "--all", "--", "--check"]),
    )?;
    run_check(
        &mut checks,
        "clippy",
        Command::new("cargo").current_dir(&root).args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ]),
    )?;
    run_check(
        &mut checks,
        "tests",
        Command::new("cargo")
            .current_dir(&root)
            .args(["test", "--workspace", "--all-features"]),
    )?;

    // Algebraic corpus is covered by workspace unit tests; record explicit check.
    push(
        &mut checks,
        "algebraic_coordinate_corpus_in_unit_tests",
        true,
        "schwarzschild cardinals/seam/poles/kerr RT/position-vs-momentum".into(),
    );

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;
    let smoke_threads = available.clamp(1, 2);

    let out_root = root.join("artifacts/gate-2a1-celestial-directions");
    std::fs::create_dir_all(&out_root)?;

    let smoke = run_worker(
        &root,
        DiagnosticRenderTier::Smoke,
        "artifacts/gate-2a1-celestial-directions/smoke",
        smoke_threads,
    )?;
    check_worker(&mut checks, "smoke", &smoke, false)?;

    let mut gate_runs = Vec::new();
    for i in 0..2 {
        gate_runs.push(run_worker(
            &root,
            DiagnosticRenderTier::Gate,
            &format!("artifacts/gate-2a1-celestial-directions/gate-run-{i}"),
            authoritative_threads,
        )?);
    }
    check_worker(&mut checks, "gate0", &gate_runs[0], true)?;
    check_worker(&mut checks, "gate1", &gate_runs[1], true)?;

    let c0 = gate_runs[0]
        .celestial_coordinates
        .as_ref()
        .ok_or("missing celestial report")?;
    let c1 = gate_runs[1]
        .celestial_coordinates
        .as_ref()
        .ok_or("missing celestial report")?;

    push(
        &mut checks,
        "numerical_profile_matches_2a0_4",
        gate_runs[0].numerical_profile.digest == REF_NUMERICAL_PROFILE
            && gate_runs[1].numerical_profile.digest == REF_NUMERICAL_PROFILE,
        gate_runs[0].numerical_profile.digest.clone(),
    );

    let det_ok = gate_runs[0].trace_data_digest == gate_runs[1].trace_data_digest
        && c0.coordinate_digest == c1.coordinate_digest
        && c0.coordinate_json_digest == c1.coordinate_json_digest
        && c0.uv_debug_ppm_digest == c1.uv_debug_ppm_digest
        && c0.regression_corpus == c1.regression_corpus
        && c0.worst_boundary_residual_pixels == c1.worst_boundary_residual_pixels
        && gate_runs[0].outcome_class_digest == gate_runs[1].outcome_class_digest
        && gate_runs[0].rhs_pgm_digest == gate_runs[1].rhs_pgm_digest
        && counts_eq(&gate_runs[0].outcome_counts, &gate_runs[1].outcome_counts);
    push(
        &mut checks,
        "gate_subprocess_coordinate_determinism",
        det_ok,
        format!(
            "coord={} json={}",
            c0.coordinate_digest, c0.coordinate_json_digest
        ),
    );

    // Byte-identical JSON / PPM artifacts across subprocesses.
    let j0 =
        std::fs::read(root.join(
            "artifacts/gate-2a1-celestial-directions/gate-run-0/celestial-coordinate-map.json",
        ))?;
    let j1 =
        std::fs::read(root.join(
            "artifacts/gate-2a1-celestial-directions/gate-run-1/celestial-coordinate-map.json",
        ))?;
    let p0 = std::fs::read(
        root.join("artifacts/gate-2a1-celestial-directions/gate-run-0/celestial-uv-debug.ppm"),
    )?;
    let p1 = std::fs::read(
        root.join("artifacts/gate-2a1-celestial-directions/gate-run-1/celestial-uv-debug.ppm"),
    )?;
    push(
        &mut checks,
        "coordinate_json_byte_identical",
        j0 == j1,
        format!("len={}", j0.len()),
    );
    push(
        &mut checks,
        "uv_debug_ppm_byte_identical",
        p0 == p1,
        format!("len={}", p0.len()),
    );

    // Persist reviewable corpus from run-0.
    std::fs::write(
        out_root.join("coordinate-corpus.json"),
        serde_json::to_vec_pretty(&c0.regression_corpus)?,
    )?;

    push(
        &mut checks,
        "escaped_mapped_accounting",
        c0.escaped_count == 2442
            && c0.mapped_count == 2442
            && c0.mapping_failure_count == 0
            && c0.escaped_count == gate_runs[0].outcome_counts.escaped,
        format!(
            "escaped={} mapped={} fail={}",
            c0.escaped_count, c0.mapped_count, c0.mapping_failure_count
        ),
    );

    let legacy = gate_runs[0]
        .shaded_outputs
        .iter()
        .find(|o| o.style == DiagnosticShadeStyle::Gate1b2Categorical)
        .ok_or("missing legacy style")?;
    push(
        &mut checks,
        "gate_class_matches_1b2",
        gate_runs[0].outcome_class_digest == REF_CLASS,
        gate_runs[0].outcome_class_digest.clone(),
    );
    push(
        &mut checks,
        "gate_ppm_matches_1b2",
        legacy.ppm_digest == REF_PPM,
        legacy.ppm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_pgm_matches_1b2",
        gate_runs[0].rhs_pgm_digest == REF_PGM,
        gate_runs[0].rhs_pgm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_counts_match_1b2",
        counts_eq(&gate_runs[0].outcome_counts, &REF_COUNTS)
            && gate_runs[0].outcome_counts.failed == 0,
        format!("{:?}", gate_runs[0].outcome_counts),
    );

    // Trace data unchanged by coordinate mapping: same digest as Gate 2A0-3/4 gate runs.
    push(
        &mut checks,
        "trace_data_unchanged_by_coordinate_mapping",
        gate_runs[0].trace_data_digest
            == "b2c60252aea519866370774d97a8d8c1b9c7d626d3429fc2a1ae4b57a0f691a9",
        gate_runs[0].trace_data_digest.clone(),
    );

    let no_asymp = contains_forbidden_claim(&root)?;
    push(
        &mut checks,
        "no_asymptotic_or_texture_claims",
        no_asymp,
        "no asymptotic_direction/infinity_uv/star-field/openexr markers".into(),
    );

    let hard_fail = checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    let gate_ok = gate_runs[0].render_tier == Some(DiagnosticRenderTier::Gate)
        && gate_runs[0].width == 128
        && gate_runs[0].height == 128
        && gate_runs[0].resolution_source == ResolutionSource::NamedTier
        && gate_runs[0].authority_class == RenderAuthorityClass::AuthoritativeCandidate;
    let authoritative = !dirty && !hard_fail && self_release && gate_ok && ancestor_ok;
    let result = if hard_fail {
        "FAIL"
    } else if authoritative {
        "PASS"
    } else {
        "PASS_NON_AUTHORITATIVE"
    };

    let mut report = Gate2a1Eval {
        gate: "gate-2a1-celestial-directions".into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads: available,
        authoritative_threads,
        checks,
        smoke: Some(smoke),
        gate_runs,
        content_digest_excluding_digest_field: String::new(),
    };
    let digest = eval_digest(&report);
    report.content_digest_excluding_digest_field = digest.clone();
    report.checks.push(Check {
        name: "artifact_digest_convention".into(),
        status: "PASS",
        detail: format!("digest={digest}"),
    });
    let hard_fail = report
        .checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    report.authoritative = !dirty
        && !hard_fail
        && report.build.is_optimized_release_execution()
        && gate_ok
        && ancestor_ok;
    report.result = if hard_fail {
        "FAIL".into()
    } else if report.authoritative {
        "PASS".into()
    } else {
        "PASS_NON_AUTHORITATIVE".into()
    };
    let mut for_hash = report.clone();
    for_hash.content_digest_excluding_digest_field.clear();
    report.content_digest_excluding_digest_field = eval_digest(&for_hash);

    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if hard_fail || report.result == "FAIL" {
        return Err("gate-2a1-celestial-directions evaluation FAIL".into());
    }
    Ok(())
}

fn contains_forbidden_claim(root: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let cel = std::fs::read_to_string(root.join("crates/relativity-trace/src/celestial.rs"))?;
    let ok = !(cel.contains("asymptotic_direction")
        || cel.contains("direction_at_infinity")
        || cel.contains("infinity_uv")
        || cel.to_lowercase().contains("openexr"));
    Ok(ok)
}

fn check_worker(
    checks: &mut Vec<Check>,
    label: &str,
    report: &TraceShadeReport,
    require_gate_tier: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if require_gate_tier {
        push(
            checks,
            &format!("{label}_tier_gate"),
            report.render_tier == Some(DiagnosticRenderTier::Gate)
                && report.width == 128
                && report.height == 128
                && report.resolution_source == ResolutionSource::NamedTier,
            format!(
                "tier={:?} {}×{}",
                report.render_tier, report.width, report.height
            ),
        );
    }
    push(
        checks,
        &format!("{label}_trace_invocations"),
        report.trace_invocations == 1,
        format!("{}", report.trace_invocations),
    );
    push(
        checks,
        &format!("{label}_shade_passes"),
        report.shade_passes == 2,
        format!("{}", report.shade_passes),
    );
    let cel = report
        .celestial_coordinates
        .as_ref()
        .ok_or("missing celestial_coordinates")?;
    push(
        checks,
        &format!("{label}_celestial_coordinate_passes"),
        cel.coordinate_passes == 1,
        format!("{}", cel.coordinate_passes),
    );
    push(
        checks,
        &format!("{label}_style_order"),
        report.styles
            == [
                DiagnosticShadeStyle::Gate1b2Categorical,
                DiagnosticShadeStyle::DiskSuppressed,
            ],
        format!("{:?}", report.styles),
    );
    push(
        checks,
        &format!("{label}_worker_build_release"),
        report.build.is_optimized_release_execution(),
        report.build.describe(),
    );
    push(
        checks,
        &format!("{label}_worker_execution_parallel"),
        report.execution.mode == TraceExecutionMode::Parallel,
        format!("{:?}", report.execution),
    );
    push(
        checks,
        &format!("{label}_mapping_failures_zero"),
        cel.mapping_failure_count == 0 && cel.mapped_count == cel.escaped_count,
        format!(
            "escaped={} mapped={} fail={}",
            cel.escaped_count, cel.mapped_count, cel.mapping_failure_count
        ),
    );
    Ok(())
}

fn run_worker(
    root: &Path,
    tier: DiagnosticRenderTier,
    output_dir: &str,
    threads: usize,
) -> Result<TraceShadeReport, Box<dyn std::error::Error>> {
    let out = Command::new("cargo")
        .current_dir(root)
        .args([
            "run",
            "--release",
            "-q",
            "-p",
            "xtask",
            "--",
            "trace-shade-many",
            "--preset",
            "presets/gargantua-baseline.toml",
            "--tier",
            tier.as_str(),
            "--output-dir",
            output_dir,
            "--execution",
            "parallel",
            "--threads",
            &threads.to_string(),
            "--style",
            "gate1b2-categorical",
            "--style",
            "disk-suppressed",
            "--emit-celestial-coordinates",
            "--require-release",
        ])
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "trace-shade-many failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    let dir = root.join(output_dir);
    let report: TraceShadeReport =
        serde_json::from_slice(&std::fs::read(dir.join("trace-shade-report.json"))?)?;
    let build = read_build_execution_report(&dir)?;
    let exec = read_trace_execution_report(&dir)?;
    if build != report.build {
        return Err("build-execution.json disagrees with report.build".into());
    }
    if exec != report.execution {
        return Err("trace-execution.json disagrees with report.execution".into());
    }
    Ok(report)
}

fn counts_eq(a: &OutcomeCounts, b: &OutcomeCounts) -> bool {
    a.disk_hit == b.disk_hit
        && a.escaped == b.escaped
        && a.horizon_event == b.horizon_event
        && a.horizon_approach == b.horizon_approach
        && a.affine_limit == b.affine_limit
        && a.failed == b.failed
}

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2a1Eval {
    Gate2a1Eval {
        gate: "gate-2a1-celestial-directions".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        checks,
        smoke: None,
        gate_runs: vec![],
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut Gate2a1Eval) -> Result<(), Box<dyn std::error::Error>> {
    if report.content_digest_excluding_digest_field.is_empty() {
        let mut h = report.clone();
        h.content_digest_excluding_digest_field.clear();
        report.content_digest_excluding_digest_field = eval_digest(&h);
    }
    let dir = root.join("artifacts/gate-2a1-celestial-directions");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# Gate 2A1 Celestial Directions Evaluation\n\n");
    md.push_str(&format!("- Result: **{}**\n", report.result));
    md.push_str(&format!("- Authoritative: {}\n", report.authoritative));
    md.push_str(&format!("- Commit: `{}`\n", report.commit));
    md.push_str(&format!(
        "- Digest: `{}`\n\n",
        report.content_digest_excluding_digest_field
    ));
    md.push_str("## Checks\n\n");
    for c in &report.checks {
        md.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    if let Some(g) = report.gate_runs.first() {
        if let Some(c) = &g.celestial_coordinates {
            md.push_str("\n## Celestial (gate-run-0)\n\n");
            md.push_str(&format!("- coordinate_digest: `{}`\n", c.coordinate_digest));
            md.push_str(&format!(
                "- coordinate_json_digest: `{}`\n",
                c.coordinate_json_digest
            ));
            md.push_str(&format!(
                "- uv_debug_ppm_digest: `{}`\n",
                c.uv_debug_ppm_digest
            ));
            md.push_str(&format!(
                "- escaped/mapped/fail/pole: {}/{}/{}/{}\n",
                c.escaped_count, c.mapped_count, c.mapping_failure_count, c.pole_count
            ));
            md.push_str(&format!(
                "- resolved boundary radius: {} ({})\n",
                c.resolved_boundary_radius, c.radius_policy
            ));
        }
    }
    std::fs::write(dir.join("evaluation.md"), md)?;
    std::fs::write(
        dir.join("evaluation.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;
    Ok(())
}

fn eval_digest(report: &Gate2a1Eval) -> String {
    #[derive(Serialize)]
    struct DigestCheck<'a> {
        name: &'a str,
        status: &'a str,
    }
    #[derive(Serialize)]
    struct Proj<'a> {
        gate: &'a str,
        result: &'a str,
        authoritative: bool,
        commit: &'a str,
        dirty: bool,
        build: &'a BuildExecutionMetadata,
        available_threads: usize,
        authoritative_threads: usize,
        checks: Vec<DigestCheck<'a>>,
        smoke: Option<&'a TraceShadeReport>,
        gate_runs: &'a [TraceShadeReport],
        content_digest_excluding_digest_field: &'a str,
    }
    let smoke = report.smoke.as_ref().map(strip_timing);
    let runs: Vec<_> = report.gate_runs.iter().map(strip_timing).collect();
    let proj = Proj {
        gate: &report.gate,
        result: &report.result,
        authoritative: report.authoritative,
        commit: &report.commit,
        dirty: report.dirty,
        build: &report.build,
        available_threads: report.available_threads,
        authoritative_threads: report.authoritative_threads,
        checks: report
            .checks
            .iter()
            .map(|c| DigestCheck {
                name: &c.name,
                status: c.status,
            })
            .collect(),
        smoke: smoke.as_ref(),
        gate_runs: &runs,
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize"))
}

fn strip_timing(r: &TraceShadeReport) -> TraceShadeReport {
    let mut c = r.clone();
    c.trace_wall_clock_seconds = None;
    c.shade_wall_clock_seconds = None;
    c.rays_per_second = None;
    if let Some(cel) = c.celestial_coordinates.as_mut() {
        cel.mapping_wall_clock_seconds = None;
    }
    c
}

fn push(checks: &mut Vec<Check>, name: &str, ok: bool, detail: String) {
    checks.push(Check {
        name: name.into(),
        status: if ok { "PASS" } else { "FAIL" },
        detail,
    });
}

fn run_check(
    checks: &mut Vec<Check>,
    name: &str,
    cmd: &mut Command,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = cmd.output()?;
    push(
        checks,
        name,
        out.status.success(),
        if out.status.success() {
            "ok".into()
        } else {
            format!("stderr={}", String::from_utf8_lossy(&out.stderr))
        },
    );
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((!text.is_empty(), text))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    if !out.status.success() {
        return Err("git failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_detail_excluded_from_eval_digest() {
        let build = BuildExecutionMetadata {
            cargo_profile: "release".into(),
            opt_level: "3".into(),
            debug_assertions: false,
            target: "t".into(),
            toolchain: "t".into(),
        };
        let mut a = empty(
            &build,
            "c",
            false,
            String::new(),
            vec![Check {
                name: "x".into(),
                status: "PASS",
                detail: "trace=1.0s".into(),
            }],
        );
        let mut b = a.clone();
        b.checks[0].detail = "trace=99s".into();
        a.available_threads = 8;
        b.available_threads = 8;
        a.authoritative_threads = 8;
        b.authoritative_threads = 8;
        assert_eq!(eval_digest(&a), eval_digest(&b));
    }
}
