//! Gate 1B0 evaluator.

use crate::spike_dop853;
use gate_1b0_contract::{
    json_digest, validate_candidate_report, ComparisonReport, RequirementComparisonRow,
    SupportLevel, ALL_EXPERIMENT_IDS, CANDIDATE_IVP, CANDIDATE_ODE_SOLVERS, DECISION_REQUIREMENTS,
};
use std::path::Path;
use std::process::Command;

#[derive(serde::Serialize)]
struct Gate1b0Report {
    gate: &'static str,
    result: &'static str,
    authoritative: bool,
    commit: String,
    dirty: bool,
    toolchain: String,
    target: String,
    candidates: Vec<String>,
    ode_solvers_digest: String,
    ivp_digest: String,
    comparison_digest: String,
    subprocess_digests: SubprocessDigests,
    checks: Vec<Check>,
}

#[derive(serde::Serialize)]
struct SubprocessDigests {
    ode_solvers: Vec<String>,
    ivp: Vec<String>,
    deterministic: bool,
}

#[derive(serde::Serialize)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

pub fn evaluate() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let (dirty, dirty_detail) = porcelain_dirty(&root)?;
    let commit = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());
    let toolchain = Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let target = std::env::var("TARGET").unwrap_or_else(|_| default_target());

    let mut checks = Vec::new();
    if dirty {
        checks.push(Check {
            name: "worktree_clean".into(),
            status: "FAIL",
            detail: format!("non-authoritative dirty worktree: {dirty_detail}"),
        });
    } else {
        checks.push(Check {
            name: "worktree_clean".into(),
            status: "PASS",
            detail: "clean".into(),
        });
    }

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

    let ode = spike_dop853::run(CANDIDATE_ODE_SOLVERS, commit.trim(), &toolchain, &target)?;
    let ivp = spike_dop853::run(CANDIDATE_IVP, commit.trim(), &toolchain, &target)?;

    let out_dir = root.join("artifacts/gate-1b0");
    spike_dop853::write_report(&ode, &out_dir)?;
    spike_dop853::write_report(&ivp, &out_dir)?;

    let comparison = build_comparison(commit.trim(), &toolchain, &target, &ode, &ivp);
    let comparison_path = out_dir.join("comparison.json");
    std::fs::write(&comparison_path, serde_json::to_string_pretty(&comparison)?)?;

    let skips_ok = ode.unexplained_skips == 0
        && ivp.unexplained_skips == 0
        && ode.experiments_run == ode.experiments_expected
        && ivp.experiments_run == ivp.experiments_expected;
    checks.push(Check {
        name: "experiment_matrix_complete".into(),
        status: if skips_ok { "PASS" } else { "FAIL" },
        detail: format!(
            "ode {}/{} skips={} ivp {}/{} skips={}",
            ode.experiments_run,
            ode.experiments_expected,
            ode.unexplained_skips,
            ivp.experiments_run,
            ivp.experiments_expected,
            ivp.unexplained_skips
        ),
    });

    let subprocess_ode = spike_dop853::subprocess_digest(CANDIDATE_ODE_SOLVERS, 5)?;
    let subprocess_ivp = spike_dop853::subprocess_digest(CANDIDATE_IVP, 5)?;
    let subprocess_det = subprocess_ode.windows(2).all(|w| w[0] == w[1])
        && subprocess_ivp.windows(2).all(|w| w[0] == w[1]);

    let ode_validation = validate_candidate_report(&ode);
    let ivp_validation = validate_candidate_report(&ivp);
    let validation_ok = ode_validation.is_ok() && ivp_validation.is_ok();
    checks.push(Check {
        name: "candidate_validation".into(),
        status: if validation_ok { "PASS" } else { "FAIL" },
        detail: format_candidate_validation_detail(&ode_validation, &ivp_validation),
    });

    let in_process_det = [(&ode, "ode_solvers"), (&ivp, "ivp")]
        .iter()
        .all(|(report, _)| all_experiments_in_process_deterministic(report));
    checks.push(Check {
        name: "in_process_determinism".into(),
        status: if in_process_det { "PASS" } else { "FAIL" },
        detail: "experiments A-G x5 in-process per candidate".into(),
    });
    checks.push(Check {
        name: "subprocess_determinism".into(),
        status: if subprocess_det { "PASS" } else { "FAIL" },
        detail: format!("x5 spike-dop853 subprocess; ode={subprocess_det} ivp={subprocess_det}"),
    });

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let authoritative = !dirty && all_pass;
    let result = if authoritative { "PASS" } else { "FAIL" };

    let report = Gate1b0Report {
        gate: "gate-1b0",
        result,
        authoritative,
        commit: commit.trim().to_string(),
        dirty,
        toolchain: toolchain.clone(),
        target: target.clone(),
        candidates: vec![CANDIDATE_ODE_SOLVERS.into(), CANDIDATE_IVP.into()],
        ode_solvers_digest: ode.report_digest.clone(),
        ivp_digest: ivp.report_digest.clone(),
        comparison_digest: comparison.comparison_digest.clone(),
        subprocess_digests: SubprocessDigests {
            ode_solvers: subprocess_ode,
            ivp: subprocess_ivp,
            deterministic: subprocess_det,
        },
        checks,
    };

    std::fs::write(
        out_dir.join("evaluation.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    std::fs::write(
        out_dir.join("evaluation.md"),
        render_md(&report, &comparison),
    )?;

    println!("Gate 1B0: {result} (authoritative={authoritative})");
    if result != "PASS" {
        std::process::exit(1);
    }
    Ok(())
}

fn build_comparison(
    commit: &str,
    toolchain: &str,
    target: &str,
    ode: &gate_1b0_contract::CandidateReport,
    ivp: &gate_1b0_contract::CandidateReport,
) -> ComparisonReport {
    let mut rows = Vec::new();
    for req in DECISION_REQUIREMENTS {
        let ode_level = ode
            .decision_matrix
            .iter()
            .find(|r| r.requirement == *req)
            .map(|r| r.level)
            .unwrap_or(SupportLevel::Unverified);
        let ivp_level = ivp
            .decision_matrix
            .iter()
            .find(|r| r.requirement == *req)
            .map(|r| r.level)
            .unwrap_or(SupportLevel::Unverified);
        rows.push(RequirementComparisonRow {
            requirement: (*req).into(),
            ode_solvers: ode_level,
            ivp: ivp_level,
            notes: String::new(),
        });
    }
    let mut comparison = ComparisonReport {
        schema_version: gate_1b0_contract::SPIKE_VERSION.into(),
        commit: commit.into(),
        toolchain: toolchain.into(),
        target: target.into(),
        candidates: vec![CANDIDATE_ODE_SOLVERS.into(), CANDIDATE_IVP.into()],
        comparison_digest: String::new(),
        requirement_comparison: rows,
        adr_recommendation: adr_recommendation(ode, ivp),
    };
    comparison.comparison_digest =
        json_digest(&comparison).unwrap_or_else(|_| "digest_error".into());
    comparison
}

fn adr_recommendation(
    ode: &gate_1b0_contract::CandidateReport,
    ivp: &gate_1b0_contract::CandidateReport,
) -> String {
    let ode_dense = level_of(ode, "accepted_step_dense_interpolation");
    let ivp_dense = level_of(ivp, "accepted_step_dense_interpolation");
    let ivp_tol = level_of(ivp, "vector_tolerance_direct");
    let _ode_tol = level_of(ode, "vector_tolerance_direct");
    if matches!(ivp_dense, SupportLevel::Supported) && matches!(ivp_tol, SupportLevel::Supported) {
        "Proposed: Accept ivp pending owner review — demonstrates SolOut StepInterpolant and vector tolerances.".into()
    } else if matches!(ode_dense, SupportLevel::Supported) {
        "Proposed: Accept ode_solvers pending owner review.".into()
    } else {
        "Proposed: ADR 0005 remains Proposed; neither candidate fully satisfies accepted-step dense interpolation + vector tolerance without adapter gaps.".into()
    }
}

fn all_experiments_in_process_deterministic(report: &gate_1b0_contract::CandidateReport) -> bool {
    ALL_EXPERIMENT_IDS.iter().all(|&id| {
        report
            .experiments
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.determinism.as_ref())
            .is_some_and(|d| d.deterministic && d.in_process_runs >= 5 && d.signatures.len() >= 5)
    })
}

fn format_candidate_validation_detail(
    ode: &Result<(), Vec<gate_1b0_contract::ValidationIssue>>,
    ivp: &Result<(), Vec<gate_1b0_contract::ValidationIssue>>,
) -> String {
    fn issues_text(
        label: &str,
        result: &Result<(), Vec<gate_1b0_contract::ValidationIssue>>,
    ) -> String {
        match result {
            Ok(()) => format!("{label}: ok"),
            Err(issues) => {
                let details: Vec<String> = issues
                    .iter()
                    .map(|i| format!("{}: {}", i.code, i.detail))
                    .collect();
                format!("{label}: {}", details.join("; "))
            }
        }
    }
    format!(
        "{} | {}",
        issues_text("ode-solvers", ode),
        issues_text("ivp", ivp)
    )
}

fn level_of(report: &gate_1b0_contract::CandidateReport, req: &str) -> SupportLevel {
    report
        .decision_matrix
        .iter()
        .find(|r| r.requirement == req)
        .map(|r| r.level)
        .unwrap_or(SupportLevel::Unverified)
}

fn render_md(r: &Gate1b0Report, cmp: &ComparisonReport) -> String {
    let mut s = String::from("# Gate 1B0 evaluation\n\n");
    s.push_str(&format!(
        "**Result:** {} (authoritative={})\n\n",
        r.result, r.authoritative
    ));
    s.push_str(&format!("- commit: `{}`\n", r.commit));
    s.push_str(&format!("- ode digest: `{}`\n", r.ode_solvers_digest));
    s.push_str(&format!("- ivp digest: `{}`\n", r.ivp_digest));
    s.push_str(&format!("- ADR note: {}\n\n", cmp.adr_recommendation));
    s.push_str("## Checks\n\n");
    for c in &r.checks {
        s.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    s.push_str("\n## Requirement comparison\n\n");
    for row in &cmp.requirement_comparison {
        s.push_str(&format!(
            "- {}: ode={:?} ivp={:?}\n",
            row.requirement, row.ode_solvers, row.ivp
        ));
    }
    s
}

fn run_check(
    checks: &mut Vec<Check>,
    name: &str,
    cmd: &mut Command,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = cmd.output()?;
    checks.push(Check {
        name: name.into(),
        status: if output.status.success() {
            "PASS"
        } else {
            "FAIL"
        },
        detail: if output.status.success() {
            "ok".into()
        } else {
            format!("exit {:?}", output.status.code())
        },
    });
    Ok(())
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()?;
    let text = String::from_utf8(out.stdout)?;
    Ok((
        !text.trim().is_empty(),
        text.lines().take(8).collect::<Vec<_>>().join("; "),
    ))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    Ok(String::from_utf8(out.stdout)?)
}

fn workspace_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    Ok(dir)
}

fn default_target() -> String {
    Command::new("rustc")
        .args(["-vV"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("host: ").map(str::to_string))
        })
        .unwrap_or_else(|| "unknown".into())
}
