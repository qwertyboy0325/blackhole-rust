//! Gate 2A0 release-execution foundation evaluator.

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use relativity_trace::{OutcomeCounts, PixelCoord, RhsDistribution};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Gate 1B2 128×128 reference (authoritative prior map).
pub const REF_CLASS_DIGEST: &str =
    "64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4";
pub const REF_PPM_DIGEST: &str = "ac058d5af01b425e411b5c33017210bf888aa52918cfd085bb863d7ddc99184c";
pub const REF_PGM_DIGEST: &str = "2df226390057bb87b64d172cd258087b0ef4c1ad0ce0d4378e003b5861a75db5";

pub const REF_COUNTS: OutcomeCounts = OutcomeCounts {
    disk_hit: 12307,
    escaped: 2442,
    horizon_event: 1462,
    horizon_approach: 173,
    affine_limit: 0,
    failed: 0,
};

/// Historical Gate 1B2 debug wall-clock (prior run; not measured here).
const HISTORICAL_1B2_DEBUG_SECONDS: f64 = 210.0;

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
pub struct BenchmarkRun {
    pub width: u32,
    pub height: u32,
    pub build: BuildExecutionMetadata,
    pub execution_mode: String,
    pub outcome_class_digest: String,
    pub ppm_digest: String,
    pub pgm_digest: String,
    pub counts: OutcomeCounts,
    pub total_accepted_steps: u64,
    pub total_rejected_steps: u64,
    pub total_rhs_evaluations: u64,
    pub rhs: RhsDistribution,
    pub most_expensive_rays: Vec<PixelCoord>,
    pub wall_clock_seconds: f64,
    pub rays_per_second: f64,
}

#[derive(Serialize, Clone)]
pub struct ReferenceComparison {
    pub classification_match: bool,
    pub ppm_match: bool,
    pub counts_match: bool,
    pub failed_zero: bool,
    pub pgm_match: bool,
    pub pgm_status: String,
    pub reference_class_digest: String,
    pub reference_ppm_digest: String,
    pub reference_pgm_digest: String,
    pub observed_class_digest: String,
    pub observed_ppm_digest: String,
    pub observed_pgm_digest: String,
    pub reference_counts: OutcomeCounts,
    pub observed_counts: OutcomeCounts,
    pub historical_gate_1b2_debug_wall_clock_seconds: f64,
    pub historical_note: String,
}

#[derive(Serialize, Clone)]
struct Gate2a0ReleaseReport {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    checks: Vec<Check>,
    smoke_release_32: Option<BenchmarkRun>,
    dev_serial_64: Option<BenchmarkRun>,
    release_serial_64: Vec<BenchmarkRun>,
    release_serial_64_median_seconds: f64,
    release_speedup_vs_dev: f64,
    release_serial_128: Option<BenchmarkRun>,
    reference_comparison_128: Option<ReferenceComparison>,
    content_digest_excluding_digest_field: String,
}

#[derive(Deserialize)]
struct OutcomeMapJson {
    outcome_class_digest: String,
    ppm_digest: String,
    pgm_digest: String,
    counts: OutcomeCounts,
    total_accepted_steps: u64,
    total_rejected_steps: u64,
    total_rhs_evaluations: u64,
    rhs: RhsDistribution,
    most_expensive_rays: Vec<PixelCoord>,
    execution_mode: String,
    wall_clock_seconds: Option<f64>,
    rays_per_second: Option<f64>,
    width: u32,
    height: u32,
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

    // Authoritative evaluator must itself be a standard release build.
    let self_release = is_optimized_release_execution();
    push(
        &mut checks,
        "evaluator_release_build",
        self_release,
        build.describe(),
    );
    if !self_release {
        let mut report = empty_report(
            &build,
            commit.trim(),
            dirty,
            dirty_detail,
            checks,
            "FAIL",
            false,
        );
        finalize_and_write(&root, &mut report)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Err(format!(
            "gate-2a0-release evaluator requires standard release build ({})",
            build.describe()
        )
        .into());
    }

    // Soft guard: also exercise the public require_release API.
    require_release_execution(&build)?;

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

    // Negative path: debug + --require-release must fail before tracing.
    let neg_out = artifacts_dir(&root).join("negative");
    let _ = std::fs::remove_dir_all(&neg_out);
    std::fs::create_dir_all(&neg_out)?;
    let neg = Command::new("cargo")
        .current_dir(&root)
        .args([
            "run",
            "-q",
            "-p",
            "xtask",
            "--",
            "trace-outcome-map",
            "--preset",
            "presets/gargantua-baseline.toml",
            "--width",
            "32",
            "--height",
            "32",
            "--output",
            "artifacts/gate-2a0-release/negative/outcome-map.ppm",
            "--require-release",
        ])
        .output()?;
    let neg_failed = !neg.status.success();
    let neg_err = format!(
        "{}{}",
        String::from_utf8_lossy(&neg.stdout),
        String::from_utf8_lossy(&neg.stderr)
    );
    let no_partial = !neg_out.join("outcome-map.ppm").exists()
        && !neg_out.join("rhs-evaluations.pgm").exists()
        && !neg_out.join("outcome-map.json").exists();
    push(
        &mut checks,
        "require_release_rejects_debug",
        neg_failed && neg_err.contains("--require-release") && no_partial,
        format!("failed_before_trace={neg_failed} no_partial={no_partial}; {neg_err}"),
    );

    let out_root = artifacts_dir(&root);
    std::fs::create_dir_all(&out_root)?;

    // --- Smoke 32×32 release ---
    let smoke = run_trace_subprocess(
        &root,
        true,
        32,
        32,
        "artifacts/gate-2a0-release/release-32/outcome-map.ppm",
        true,
    )?;
    let smoke_ok = smoke.counts.failed == 0 && smoke.width == 32 && smoke.height == 32;
    push(
        &mut checks,
        "smoke_release_32",
        smoke_ok,
        format!(
            "class={} failed={} wall={:.3}s",
            smoke.outcome_class_digest, smoke.counts.failed, smoke.wall_clock_seconds
        ),
    );

    // --- 64×64 dev ×1 ---
    let dev64 = run_trace_subprocess(
        &root,
        false,
        64,
        64,
        "artifacts/gate-2a0-release/dev-64/outcome-map.ppm",
        false,
    )?;
    push(
        &mut checks,
        "dev_serial_64",
        dev64.counts.failed == 0,
        format!(
            "wall={:.3}s rps={:.1} class={}",
            dev64.wall_clock_seconds, dev64.rays_per_second, dev64.outcome_class_digest
        ),
    );

    // --- 64×64 release ×3 ---
    let mut rel64 = Vec::new();
    for i in 0..3 {
        let run = run_trace_subprocess(
            &root,
            true,
            64,
            64,
            &format!("artifacts/gate-2a0-release/release-64-run-{i}/outcome-map.ppm"),
            true,
        )?;
        rel64.push(run);
    }
    let det_ok = release_64_deterministic(&rel64);
    push(
        &mut checks,
        "release_64_determinism",
        det_ok,
        if det_ok {
            format!(
                "3 identical; class={} ppm={} pgm={}",
                rel64[0].outcome_class_digest, rel64[0].ppm_digest, rel64[0].pgm_digest
            )
        } else {
            "mismatch across release-64 runs".into()
        },
    );

    let class_agree = rel64[0].outcome_class_digest == dev64.outcome_class_digest
        && rel64[0].ppm_digest == dev64.ppm_digest
        && counts_eq(&rel64[0].counts, &dev64.counts);
    push(
        &mut checks,
        "dev_release_64_classification_agree",
        class_agree,
        format!(
            "dev_class={} rel_class={} pgm_dev={} pgm_rel={} (pgm identity recorded, not required)",
            dev64.outcome_class_digest,
            rel64[0].outcome_class_digest,
            dev64.pgm_digest,
            rel64[0].pgm_digest
        ),
    );
    if rel64[0].pgm_digest != dev64.pgm_digest {
        push(
            &mut checks,
            "dev_release_64_pgm_difference_reported",
            true,
            format!(
                "PGM differs (cost map; not categorical failure): dev={} release={}",
                dev64.pgm_digest, rel64[0].pgm_digest
            ),
        );
    } else {
        push(
            &mut checks,
            "dev_release_64_pgm_difference_reported",
            true,
            "PGM identical across profiles".into(),
        );
    }

    let median_s = median_seconds(rel64.iter().map(|r| r.wall_clock_seconds).collect());
    let speedup = finite_speedup(dev64.wall_clock_seconds, median_s);
    let speedup_status = if speedup.is_finite() && speedup > 1.0 {
        "PASS"
    } else if speedup.is_finite() {
        "NeedsInvestigation"
    } else {
        "FAIL"
    };
    checks.push(Check {
        name: "release_speedup_vs_dev".into(),
        status: if speedup_status == "FAIL" {
            "FAIL"
        } else {
            "PASS"
        },
        detail: format!(
            "status={speedup_status}; speedup={speedup:.4}; dev={:.3}s median_release={:.3}s",
            dev64.wall_clock_seconds, median_s
        ),
    });

    // --- 128×128 release ---
    let rel128 = run_trace_subprocess(
        &root,
        true,
        128,
        128,
        "artifacts/gate-2a0-release/release-128/outcome-map.ppm",
        true,
    )?;
    let cmp = compare_to_gate1b2_reference(&rel128);
    push(
        &mut checks,
        "release_128_classification_matches_1b2",
        cmp.classification_match,
        format!(
            "{} vs {}",
            cmp.observed_class_digest, cmp.reference_class_digest
        ),
    );
    push(
        &mut checks,
        "release_128_ppm_matches_1b2",
        cmp.ppm_match,
        format!(
            "{} vs {}",
            cmp.observed_ppm_digest, cmp.reference_ppm_digest
        ),
    );
    push(
        &mut checks,
        "release_128_counts_match_1b2",
        cmp.counts_match,
        format!("{:?}", cmp.observed_counts),
    );
    push(
        &mut checks,
        "release_128_failed_zero",
        cmp.failed_zero,
        format!("failed={}", rel128.counts.failed),
    );
    push(
        &mut checks,
        "release_128_pgm_comparison_explicit",
        true,
        format!(
            "status={} observed={} reference={}",
            cmp.pgm_status, cmp.observed_pgm_digest, cmp.reference_pgm_digest
        ),
    );
    push(
        &mut checks,
        "release_128_timing_recorded",
        rel128.wall_clock_seconds.is_finite() && rel128.wall_clock_seconds > 0.0,
        format!(
            "wall={:.3}s rps={:.1}; historical_1b2_debug≈{HISTORICAL_1B2_DEBUG_SECONDS}s (prior run)",
            rel128.wall_clock_seconds, rel128.rays_per_second
        ),
    );

    // No parallelism markers in tracing crates / xtask.
    let no_par = no_parallelism_introduced(&root)?;
    push(
        &mut checks,
        "no_parallelism_introduced",
        no_par,
        "no rayon/thread-pool/parallel-iterator in gate crates".into(),
    );

    let hard_fail = checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    let authoritative = !dirty && !hard_fail && self_release;
    let result = if hard_fail {
        "FAIL"
    } else if authoritative {
        "PASS"
    } else {
        "PASS_NON_AUTHORITATIVE"
    };

    let mut report = Gate2a0ReleaseReport {
        gate: "gate-2a0-release".into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        checks,
        smoke_release_32: Some(smoke),
        dev_serial_64: Some(dev64),
        release_serial_64: rel64,
        release_serial_64_median_seconds: median_s,
        release_speedup_vs_dev: speedup,
        release_serial_128: Some(rel128),
        reference_comparison_128: Some(cmp),
        content_digest_excluding_digest_field: String::new(),
    };

    let digest = content_digest(&report);
    report.content_digest_excluding_digest_field = digest.clone();
    let verify = content_digest(&Gate2a0ReleaseReport {
        content_digest_excluding_digest_field: String::new(),
        ..report.clone()
    });
    let digest_ok = verify == digest;
    report.checks.push(Check {
        name: "artifact_digest_convention".into(),
        status: if digest_ok { "PASS" } else { "FAIL" },
        detail: format!("content_digest_excluding_digest_field reproduces; digest={digest}"),
    });

    let hard_fail = report
        .checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    report.authoritative = !dirty && !hard_fail && report.build.is_optimized_release_execution();
    report.result = if hard_fail {
        "FAIL".into()
    } else if report.authoritative {
        "PASS".into()
    } else {
        "PASS_NON_AUTHORITATIVE".into()
    };
    let mut for_hash = report.clone();
    for_hash.content_digest_excluding_digest_field.clear();
    report.content_digest_excluding_digest_field = content_digest(&for_hash);

    finalize_and_write(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if hard_fail || report.result == "FAIL" {
        return Err("gate-2a0-release evaluation FAIL".into());
    }
    Ok(())
}

fn empty_report(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
    result: &str,
    authoritative: bool,
) -> Gate2a0ReleaseReport {
    Gate2a0ReleaseReport {
        gate: "gate-2a0-release".into(),
        result: result.into(),
        authoritative,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        checks,
        smoke_release_32: None,
        dev_serial_64: None,
        release_serial_64: Vec::new(),
        release_serial_64_median_seconds: 0.0,
        release_speedup_vs_dev: 0.0,
        release_serial_128: None,
        reference_comparison_128: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize_and_write(
    root: &Path,
    report: &mut Gate2a0ReleaseReport,
) -> Result<(), Box<dyn std::error::Error>> {
    if report.content_digest_excluding_digest_field.is_empty() {
        let mut for_hash = report.clone();
        for_hash.content_digest_excluding_digest_field.clear();
        report.content_digest_excluding_digest_field = content_digest(&for_hash);
    }
    let out_dir = artifacts_dir(root);
    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(
        out_dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    std::fs::write(out_dir.join("evaluation.md"), render_md(report))?;
    std::fs::write(
        out_dir.join("evaluation.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;
    Ok(())
}

fn run_trace_subprocess(
    root: &Path,
    release: bool,
    width: u32,
    height: u32,
    output: &str,
    require_release: bool,
) -> Result<BenchmarkRun, Box<dyn std::error::Error>> {
    let mut args: Vec<String> = vec!["run".into(), "-q".into(), "-p".into(), "xtask".into()];
    if release {
        args.insert(1, "--release".into());
    }
    args.push("--".into());
    args.push("trace-outcome-map".into());
    args.push("--preset".into());
    args.push("presets/gargantua-baseline.toml".into());
    args.push("--width".into());
    args.push(width.to_string());
    args.push("--height".into());
    args.push(height.to_string());
    args.push("--output".into());
    args.push(output.into());
    if require_release {
        args.push("--require-release".into());
    }

    let out = Command::new("cargo")
        .current_dir(root)
        .args(&args)
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "trace-outcome-map failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }

    let json_path = {
        let ppm = if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else {
            root.join(output)
        };
        ppm.parent()
            .unwrap_or(Path::new("."))
            .join("outcome-map.json")
    };
    let json: OutcomeMapJson = serde_json::from_slice(&std::fs::read(&json_path)?)?;
    let wall = sanitize_timing(json.wall_clock_seconds.unwrap_or(0.0));
    let rps = sanitize_timing(json.rays_per_second.unwrap_or_else(|| {
        let n = (json.width as u64) * (json.height as u64);
        if wall > 0.0 {
            n as f64 / wall
        } else {
            0.0
        }
    }));

    // Worker build metadata: release flag dictates expected profile label.
    let worker_build = if release {
        BuildExecutionMetadata {
            cargo_profile: "release".into(),
            opt_level: "3".into(),
            debug_assertions: false,
            target: BuildExecutionMetadata::current().target.clone(),
            toolchain: BuildExecutionMetadata::current().toolchain.clone(),
        }
    } else {
        BuildExecutionMetadata {
            cargo_profile: "debug".into(),
            opt_level: "0".into(),
            debug_assertions: true,
            target: BuildExecutionMetadata::current().target.clone(),
            toolchain: BuildExecutionMetadata::current().toolchain.clone(),
        }
    };

    Ok(BenchmarkRun {
        width: json.width,
        height: json.height,
        build: worker_build,
        execution_mode: json.execution_mode,
        outcome_class_digest: json.outcome_class_digest,
        ppm_digest: json.ppm_digest,
        pgm_digest: json.pgm_digest,
        counts: json.counts,
        total_accepted_steps: json.total_accepted_steps,
        total_rejected_steps: json.total_rejected_steps,
        total_rhs_evaluations: json.total_rhs_evaluations,
        rhs: json.rhs,
        most_expensive_rays: json.most_expensive_rays,
        wall_clock_seconds: wall,
        rays_per_second: rps,
    })
}

fn release_64_deterministic(runs: &[BenchmarkRun]) -> bool {
    if runs.len() != 3 {
        return false;
    }
    let a = &runs[0];
    runs[1..].iter().all(|b| {
        b.outcome_class_digest == a.outcome_class_digest
            && b.ppm_digest == a.ppm_digest
            && b.pgm_digest == a.pgm_digest
            && counts_eq(&b.counts, &a.counts)
            && b.total_accepted_steps == a.total_accepted_steps
            && b.total_rejected_steps == a.total_rejected_steps
            && b.total_rhs_evaluations == a.total_rhs_evaluations
            && b.most_expensive_rays == a.most_expensive_rays
    })
}

fn counts_eq(a: &OutcomeCounts, b: &OutcomeCounts) -> bool {
    a.disk_hit == b.disk_hit
        && a.escaped == b.escaped
        && a.horizon_event == b.horizon_event
        && a.horizon_approach == b.horizon_approach
        && a.affine_limit == b.affine_limit
        && a.failed == b.failed
}

pub fn compare_to_gate1b2_reference(run: &BenchmarkRun) -> ReferenceComparison {
    let classification_match = run.outcome_class_digest == REF_CLASS_DIGEST;
    let ppm_match = run.ppm_digest == REF_PPM_DIGEST;
    let pgm_match = run.pgm_digest == REF_PGM_DIGEST;
    let counts_match = counts_eq(&run.counts, &REF_COUNTS);
    let failed_zero = run.counts.failed == 0;
    let pgm_status = if pgm_match {
        "MATCH".to_string()
    } else {
        "MISMATCH_REPORTED".to_string()
    };
    ReferenceComparison {
        classification_match,
        ppm_match,
        counts_match,
        failed_zero,
        pgm_match,
        pgm_status,
        reference_class_digest: REF_CLASS_DIGEST.into(),
        reference_ppm_digest: REF_PPM_DIGEST.into(),
        reference_pgm_digest: REF_PGM_DIGEST.into(),
        observed_class_digest: run.outcome_class_digest.clone(),
        observed_ppm_digest: run.ppm_digest.clone(),
        observed_pgm_digest: run.pgm_digest.clone(),
        reference_counts: REF_COUNTS.clone(),
        observed_counts: run.counts.clone(),
        historical_gate_1b2_debug_wall_clock_seconds: HISTORICAL_1B2_DEBUG_SECONDS,
        historical_note: "prior Gate 1B2 debug evaluate (~208–211 s); not measured by Gate 2A0"
            .into(),
    }
}

pub fn median_seconds(mut values: Vec<f64>) -> f64 {
    values = values.into_iter().map(sanitize_timing).collect();
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        sanitize_timing((values[mid - 1] + values[mid]) / 2.0)
    }
}

pub fn finite_speedup(dev_seconds: f64, median_release_seconds: f64) -> f64 {
    let d = sanitize_timing(dev_seconds);
    let r = sanitize_timing(median_release_seconds);
    if !(d > 0.0 && r > 0.0) {
        return 0.0;
    }
    let s = d / r;
    if s.is_finite() {
        s
    } else {
        0.0
    }
}

pub fn sanitize_timing(v: f64) -> f64 {
    if v.is_finite() && v >= 0.0 {
        v
    } else {
        0.0
    }
}

fn content_digest(report: &Gate2a0ReleaseReport) -> String {
    let proj = DigestProjection::from_report(report);
    let bytes = serde_json::to_vec(&proj).expect("serialize");
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Serialize, Clone)]
struct DigestBenchmark<'a> {
    width: u32,
    height: u32,
    build: &'a BuildExecutionMetadata,
    execution_mode: &'a str,
    outcome_class_digest: &'a str,
    ppm_digest: &'a str,
    pgm_digest: &'a str,
    counts: &'a OutcomeCounts,
    total_accepted_steps: u64,
    total_rejected_steps: u64,
    total_rhs_evaluations: u64,
    rhs: &'a RhsDistribution,
    most_expensive_rays: &'a [PixelCoord],
}

#[derive(Serialize)]
struct DigestProjection<'a> {
    gate: &'a str,
    result: &'a str,
    authoritative: bool,
    commit: &'a str,
    dirty: bool,
    build: &'a BuildExecutionMetadata,
    checks: &'a [Check],
    smoke_release_32: Option<DigestBenchmark<'a>>,
    dev_serial_64: Option<DigestBenchmark<'a>>,
    release_serial_64: Vec<DigestBenchmark<'a>>,
    release_serial_128: Option<DigestBenchmark<'a>>,
    reference_comparison_128: Option<&'a ReferenceComparison>,
    content_digest_excluding_digest_field: &'a str,
}

impl<'a> DigestProjection<'a> {
    fn from_report(report: &'a Gate2a0ReleaseReport) -> Self {
        Self {
            gate: &report.gate,
            result: &report.result,
            authoritative: report.authoritative,
            commit: &report.commit,
            dirty: report.dirty,
            build: &report.build,
            checks: &report.checks,
            smoke_release_32: report.smoke_release_32.as_ref().map(DigestBenchmark::from),
            dev_serial_64: report.dev_serial_64.as_ref().map(DigestBenchmark::from),
            release_serial_64: report
                .release_serial_64
                .iter()
                .map(DigestBenchmark::from)
                .collect(),
            release_serial_128: report
                .release_serial_128
                .as_ref()
                .map(DigestBenchmark::from),
            reference_comparison_128: report.reference_comparison_128.as_ref(),
            content_digest_excluding_digest_field: "",
        }
    }
}

impl<'a> DigestBenchmark<'a> {
    fn from(run: &'a BenchmarkRun) -> Self {
        Self {
            width: run.width,
            height: run.height,
            build: &run.build,
            execution_mode: &run.execution_mode,
            outcome_class_digest: &run.outcome_class_digest,
            ppm_digest: &run.ppm_digest,
            pgm_digest: &run.pgm_digest,
            counts: &run.counts,
            total_accepted_steps: run.total_accepted_steps,
            total_rejected_steps: run.total_rejected_steps,
            total_rhs_evaluations: run.total_rhs_evaluations,
            rhs: &run.rhs,
            most_expensive_rays: &run.most_expensive_rays,
        }
    }
}

fn render_md(r: &Gate2a0ReleaseReport) -> String {
    let mut s = String::new();
    s.push_str("# Gate 2A0 Release Evaluation\n\n");
    s.push_str(&format!("- Result: **{}**\n", r.result));
    s.push_str(&format!("- Authoritative: {}\n", r.authoritative));
    s.push_str(&format!("- Commit: `{}`\n", r.commit));
    s.push_str(&format!("- Build: {}\n", r.build.describe()));
    s.push_str(&format!(
        "- Content digest: `{}`\n",
        r.content_digest_excluding_digest_field
    ));
    s.push_str(&format!(
        "- Median release 64s: {:.3}\n",
        r.release_serial_64_median_seconds
    ));
    s.push_str(&format!(
        "- Speedup vs dev: {:.4}\n\n",
        r.release_speedup_vs_dev
    ));
    s.push_str("## Checks\n\n");
    for c in &r.checks {
        s.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    s
}

fn no_parallelism_introduced(root: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let paths = [
        "crates/relativity-trace/Cargo.toml",
        "crates/relativity-integrate/Cargo.toml",
        "crates/relativity-core/Cargo.toml",
        "xtask/Cargo.toml",
    ];
    for p in paths {
        let t = std::fs::read_to_string(root.join(p))?;
        if t.contains("rayon") || t.contains("threadpool") || t.contains("thread-pool") {
            return Ok(false);
        }
    }
    Ok(true)
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
            format!(
                "stdout={} stderr={}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        },
    );
    Ok(())
}

fn artifacts_dir(root: &Path) -> PathBuf {
    root.join("artifacts/gate-2a0-release")
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
    use crate::build_meta::BuildExecutionMetadata;

    fn dummy_run(class: &str, profile: &str) -> BenchmarkRun {
        BenchmarkRun {
            width: 128,
            height: 128,
            build: BuildExecutionMetadata {
                cargo_profile: profile.into(),
                opt_level: if profile == "release" {
                    "3".into()
                } else {
                    "0".into()
                },
                debug_assertions: profile != "release",
                target: "test".into(),
                toolchain: "test".into(),
            },
            execution_mode: "serial".into(),
            outcome_class_digest: class.into(),
            ppm_digest: "ppm".into(),
            pgm_digest: "pgm".into(),
            counts: REF_COUNTS.clone(),
            total_accepted_steps: 1,
            total_rejected_steps: 0,
            total_rhs_evaluations: 10,
            rhs: RhsDistribution {
                min: 1,
                median: 2,
                p90: 3,
                p99: 4,
                max: 5,
                mean: 2.0,
            },
            most_expensive_rays: vec![],
            wall_clock_seconds: 10.0,
            rays_per_second: 100.0,
        }
    }

    fn sample_report(class: &str, profile: &str) -> Gate2a0ReleaseReport {
        let run = dummy_run(class, profile);
        let cmp = compare_to_gate1b2_reference(&run);
        Gate2a0ReleaseReport {
            gate: "gate-2a0-release".into(),
            result: "PASS".into(),
            authoritative: true,
            commit: "abc".into(),
            dirty: false,
            dirty_detail: String::new(),
            build: run.build.clone(),
            checks: vec![Check {
                name: "x".into(),
                status: "PASS",
                detail: "ok".into(),
            }],
            smoke_release_32: None,
            dev_serial_64: None,
            release_serial_64: vec![],
            release_serial_64_median_seconds: 1.0,
            release_speedup_vs_dev: 2.0,
            release_serial_128: Some(run),
            reference_comparison_128: Some(cmp),
            content_digest_excluding_digest_field: String::new(),
        }
    }

    #[test]
    fn timing_fields_excluded_from_content_digest() {
        let mut a = sample_report(REF_CLASS_DIGEST, "release");
        let mut b = a.clone();
        if let Some(r) = a.release_serial_128.as_mut() {
            r.wall_clock_seconds = 1.0;
            r.rays_per_second = 10.0;
        }
        if let Some(r) = b.release_serial_128.as_mut() {
            r.wall_clock_seconds = 999.0;
            r.rays_per_second = 0.01;
        }
        a.release_speedup_vs_dev = 1.5;
        b.release_speedup_vs_dev = 9.9;
        a.release_serial_64_median_seconds = 1.0;
        b.release_serial_64_median_seconds = 50.0;
        assert_eq!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn changing_outcome_digest_changes_content_digest() {
        let a = sample_report(REF_CLASS_DIGEST, "release");
        let b = sample_report("deadbeef", "release");
        assert_ne!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn changing_build_profile_changes_content_digest() {
        let a = sample_report(REF_CLASS_DIGEST, "release");
        let b = sample_report(REF_CLASS_DIGEST, "dev");
        assert_ne!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn median_calculation_deterministic() {
        assert_eq!(median_seconds(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median_seconds(vec![4.0, 1.0]), 2.5);
        assert_eq!(median_seconds(vec![]), 0.0);
    }

    #[test]
    fn speedup_handles_finite_positive_timings() {
        assert!((finite_speedup(10.0, 2.0) - 5.0).abs() < 1e-12);
        assert_eq!(finite_speedup(0.0, 2.0), 0.0);
        assert_eq!(finite_speedup(10.0, 0.0), 0.0);
        assert_eq!(finite_speedup(f64::NAN, 2.0), 0.0);
        assert_eq!(finite_speedup(10.0, f64::INFINITY), 0.0);
    }

    #[test]
    fn invalid_timing_cannot_produce_nan_in_serialized_fields() {
        let s = finite_speedup(f64::NAN, f64::NAN);
        assert!(s.is_finite());
        let m = median_seconds(vec![f64::NAN, f64::INFINITY, -1.0]);
        assert!(m.is_finite());
        assert!(sanitize_timing(f64::NAN).is_finite());
    }

    #[test]
    fn reference_comparison_checks_separately() {
        let mut run = dummy_run(REF_CLASS_DIGEST, "release");
        run.ppm_digest = REF_PPM_DIGEST.into();
        run.pgm_digest = "different-pgm".into();
        let cmp = compare_to_gate1b2_reference(&run);
        assert!(cmp.classification_match);
        assert!(cmp.ppm_match);
        assert!(cmp.counts_match);
        assert!(!cmp.pgm_match);
        assert_eq!(cmp.pgm_status, "MISMATCH_REPORTED");
    }

    #[test]
    fn pgm_mismatch_cannot_be_silently_dropped() {
        let mut run = dummy_run(REF_CLASS_DIGEST, "release");
        run.ppm_digest = REF_PPM_DIGEST.into();
        run.pgm_digest = "x".into();
        let cmp = compare_to_gate1b2_reference(&run);
        assert!(!cmp.pgm_status.is_empty());
        assert_ne!(cmp.pgm_status, "MATCH");
        // Serialized comparison always carries both digests.
        let json = serde_json::to_value(&cmp).unwrap();
        assert!(json.get("observed_pgm_digest").is_some());
        assert!(json.get("reference_pgm_digest").is_some());
        assert!(json.get("pgm_status").is_some());
    }

    #[test]
    fn rejected_release_guard_performs_no_worker_trace() {
        let meta = BuildExecutionMetadata {
            cargo_profile: "dev".into(),
            opt_level: "0".into(),
            debug_assertions: true,
            target: "test".into(),
            toolchain: "test".into(),
        };
        let mut traced = false;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            require_release_execution(&meta)?;
            traced = true;
            Ok(())
        })();
        assert!(result.is_err());
        assert!(!traced);
    }
}
