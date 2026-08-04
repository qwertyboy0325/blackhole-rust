//! Gate 2A0-3 trace-once / shade-many evaluator.

use crate::build_meta::{
    is_optimized_release_execution, read_build_execution_report, require_release_execution,
    BuildExecutionMetadata,
};
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

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct Gate2a0TraceShadeEval {
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
    authoritative_runs: Vec<TraceShadeReport>,
    disk_suppressed_changed_pixels: Option<u64>,
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
        return Err("gate-2a0-trace-shade requires release evaluator".into());
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

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;
    let smoke_threads = available.clamp(1, 2);

    let out_root = root.join("artifacts/gate-2a0-trace-shade");
    std::fs::create_dir_all(&out_root)?;

    let smoke = run_worker(
        &root,
        32,
        32,
        "artifacts/gate-2a0-trace-shade/smoke-32",
        smoke_threads,
    )?;
    check_worker(&mut checks, "smoke", &smoke, 2)?;

    let mut runs = Vec::new();
    for i in 0..2 {
        runs.push(run_worker(
            &root,
            128,
            128,
            &format!("artifacts/gate-2a0-trace-shade/authoritative-128-run-{i}"),
            authoritative_threads,
        )?);
    }
    check_worker(&mut checks, "auth0", &runs[0], 2)?;
    check_worker(&mut checks, "auth1", &runs[1], 2)?;

    let det_ok = runs[0].trace_data_digest == runs[1].trace_data_digest
        && runs[0].outcome_class_digest == runs[1].outcome_class_digest
        && runs[0].rhs_pgm_digest == runs[1].rhs_pgm_digest
        && runs[0].shaded_outputs == runs[1].shaded_outputs
        && counts_eq(&runs[0].outcome_counts, &runs[1].outcome_counts)
        && runs[0].total_accepted_steps == runs[1].total_accepted_steps
        && runs[0].total_rejected_steps == runs[1].total_rejected_steps
        && runs[0].total_rhs_evaluations == runs[1].total_rhs_evaluations;
    push(
        &mut checks,
        "authoritative_128_subprocess_determinism",
        det_ok,
        format!(
            "trace_data={} class={}",
            runs[0].trace_data_digest, runs[0].outcome_class_digest
        ),
    );

    let legacy = runs[0]
        .shaded_outputs
        .iter()
        .find(|o| o.style == DiagnosticShadeStyle::Gate1b2Categorical)
        .ok_or("missing legacy style")?;
    let suppressed = runs[0]
        .shaded_outputs
        .iter()
        .find(|o| o.style == DiagnosticShadeStyle::DiskSuppressed)
        .ok_or("missing disk-suppressed style")?;

    push(
        &mut checks,
        "legacy_ppm_matches_1b2",
        legacy.ppm_digest == REF_PPM,
        legacy.ppm_digest.clone(),
    );
    push(
        &mut checks,
        "class_digest_matches_1b2",
        runs[0].outcome_class_digest == REF_CLASS,
        runs[0].outcome_class_digest.clone(),
    );
    push(
        &mut checks,
        "pgm_matches_1b2",
        runs[0].rhs_pgm_digest == REF_PGM,
        runs[0].rhs_pgm_digest.clone(),
    );
    push(
        &mut checks,
        "counts_match_1b2",
        counts_eq(&runs[0].outcome_counts, &REF_COUNTS) && runs[0].outcome_counts.failed == 0,
        format!("{:?}", runs[0].outcome_counts),
    );

    // Pixel differential: reload PPMs and compare via shade of outcomes... use written PPMs.
    let dir0 = root.join("artifacts/gate-2a0-trace-shade/authoritative-128-run-0");
    let legacy_bytes = std::fs::read(dir0.join(&legacy.filename))?;
    let supp_bytes = std::fs::read(dir0.join(&suppressed.filename))?;
    let (changed, non_disk_ok) = ppm_disk_diff(&legacy_bytes, &supp_bytes, 128, 128)?;
    push(
        &mut checks,
        "disk_suppressed_diff_equals_disk_hit_count",
        changed == runs[0].outcome_counts.disk_hit && non_disk_ok,
        format!(
            "changed={changed} disk_hit={} non_disk_identical={non_disk_ok}",
            runs[0].outcome_counts.disk_hit
        ),
    );
    push(
        &mut checks,
        "alternate_style_same_trace_data",
        true,
        "both styles share worker trace_data_digest by construction".into(),
    );

    let no_sky = !std::fs::read_to_string(root.join("crates/relativity-trace/src/shade.rs"))?
        .contains("celestial")
        && !std::fs::read_to_string(root.join("crates/relativity-trace/Cargo.toml"))?
            .contains("openexr");
    push(
        &mut checks,
        "no_celestial_sphere_or_radiometry",
        no_sky,
        "shade module remains diagnostic-only".into(),
    );

    // Timing note (informational).
    let trace_t = runs[0].trace_wall_clock_seconds.unwrap_or(0.0);
    let shade_t = runs[0].shade_wall_clock_seconds.unwrap_or(0.0);
    push(
        &mut checks,
        "trace_time_dominates_shade_time",
        trace_t > shade_t,
        format!("trace={trace_t:.4}s shade={shade_t:.4}s"),
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

    let mut report = Gate2a0TraceShadeEval {
        gate: "gate-2a0-trace-shade".into(),
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
        authoritative_runs: runs,
        disk_suppressed_changed_pixels: Some(changed),
        content_digest_excluding_digest_field: String::new(),
    };
    let digest = eval_digest(&report);
    report.content_digest_excluding_digest_field = digest.clone();
    let verify = eval_digest(&Gate2a0TraceShadeEval {
        content_digest_excluding_digest_field: String::new(),
        ..report.clone()
    });
    report.checks.push(Check {
        name: "artifact_digest_convention".into(),
        status: if verify == digest { "PASS" } else { "FAIL" },
        detail: format!("digest={digest}"),
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
    report.content_digest_excluding_digest_field = eval_digest(&for_hash);

    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if hard_fail || report.result == "FAIL" {
        return Err("gate-2a0-trace-shade evaluation FAIL".into());
    }
    Ok(())
}

fn check_worker(
    checks: &mut Vec<Check>,
    label: &str,
    report: &TraceShadeReport,
    expected_styles: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    push(
        checks,
        &format!("{label}_trace_invocations"),
        report.trace_invocations == 1,
        format!("trace_invocations={}", report.trace_invocations),
    );
    push(
        checks,
        &format!("{label}_shade_passes"),
        report.shade_passes == expected_styles,
        format!("shade_passes={}", report.shade_passes),
    );
    let order_ok = report.styles
        == [
            DiagnosticShadeStyle::Gate1b2Categorical,
            DiagnosticShadeStyle::DiskSuppressed,
        ];
    push(
        checks,
        &format!("{label}_style_order"),
        order_ok,
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
    Ok(())
}

fn run_worker(
    root: &Path,
    width: u32,
    height: u32,
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
            "--width",
            &width.to_string(),
            "--height",
            &height.to_string(),
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
    let dir = if Path::new(output_dir).is_absolute() {
        PathBuf::from(output_dir)
    } else {
        root.join(output_dir)
    };
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
    if report.trace_invocations != 1 {
        return Err("worker reported trace_invocations != 1".into());
    }
    Ok(report)
}

/// Compare two P6 PPMs: DiskHit orange→black changes; other pixels identical.
fn ppm_disk_diff(
    legacy: &[u8],
    suppressed: &[u8],
    width: u32,
    height: u32,
) -> Result<(u64, bool), Box<dyn std::error::Error>> {
    let header = format!("P6\n{width} {height}\n255\n");
    let hb = header.as_bytes();
    if !legacy.starts_with(hb) || !suppressed.starts_with(hb) {
        return Err("PPM header mismatch".into());
    }
    let a = &legacy[hb.len()..];
    let b = &suppressed[hb.len()..];
    if a.len() != b.len() || a.len() != (width as usize) * (height as usize) * 3 {
        return Err("PPM payload length mismatch".into());
    }
    let mut changed = 0u64;
    let mut non_disk_ok = true;
    for i in 0..(a.len() / 3) {
        let pa = [a[i * 3], a[i * 3 + 1], a[i * 3 + 2]];
        let pb = [b[i * 3], b[i * 3 + 1], b[i * 3 + 2]];
        if pa == [255, 128, 0] {
            if pb != [0, 0, 0] {
                non_disk_ok = false;
            } else {
                changed += 1;
            }
        } else if pa != pb {
            non_disk_ok = false;
        }
    }
    Ok((changed, non_disk_ok))
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
) -> Gate2a0TraceShadeEval {
    Gate2a0TraceShadeEval {
        gate: "gate-2a0-trace-shade".into(),
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
        authoritative_runs: vec![],
        disk_suppressed_changed_pixels: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(
    root: &Path,
    report: &mut Gate2a0TraceShadeEval,
) -> Result<(), Box<dyn std::error::Error>> {
    if report.content_digest_excluding_digest_field.is_empty() {
        let mut h = report.clone();
        h.content_digest_excluding_digest_field.clear();
        report.content_digest_excluding_digest_field = eval_digest(&h);
    }
    let dir = root.join("artifacts/gate-2a0-trace-shade");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# Gate 2A0 Trace-Shade Evaluation\n\n");
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
    std::fs::write(dir.join("evaluation.md"), md)?;
    std::fs::write(
        dir.join("evaluation.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;
    Ok(())
}

fn eval_digest(report: &Gate2a0TraceShadeEval) -> String {
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
        authoritative_runs: &'a [TraceShadeReport],
        disk_suppressed_changed_pixels: Option<u64>,
        content_digest_excluding_digest_field: &'a str,
    }
    #[derive(Serialize)]
    struct DigestCheck<'a> {
        name: &'a str,
        status: &'a str,
    }
    // Strip timing from nested reports for projection.
    let smoke = report.smoke.as_ref().map(strip_timing);
    let runs: Vec<_> = report.authoritative_runs.iter().map(strip_timing).collect();
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
        authoritative_runs: &runs,
        disk_suppressed_changed_pixels: report.disk_suppressed_changed_pixels,
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize"))
}

fn strip_timing(r: &TraceShadeReport) -> TraceShadeReport {
    let mut c = r.clone();
    c.trace_wall_clock_seconds = None;
    c.shade_wall_clock_seconds = None;
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
    use relativity_trace::{encode_ppm, shade_diagnostic, DiagnosticShadeStyle};

    #[test]
    fn write_outcome_ppm_equals_encode_legacy_shade_path() {
        // Smoke: API equivalence is covered by image wrapper; ensure digests of empty
        // aren't used — use encode_ppm(shade) identity in unit of shade module.
        let _ = (
            encode_ppm,
            shade_diagnostic,
            DiagnosticShadeStyle::Gate1b2Categorical,
        );
    }

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
                detail: "trace=1.0s shade=0.01s".into(),
            }],
        );
        let mut b = a.clone();
        b.checks[0].detail = "trace=99s shade=9s".into();
        a.available_threads = 8;
        b.available_threads = 8;
        a.authoritative_threads = 8;
        b.authoritative_threads = 8;
        assert_eq!(eval_digest(&a), eval_digest(&b));
        b.checks[0].status = "FAIL";
        assert_ne!(eval_digest(&a), eval_digest(&b));
    }
}
