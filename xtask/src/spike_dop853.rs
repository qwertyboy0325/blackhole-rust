//! Gate 1B0 DOP853 spike runner dispatch.

use gate_1b0_contract::{CandidateReport, CANDIDATE_IVP, CANDIDATE_ODE_SOLVERS};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

pub fn run(
    candidate: &str,
    commit: &str,
    toolchain: &str,
    target: &str,
) -> Result<CandidateReport, Box<dyn std::error::Error>> {
    let tree = cargo_tree(candidate)?;
    let report = match candidate {
        CANDIDATE_ODE_SOLVERS => {
            gate_1b0_ode_solvers::run_candidate_report(commit, toolchain, target, &tree)
        }
        CANDIDATE_IVP => gate_1b0_ivp::run_candidate_report(commit, toolchain, target, &tree),
        other => return Err(format!("unknown candidate {other}; use ode-solvers or ivp").into()),
    };
    Ok(report)
}

pub fn write_report(
    report: &CandidateReport,
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out_dir)?;
    let path = out_dir.join(format!("{}.json", report.candidate));
    std::fs::write(&path, serde_json::to_string_pretty(report)?)?;
    println!("Wrote {}", path.display());
    Ok(())
}

fn cargo_tree(candidate: &str) -> Result<String, Box<dyn std::error::Error>> {
    let pkg = match candidate {
        CANDIDATE_ODE_SOLVERS => "gate-1b0-ode-solvers",
        CANDIDATE_IVP => "gate-1b0-ivp",
        other => return Err(format!("unknown candidate {other}").into()),
    };
    let root = workspace_root()?;
    let out = Command::new("cargo")
        .current_dir(&root)
        .args(["tree", "-p", pkg, "--format", "{p} {v}"])
        .output()?;
    Ok(String::from_utf8(out.stdout)?)
}

fn workspace_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    Ok(dir)
}

pub fn subprocess_digest(
    candidate: &str,
    runs: u32,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let mut digests = Vec::new();
    for _ in 0..runs {
        let out = Command::new("cargo")
            .current_dir(&root)
            .args([
                "run",
                "-p",
                "xtask",
                "--",
                "spike-dop853",
                "--candidate",
                candidate,
            ])
            .output()?;
        if !out.status.success() {
            return Err(format!(
                "subprocess spike failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )
            .into());
        }
        let path = root.join(format!("artifacts/gate-1b0/{candidate}.json"));
        let bytes = std::fs::read(&path)?;
        digests.push(hex::encode(Sha256::digest(&bytes)));
    }
    Ok(digests)
}
