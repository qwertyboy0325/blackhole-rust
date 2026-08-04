//! Gate 2A0-2 deterministic CPU parallelism evaluator.

use crate::build_meta::{
    is_optimized_release_execution, read_build_execution_report, require_release_execution,
    BuildExecutionMetadata,
};
use crate::trace_outcome_map::read_trace_execution_report;
use relativity_trace::{
    OutcomeCounts, PixelCoord, RhsDistribution, TraceExecutionMetadata, TraceExecutionMode,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

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

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ParallelPerformanceStatus {
    Verified,
    NeedsInvestigation,
    Unavailable,
}

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct BenchmarkRun {
    width: u32,
    height: u32,
    build: BuildExecutionMetadata,
    execution: TraceExecutionMetadata,
    outcome_class_digest: String,
    ppm_digest: String,
    pgm_digest: String,
    counts: OutcomeCounts,
    total_accepted_steps: u64,
    total_rejected_steps: u64,
    total_rhs_evaluations: u64,
    rhs: RhsDistribution,
    most_expensive_rays: Vec<PixelCoord>,
    failure_counts_digest: String,
    wall_clock_seconds: f64,
    rays_per_second: f64,
}

#[derive(Serialize, Clone)]
struct ReferenceComparison {
    classification_match: bool,
    ppm_match: bool,
    counts_match: bool,
    failed_zero: bool,
    pgm_match: bool,
    pgm_status: String,
    reference_class_digest: String,
    reference_ppm_digest: String,
    reference_pgm_digest: String,
    observed_class_digest: String,
    observed_ppm_digest: String,
    observed_pgm_digest: String,
    reference_counts: OutcomeCounts,
    observed_counts: OutcomeCounts,
}

#[derive(Serialize, Clone)]
struct EquivalenceResult {
    name: String,
    match_ok: bool,
    detail: String,
}

#[derive(Serialize, Clone)]
struct Gate2a0ParallelReport {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    performance_status: ParallelPerformanceStatus,
    checks: Vec<Check>,
    smoke_parallel_32: Option<BenchmarkRun>,
    serial_64: Option<BenchmarkRun>,
    parallel_64: Vec<BenchmarkRun>,
    parallel_64_median_seconds: f64,
    parallel_speedup_64: f64,
    serial_128: Option<BenchmarkRun>,
    parallel_128: Vec<BenchmarkRun>,
    parallel_128_median_seconds: f64,
    parallel_speedup_128: f64,
    thread_count_equivalence: Vec<BenchmarkRun>,
    equivalence: Vec<EquivalenceResult>,
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
    failure_counts: Vec<relativity_trace::FailureCount>,
    execution_mode: String,
    thread_count: Option<usize>,
    scheduler: Option<String>,
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

    let self_release = is_optimized_release_execution();
    push(
        &mut checks,
        "evaluator_release_build",
        self_release,
        build.describe(),
    );
    if !self_release {
        let mut report = empty_report(&build, commit.trim(), dirty, dirty_detail, checks);
        finalize_and_write(&root, &mut report)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Err(format!(
            "gate-2a0-parallel evaluator requires standard release build ({})",
            build.describe()
        )
        .into());
    }
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

    let available_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available_threads;
    push(
        &mut checks,
        "available_parallelism_recorded",
        available_threads >= 1,
        format!(
            "available_threads={available_threads} authoritative_threads={authoritative_threads}"
        ),
    );

    let out_root = artifacts_dir(&root);
    std::fs::create_dir_all(&out_root)?;

    let smoke_threads = if available_threads >= 2 { 2 } else { 1 };
    let smoke = run_map(
        &root,
        32,
        32,
        "artifacts/gate-2a0-parallel/parallel-smoke-32/outcome-map.ppm",
        true,
        Some(smoke_threads),
    )?;
    push(
        &mut checks,
        "smoke_parallel_32",
        smoke.counts.failed == 0
            && smoke.execution.mode == TraceExecutionMode::Parallel
            && smoke.execution.thread_count == smoke_threads,
        format!(
            "threads={} class={} wall={:.3}s",
            smoke.execution.thread_count, smoke.outcome_class_digest, smoke.wall_clock_seconds
        ),
    );

    let serial_64 = run_map(
        &root,
        64,
        64,
        "artifacts/gate-2a0-parallel/serial-64/outcome-map.ppm",
        false,
        None,
    )?;
    push(
        &mut checks,
        "serial_64",
        serial_64.execution.mode == TraceExecutionMode::Serial && serial_64.counts.failed == 0,
        format!(
            "wall={:.3}s class={}",
            serial_64.wall_clock_seconds, serial_64.outcome_class_digest
        ),
    );

    let mut parallel_64 = Vec::new();
    for i in 0..3 {
        parallel_64.push(run_map(
            &root,
            64,
            64,
            &format!("artifacts/gate-2a0-parallel/parallel-64-run-{i}/outcome-map.ppm"),
            true,
            Some(authoritative_threads),
        )?);
    }
    let p64_det = runs_identical(&parallel_64);
    push(
        &mut checks,
        "parallel_64_determinism",
        p64_det,
        if p64_det {
            format!("3 identical; class={}", parallel_64[0].outcome_class_digest)
        } else {
            "mismatch across parallel-64 runs".into()
        },
    );
    let eq_64 = numerical_equal(&serial_64, &parallel_64[0]);
    push(
        &mut checks,
        "serial_parallel_64_byte_identity",
        eq_64,
        format!(
            "serial={} parallel={}",
            serial_64.outcome_class_digest, parallel_64[0].outcome_class_digest
        ),
    );

    let serial_128 = run_map(
        &root,
        128,
        128,
        "artifacts/gate-2a0-parallel/serial-128/outcome-map.ppm",
        false,
        None,
    )?;
    push(
        &mut checks,
        "serial_128",
        serial_128.counts.failed == 0,
        format!(
            "wall={:.3}s class={}",
            serial_128.wall_clock_seconds, serial_128.outcome_class_digest
        ),
    );

    let mut parallel_128 = Vec::new();
    for i in 0..3 {
        parallel_128.push(run_map(
            &root,
            128,
            128,
            &format!("artifacts/gate-2a0-parallel/parallel-128-run-{i}/outcome-map.ppm"),
            true,
            Some(authoritative_threads),
        )?);
    }
    let p128_det = runs_identical(&parallel_128);
    push(
        &mut checks,
        "parallel_128_determinism",
        p128_det,
        if p128_det {
            format!(
                "3 identical; class={}",
                parallel_128[0].outcome_class_digest
            )
        } else {
            "mismatch".into()
        },
    );
    let eq_128 = numerical_equal(&serial_128, &parallel_128[0]);
    push(
        &mut checks,
        "serial_parallel_128_byte_identity",
        eq_128,
        format!(
            "serial={} parallel={}",
            serial_128.outcome_class_digest, parallel_128[0].outcome_class_digest
        ),
    );

    let cmp = compare_to_gate1b2(&parallel_128[0]);
    push(
        &mut checks,
        "parallel_128_classification_matches_1b2",
        cmp.classification_match,
        cmp.observed_class_digest.clone(),
    );
    push(
        &mut checks,
        "parallel_128_ppm_matches_1b2",
        cmp.ppm_match,
        cmp.observed_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "parallel_128_pgm_matches_1b2",
        cmp.pgm_match,
        format!("status={}", cmp.pgm_status),
    );
    push(
        &mut checks,
        "parallel_128_counts_match_1b2",
        cmp.counts_match && cmp.failed_zero,
        format!("{:?}", cmp.observed_counts),
    );

    // Cross-thread-count equivalence on 32×32.
    let mut thread_runs = Vec::new();
    let mut thread_set = vec![1usize, 2, authoritative_threads];
    thread_set.sort_unstable();
    thread_set.dedup();
    for t in &thread_set {
        thread_runs.push(run_map(
            &root,
            32,
            32,
            &format!(
                "artifacts/gate-2a0-parallel/thread-count-equivalence/threads-{t}/outcome-map.ppm"
            ),
            true,
            Some(*t),
        )?);
    }
    let thread_eq = runs_identical(&thread_runs);
    push(
        &mut checks,
        "cross_thread_count_equivalence",
        thread_eq,
        format!("threads={thread_set:?} identical={thread_eq}"),
    );

    let median_64 = median_seconds(parallel_64.iter().map(|r| r.wall_clock_seconds).collect());
    let median_128 = median_seconds(parallel_128.iter().map(|r| r.wall_clock_seconds).collect());
    let speedup_64 = finite_speedup(serial_64.wall_clock_seconds, median_64);
    let speedup_128 = finite_speedup(serial_128.wall_clock_seconds, median_128);

    let performance_status = if available_threads < 2 {
        ParallelPerformanceStatus::Unavailable
    } else if speedup_64 > 1.0 && speedup_128 > 1.0 {
        ParallelPerformanceStatus::Verified
    } else {
        ParallelPerformanceStatus::NeedsInvestigation
    };
    checks.push(Check {
        name: "parallel_performance_status".into(),
        status: "PASS",
        detail: format!(
            "status={performance_status:?}; speedup64={speedup_64:.4}; speedup128={speedup_128:.4}; serial64={:.3}s median_par64={median_64:.3}s serial128={:.3}s median_par128={median_128:.3}s",
            serial_64.wall_clock_seconds, serial_128.wall_clock_seconds
        ),
    });

    let worker_meta_ok = smoke.build.is_optimized_release_execution()
        && serial_64.build.is_optimized_release_execution()
        && parallel_64
            .iter()
            .all(|r| r.build.is_optimized_release_execution())
        && parallel_128
            .iter()
            .all(|r| r.build.is_optimized_release_execution())
        && smoke.execution.scheduler == "rayon-indexed-work-stealing"
        && serial_64.execution.scheduler == "serial-row-major";
    push(
        &mut checks,
        "worker_execution_metadata_from_worker_report",
        worker_meta_ok,
        format!(
            "smoke_exec={:?} serial_exec={:?} par_threads={}",
            smoke.execution, serial_64.execution, parallel_64[0].execution.thread_count
        ),
    );

    let no_par_deps_ok = !std::fs::read_to_string(root.join("crates/relativity-core/Cargo.toml"))?
        .contains("rayon")
        && !std::fs::read_to_string(root.join("crates/relativity-integrate/Cargo.toml"))?
            .contains("rayon");
    push(
        &mut checks,
        "rayon_confined_to_trace",
        no_par_deps_ok
            && std::fs::read_to_string(root.join("crates/relativity-trace/Cargo.toml"))?
                .contains("rayon"),
        "rayon only in relativity-trace".into(),
    );

    let equivalence = vec![
        EquivalenceResult {
            name: "serial_vs_parallel_64".into(),
            match_ok: eq_64,
            detail: "byte-identical numerical/image digests".into(),
        },
        EquivalenceResult {
            name: "serial_vs_parallel_128".into(),
            match_ok: eq_128,
            detail: "byte-identical numerical/image digests".into(),
        },
        EquivalenceResult {
            name: "cross_thread_count_32".into(),
            match_ok: thread_eq,
            detail: format!("threads={thread_set:?}"),
        },
    ];

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

    let mut report = Gate2a0ParallelReport {
        gate: "gate-2a0-parallel".into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads,
        authoritative_threads,
        performance_status,
        checks,
        smoke_parallel_32: Some(smoke),
        serial_64: Some(serial_64),
        parallel_64,
        parallel_64_median_seconds: median_64,
        parallel_speedup_64: speedup_64,
        serial_128: Some(serial_128),
        parallel_128,
        parallel_128_median_seconds: median_128,
        parallel_speedup_128: speedup_128,
        thread_count_equivalence: thread_runs,
        equivalence,
        reference_comparison_128: Some(cmp),
        content_digest_excluding_digest_field: String::new(),
    };

    let digest = content_digest(&report);
    report.content_digest_excluding_digest_field = digest.clone();
    let verify = content_digest(&Gate2a0ParallelReport {
        content_digest_excluding_digest_field: String::new(),
        ..report.clone()
    });
    report.checks.push(Check {
        name: "artifact_digest_convention".into(),
        status: if verify == digest { "PASS" } else { "FAIL" },
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
        return Err("gate-2a0-parallel evaluation FAIL".into());
    }
    Ok(())
}

fn empty_report(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2a0ParallelReport {
    Gate2a0ParallelReport {
        gate: "gate-2a0-parallel".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        performance_status: ParallelPerformanceStatus::Unavailable,
        checks,
        smoke_parallel_32: None,
        serial_64: None,
        parallel_64: Vec::new(),
        parallel_64_median_seconds: 0.0,
        parallel_speedup_64: 0.0,
        serial_128: None,
        parallel_128: Vec::new(),
        parallel_128_median_seconds: 0.0,
        parallel_speedup_128: 0.0,
        thread_count_equivalence: Vec::new(),
        equivalence: Vec::new(),
        reference_comparison_128: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize_and_write(
    root: &Path,
    report: &mut Gate2a0ParallelReport,
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

fn run_map(
    root: &Path,
    width: u32,
    height: u32,
    output: &str,
    parallel: bool,
    threads: Option<usize>,
) -> Result<BenchmarkRun, Box<dyn std::error::Error>> {
    let mut args: Vec<String> = vec![
        "run".into(),
        "--release".into(),
        "-q".into(),
        "-p".into(),
        "xtask".into(),
        "--".into(),
        "trace-outcome-map".into(),
        "--preset".into(),
        "presets/gargantua-baseline.toml".into(),
        "--width".into(),
        width.to_string(),
        "--height".into(),
        height.to_string(),
        "--output".into(),
        output.into(),
        "--require-release".into(),
        "--execution".into(),
        if parallel {
            "parallel".into()
        } else {
            "serial".into()
        },
    ];
    if let Some(t) = threads {
        args.push("--threads".into());
        args.push(t.to_string());
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

    let out_dir = {
        let ppm = if Path::new(output).is_absolute() {
            PathBuf::from(output)
        } else {
            root.join(output)
        };
        ppm.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let json: OutcomeMapJson =
        serde_json::from_slice(&std::fs::read(out_dir.join("outcome-map.json"))?)?;
    let worker_build = read_build_execution_report(&out_dir)?;
    let worker_exec = read_trace_execution_report(&out_dir)?;
    if !worker_build.is_optimized_release_execution() {
        return Err(format!("worker build is not release ({})", worker_build.describe()).into());
    }
    if parallel {
        if worker_exec.mode != TraceExecutionMode::Parallel {
            return Err(format!("expected parallel worker metadata, got {worker_exec:?}").into());
        }
        if let Some(t) = threads {
            if worker_exec.thread_count != t {
                return Err(format!(
                    "worker thread_count {} != requested {t}",
                    worker_exec.thread_count
                )
                .into());
            }
        }
    } else if worker_exec.mode != TraceExecutionMode::Serial || worker_exec.thread_count != 1 {
        return Err(format!("expected serial worker metadata, got {worker_exec:?}").into());
    }

    // Prefer adjacent worker metadata over JSON fields (still cross-check).
    if json.execution_mode != worker_exec.mode.as_str() {
        return Err(format!(
            "outcome-map execution_mode={} disagrees with worker {}",
            json.execution_mode,
            worker_exec.mode.as_str()
        )
        .into());
    }
    let _ = (json.thread_count, json.scheduler);

    let wall = sanitize_timing(json.wall_clock_seconds.unwrap_or(0.0));
    let rps = sanitize_timing(json.rays_per_second.unwrap_or_else(|| {
        let n = (json.width as u64) * (json.height as u64);
        if wall > 0.0 {
            n as f64 / wall
        } else {
            0.0
        }
    }));
    let failure_counts_digest = {
        let bytes = serde_json::to_vec(&json.failure_counts)?;
        hex_digest(&bytes)
    };

    Ok(BenchmarkRun {
        width: json.width,
        height: json.height,
        build: worker_build,
        execution: worker_exec,
        outcome_class_digest: json.outcome_class_digest,
        ppm_digest: json.ppm_digest,
        pgm_digest: json.pgm_digest,
        counts: json.counts,
        total_accepted_steps: json.total_accepted_steps,
        total_rejected_steps: json.total_rejected_steps,
        total_rhs_evaluations: json.total_rhs_evaluations,
        rhs: json.rhs,
        most_expensive_rays: json.most_expensive_rays,
        failure_counts_digest,
        wall_clock_seconds: wall,
        rays_per_second: rps,
    })
}

fn numerical_equal(a: &BenchmarkRun, b: &BenchmarkRun) -> bool {
    a.outcome_class_digest == b.outcome_class_digest
        && a.ppm_digest == b.ppm_digest
        && a.pgm_digest == b.pgm_digest
        && counts_eq(&a.counts, &b.counts)
        && a.total_accepted_steps == b.total_accepted_steps
        && a.total_rejected_steps == b.total_rejected_steps
        && a.total_rhs_evaluations == b.total_rhs_evaluations
        && rhs_eq(&a.rhs, &b.rhs)
        && a.most_expensive_rays == b.most_expensive_rays
        && a.failure_counts_digest == b.failure_counts_digest
}

fn runs_identical(runs: &[BenchmarkRun]) -> bool {
    if runs.is_empty() {
        return false;
    }
    runs[1..].iter().all(|r| numerical_equal(&runs[0], r))
}

fn counts_eq(a: &OutcomeCounts, b: &OutcomeCounts) -> bool {
    a.disk_hit == b.disk_hit
        && a.escaped == b.escaped
        && a.horizon_event == b.horizon_event
        && a.horizon_approach == b.horizon_approach
        && a.affine_limit == b.affine_limit
        && a.failed == b.failed
}

fn rhs_eq(a: &RhsDistribution, b: &RhsDistribution) -> bool {
    a.min == b.min
        && a.median == b.median
        && a.p90 == b.p90
        && a.p99 == b.p99
        && a.max == b.max
        && a.mean.to_bits() == b.mean.to_bits()
}

fn compare_to_gate1b2(run: &BenchmarkRun) -> ReferenceComparison {
    let pgm_match = run.pgm_digest == REF_PGM_DIGEST;
    ReferenceComparison {
        classification_match: run.outcome_class_digest == REF_CLASS_DIGEST,
        ppm_match: run.ppm_digest == REF_PPM_DIGEST,
        counts_match: counts_eq(&run.counts, &REF_COUNTS),
        failed_zero: run.counts.failed == 0,
        pgm_match,
        pgm_status: if pgm_match {
            "MATCH".into()
        } else {
            "MISMATCH_REPORTED".into()
        },
        reference_class_digest: REF_CLASS_DIGEST.into(),
        reference_ppm_digest: REF_PPM_DIGEST.into(),
        reference_pgm_digest: REF_PGM_DIGEST.into(),
        observed_class_digest: run.outcome_class_digest.clone(),
        observed_ppm_digest: run.ppm_digest.clone(),
        observed_pgm_digest: run.pgm_digest.clone(),
        reference_counts: REF_COUNTS.clone(),
        observed_counts: run.counts.clone(),
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

pub fn finite_speedup(serial: f64, parallel_median: f64) -> f64 {
    let s = sanitize_timing(serial);
    let p = sanitize_timing(parallel_median);
    if s > 0.0 && p > 0.0 {
        let v = s / p;
        if v.is_finite() {
            return v;
        }
    }
    0.0
}

pub fn sanitize_timing(v: f64) -> f64 {
    if v.is_finite() && v >= 0.0 {
        v
    } else {
        0.0
    }
}

fn content_digest(report: &Gate2a0ParallelReport) -> String {
    let proj = DigestProjection::from_report(report);
    hex_digest(&serde_json::to_vec(&proj).expect("serialize"))
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Serialize)]
struct DigestCheck<'a> {
    name: &'a str,
    status: &'a str,
}

#[derive(Serialize)]
struct DigestRun<'a> {
    width: u32,
    height: u32,
    build: &'a BuildExecutionMetadata,
    execution: &'a TraceExecutionMetadata,
    outcome_class_digest: &'a str,
    ppm_digest: &'a str,
    pgm_digest: &'a str,
    counts: &'a OutcomeCounts,
    total_accepted_steps: u64,
    total_rejected_steps: u64,
    total_rhs_evaluations: u64,
    rhs: &'a RhsDistribution,
    most_expensive_rays: &'a [PixelCoord],
    failure_counts_digest: &'a str,
}

#[derive(Serialize)]
struct DigestProjection<'a> {
    gate: &'a str,
    result: &'a str,
    authoritative: bool,
    commit: &'a str,
    dirty: bool,
    build: &'a BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    performance_status: &'a ParallelPerformanceStatus,
    checks: Vec<DigestCheck<'a>>,
    smoke_parallel_32: Option<DigestRun<'a>>,
    serial_64: Option<DigestRun<'a>>,
    parallel_64: Vec<DigestRun<'a>>,
    serial_128: Option<DigestRun<'a>>,
    parallel_128: Vec<DigestRun<'a>>,
    thread_count_equivalence: Vec<DigestRun<'a>>,
    equivalence: &'a [EquivalenceResult],
    reference_comparison_128: Option<&'a ReferenceComparison>,
    content_digest_excluding_digest_field: &'a str,
}

impl<'a> DigestProjection<'a> {
    fn from_report(report: &'a Gate2a0ParallelReport) -> Self {
        Self {
            gate: &report.gate,
            result: &report.result,
            authoritative: report.authoritative,
            commit: &report.commit,
            dirty: report.dirty,
            build: &report.build,
            available_threads: report.available_threads,
            authoritative_threads: report.authoritative_threads,
            performance_status: &report.performance_status,
            checks: report
                .checks
                .iter()
                .map(|c| DigestCheck {
                    name: &c.name,
                    status: c.status,
                })
                .collect(),
            smoke_parallel_32: report.smoke_parallel_32.as_ref().map(DigestRun::from),
            serial_64: report.serial_64.as_ref().map(DigestRun::from),
            parallel_64: report.parallel_64.iter().map(DigestRun::from).collect(),
            serial_128: report.serial_128.as_ref().map(DigestRun::from),
            parallel_128: report.parallel_128.iter().map(DigestRun::from).collect(),
            thread_count_equivalence: report
                .thread_count_equivalence
                .iter()
                .map(DigestRun::from)
                .collect(),
            equivalence: &report.equivalence,
            reference_comparison_128: report.reference_comparison_128.as_ref(),
            content_digest_excluding_digest_field: "",
        }
    }
}

impl<'a> DigestRun<'a> {
    fn from(run: &'a BenchmarkRun) -> Self {
        Self {
            width: run.width,
            height: run.height,
            build: &run.build,
            execution: &run.execution,
            outcome_class_digest: &run.outcome_class_digest,
            ppm_digest: &run.ppm_digest,
            pgm_digest: &run.pgm_digest,
            counts: &run.counts,
            total_accepted_steps: run.total_accepted_steps,
            total_rejected_steps: run.total_rejected_steps,
            total_rhs_evaluations: run.total_rhs_evaluations,
            rhs: &run.rhs,
            most_expensive_rays: &run.most_expensive_rays,
            failure_counts_digest: &run.failure_counts_digest,
        }
    }
}

fn render_md(r: &Gate2a0ParallelReport) -> String {
    let mut s = String::new();
    s.push_str("# Gate 2A0 Parallel Evaluation\n\n");
    s.push_str(&format!("- Result: **{}**\n", r.result));
    s.push_str(&format!("- Authoritative: {}\n", r.authoritative));
    s.push_str(&format!("- Commit: `{}`\n", r.commit));
    s.push_str(&format!(
        "- Threads available/authoritative: {}/{}\n",
        r.available_threads, r.authoritative_threads
    ));
    s.push_str(&format!("- Performance: {:?}\n", r.performance_status));
    s.push_str(&format!(
        "- Content digest: `{}`\n\n",
        r.content_digest_excluding_digest_field
    ));
    s.push_str("## Checks\n\n");
    for c in &r.checks {
        s.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    s
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
    root.join("artifacts/gate-2a0-parallel")
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
    use relativity_trace::TraceExecution;

    fn dummy_run(class: &str, threads: usize) -> BenchmarkRun {
        let execution = if threads == 0 {
            TraceExecution::Serial.metadata()
        } else {
            TraceExecution::parallel(std::num::NonZeroUsize::new(threads.max(1)).unwrap())
                .metadata()
        };
        BenchmarkRun {
            width: 32,
            height: 32,
            build: BuildExecutionMetadata {
                cargo_profile: "release".into(),
                opt_level: "3".into(),
                debug_assertions: false,
                target: "test".into(),
                toolchain: "test".into(),
            },
            execution,
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
            failure_counts_digest: "fail".into(),
            wall_clock_seconds: 1.0,
            rays_per_second: 100.0,
        }
    }

    fn sample_report() -> Gate2a0ParallelReport {
        let run = dummy_run("abc", 4);
        Gate2a0ParallelReport {
            gate: "gate-2a0-parallel".into(),
            result: "PASS".into(),
            authoritative: true,
            commit: "c".into(),
            dirty: false,
            dirty_detail: String::new(),
            build: run.build.clone(),
            available_threads: 8,
            authoritative_threads: 8,
            performance_status: ParallelPerformanceStatus::Verified,
            checks: vec![Check {
                name: "x".into(),
                status: "PASS",
                detail: "wall=1.0s".into(),
            }],
            smoke_parallel_32: Some(run.clone()),
            serial_64: None,
            parallel_64: vec![],
            parallel_64_median_seconds: 1.0,
            parallel_speedup_64: 2.0,
            serial_128: None,
            parallel_128: vec![],
            parallel_128_median_seconds: 1.0,
            parallel_speedup_128: 2.0,
            thread_count_equivalence: vec![],
            equivalence: vec![],
            reference_comparison_128: None,
            content_digest_excluding_digest_field: String::new(),
        }
    }

    #[test]
    fn timing_bearing_check_detail_excluded() {
        let mut a = sample_report();
        let mut b = a.clone();
        a.checks[0].detail = "wall=1.0s rps=10 speedup=2".into();
        b.checks[0].detail = "wall=99.0s rps=0.1 speedup=0.01".into();
        a.parallel_speedup_64 = 1.0;
        b.parallel_speedup_64 = 9.0;
        a.parallel_64_median_seconds = 1.0;
        b.parallel_64_median_seconds = 50.0;
        if let Some(r) = a.smoke_parallel_32.as_mut() {
            r.wall_clock_seconds = 1.0;
        }
        if let Some(r) = b.smoke_parallel_32.as_mut() {
            r.wall_clock_seconds = 99.0;
        }
        assert_eq!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn execution_thread_count_changes_digest() {
        let mut a = sample_report();
        let mut b = sample_report();
        if let Some(r) = a.smoke_parallel_32.as_mut() {
            r.execution.thread_count = 2;
        }
        if let Some(r) = b.smoke_parallel_32.as_mut() {
            r.execution.thread_count = 8;
        }
        assert_ne!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn check_status_changes_digest() {
        let mut a = sample_report();
        let mut b = sample_report();
        a.checks[0].status = "PASS";
        b.checks[0].status = "FAIL";
        assert_ne!(content_digest(&a), content_digest(&b));
    }

    #[test]
    fn performance_status_enum_in_digest_without_timings() {
        let mut a = sample_report();
        let mut b = a.clone();
        a.performance_status = ParallelPerformanceStatus::Verified;
        b.performance_status = ParallelPerformanceStatus::NeedsInvestigation;
        assert_ne!(content_digest(&a), content_digest(&b));
    }
}
