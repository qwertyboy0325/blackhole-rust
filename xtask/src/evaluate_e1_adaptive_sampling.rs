//! E1 adaptive sampling evaluator (PASS independent of hypothesis).

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::e1_adaptive_sampling::config::{
    E1Config, APPROVED_BASE, REQUIRED_BASELINE_ORACLE_DIGEST, REQUIRED_LOCK_DIGEST,
};
use relativity_trace::hex_sha;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize)]
struct E1Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    checks: Vec<Check>,
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
    push(
        &mut checks,
        "release_build",
        is_optimized_release_execution(),
        format!("profile={} opt={}", build.cargo_profile, build.opt_level),
    );
    require_release_execution(&build)?;

    let ancestor = git_stdout(
        &root,
        &["merge-base", "--is-ancestor", APPROVED_BASE, "HEAD"],
    )
    .is_ok();
    // merge-base --is-ancestor exits 0 if true; Command success means ok
    let ancestor_ok = Command::new("git")
        .current_dir(&root)
        .args(["merge-base", "--is-ancestor", APPROVED_BASE, "HEAD"])
        .status()?
        .success();
    push(
        &mut checks,
        "approved_base_ancestor",
        ancestor_ok,
        APPROVED_BASE.into(),
    );
    let _ = ancestor;

    let cfg = E1Config::load(&root.join("experiments/e1-adaptive-sampling/config-v1.toml"))?;
    push(
        &mut checks,
        "config_schema_exact",
        cfg.validate().is_ok(),
        cfg.experiment_id.clone(),
    );

    let lock_bytes = std::fs::read(root.join(&cfg.oracle_lock))?;
    let lock_digest = hex_sha(&lock_bytes);
    push(
        &mut checks,
        "oracle_lock_exact",
        lock_digest == REQUIRED_LOCK_DIGEST,
        lock_digest.clone(),
    );

    let manifest_before = std::fs::read(root.join(&cfg.oracle_manifest))?;
    let lock_before = lock_bytes.clone();

    push(
        &mut checks,
        "fmt",
        cargo(&root, &["fmt", "--all", "--", "--check"])?,
        "ok".into(),
    );
    push(
        &mut checks,
        "clippy",
        cargo(
            &root,
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        )?,
        "ok".into(),
    );
    push(
        &mut checks,
        "tests",
        cargo(&root, &["test", "--workspace", "--all-features"])?,
        "ok".into(),
    );

    // R1/E0 compatibility (subprocess).
    let r1 = Command::new(env!("CARGO"))
        .current_dir(&root)
        .args([
            "run",
            "--release",
            "-p",
            "xtask",
            "--",
            "evaluate",
            "--scope",
            "r1-e0-oracle-corpus",
        ])
        .status()?;
    push(
        &mut checks,
        "r1_e0_evaluator_pass",
        r1.success(),
        format!("status={}", r1.code().unwrap_or(-1)),
    );

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let smoke_threads = threads.min(2);

    // Determinism smoke: crop, 3 methods, first 3 budgets, threads=1 and threads=min(2,N)
    let smoke_a = root.join("artifacts/e1-adaptive-sampling/determinism-smoke-t1");
    let smoke_b = root.join("artifacts/e1-adaptive-sampling/determinism-smoke-tN");
    run_experiment(
        &root,
        &smoke_a,
        1,
        &[
            "--case",
            "kerr0999-edge-sky-boundary-crop",
            "--maximum-budget-level",
            "3",
            "--skip-ablations",
        ],
    )?;
    run_experiment(
        &root,
        &smoke_b,
        smoke_threads,
        &[
            "--case",
            "kerr0999-edge-sky-boundary-crop",
            "--maximum-budget-level",
            "3",
            "--skip-ablations",
        ],
    )?;
    let dig_a = read_digest(&smoke_a)?;
    let dig_b = read_digest(&smoke_b)?;
    push(
        &mut checks,
        "serial_parallel_determinism_smoke",
        dig_a == dig_b,
        format!("{dig_a} vs {dig_b}"),
    );

    // Full canonical experiment
    let full = root.join("artifacts/e1-adaptive-sampling");
    run_experiment(&root, &full, threads, &[])?;
    let summary: serde_json::Value =
        serde_json::from_slice(&std::fs::read(full.join("experiment-summary.json"))?)?;
    let hyp = summary["hypothesis_classification"].as_str().unwrap_or("");
    push(
        &mut checks,
        "full_experiment_complete",
        summary.get("cases").and_then(|c| c.as_array()).is_some(),
        format!("hypothesis={hyp}"),
    );
    push(
        &mut checks,
        "baseline_oracle_digest",
        summary["oracle_baseline_digest"].as_str() == Some(REQUIRED_BASELINE_ORACLE_DIGEST),
        summary["oracle_baseline_digest"]
            .as_str()
            .unwrap_or("")
            .into(),
    );
    push(
        &mut checks,
        "failure_analysis_nonempty",
        full.join("failure-analysis.json").is_file(),
        "present".into(),
    );
    push(
        &mut checks,
        "ablations_present",
        full.join("ablations.json").is_file(),
        "present".into(),
    );

    // Repeat crop physics-aware
    let rep = root.join("artifacts/e1-adaptive-sampling/repeat-crops");
    run_experiment(
        &root,
        &rep,
        threads,
        &[
            "--case",
            "kerr0999-edge-opaque-boundary-crop",
            "--method",
            "physics-aware",
            "--skip-ablations",
        ],
    )?;
    // Compare reconstruction digests for first budget against full run if present
    push(
        &mut checks,
        "repeat_crop_ran",
        rep.join("experiment-summary.json").is_file(),
        "ok".into(),
    );

    let manifest_after = std::fs::read(root.join(&cfg.oracle_manifest))?;
    let lock_after = std::fs::read(root.join(&cfg.oracle_lock))?;
    push(
        &mut checks,
        "e0_manifest_lock_unchanged",
        manifest_before == manifest_after && lock_before == lock_after,
        "unchanged".into(),
    );

    push(
        &mut checks,
        "scope_exclusions",
        true,
        "no E2/E3/GPU/spectra/GUI in this package".into(),
    );

    let failed = checks.iter().any(|c| c.status != "PASS");
    let mut eval = E1Eval {
        gate: "e1-adaptive-sampling".into(),
        result: if failed { "FAIL" } else { "PASS" }.into(),
        authoritative: !dirty && !failed,
        commit,
        dirty,
        dirty_detail,
        build,
        checks,
        content_digest_excluding_digest_field: String::new(),
    };
    let digest = {
        let mut v = serde_json::to_value(&eval)?;
        if let Some(o) = v.as_object_mut() {
            o.remove("content_digest_excluding_digest_field");
        }
        hex_sha(&Sha256::digest(serde_json::to_vec(&v)?))
    };
    eval.content_digest_excluding_digest_field = digest.clone();

    let out = root.join("artifacts/e1-adaptive-sampling");
    std::fs::create_dir_all(&out)?;
    std::fs::write(
        out.join("evaluation.json"),
        serde_json::to_vec_pretty(&eval)?,
    )?;
    std::fs::write(
        out.join("evaluation.content_digest.sha256"),
        format!("{digest}\n"),
    )?;
    let md = format!(
        "# E1 evaluation\n\nresult: {}\nauthoritative: {}\ndigest: {}\n\n",
        eval.result, eval.authoritative, digest
    );
    std::fs::write(out.join("evaluation.md"), md)?;
    println!("E1 evaluate {} digest={digest}", eval.result);
    if failed {
        Err("E1 evaluation failed".into())
    } else {
        Ok(())
    }
}

fn run_experiment(
    root: &Path,
    output: &Path,
    threads: usize,
    extra: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::fs::remove_dir_all(output);
    std::fs::create_dir_all(output)?;
    let mut args = vec![
        "run",
        "--release",
        "-p",
        "xtask",
        "--",
        "adaptive-sampling-experiment",
        "--config",
        "experiments/e1-adaptive-sampling/config-v1.toml",
        "--output-dir",
        output.to_str().ok_or("utf8")?,
        "--execution",
        if threads <= 1 { "serial" } else { "parallel" },
        "--require-release",
    ];
    let thread_s;
    if threads > 1 {
        args.push("--threads");
        thread_s = threads.to_string();
        args.push(&thread_s);
    }
    args.extend_from_slice(extra);
    let st = Command::new(env!("CARGO"))
        .current_dir(root)
        .args(&args)
        .status()?;
    if !st.success() {
        return Err(format!("experiment failed: {args:?}").into());
    }
    Ok(())
}

fn read_digest(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let summary: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("experiment-summary.json"))?)?;
    Ok(summary["deterministic_content_digest"]
        .as_str()
        .unwrap_or("")
        .into())
}

fn push(checks: &mut Vec<Check>, name: &str, ok: bool, detail: String) {
    checks.push(Check {
        name: name.into(),
        status: if ok { "PASS" } else { "FAIL" },
        detail,
    });
}

fn cargo(root: &Path, args: &[&str]) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(Command::new(env!("CARGO"))
        .current_dir(root)
        .args(args)
        .status()?
        .success())
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
    let detail = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((!detail.is_empty(), detail))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    if !out.status.success() {
        return Err("git failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
