//! Gate 1A evaluator: schema, fmt/clippy/tests, corpus, diagnostics, reports.

use crate::preset::load_preset;
use relativity_core::{
    evaluate_kerr_schild, identity_residual, initialize_rectilinear_ray,
    inverse_metric_spatial_derivatives, stratified_corpus, zamo_observer, CameraParams, KerrParams,
    PositionBl, SensorCoord, CORPUS_SEED,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize)]
struct Gate1aReport {
    gate: &'static str,
    result: &'static str,
    commit: String,
    dirty: bool,
    toolchain: String,
    target: String,
    features: String,
    corpus_seed: u64,
    preset_path: String,
    preset_sha256: String,
    checks: Vec<Check>,
    worst: WorstResiduals,
    dependency_versions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Debug, Serialize, Default)]
struct WorstResiduals {
    metric_identity: f64,
    metric_identity_at: [f64; 3],
    derivative_abs: f64,
    derivative_at: [f64; 3],
    tetrad_orthonormality: f64,
    nullness: f64,
}

pub fn evaluate(preset_path: &str, scope: &str) -> Result<(), Box<dyn std::error::Error>> {
    if scope != "gate-1a" {
        return Err(
            format!("unsupported scope {scope}; Gate 1A evaluator accepts gate-1a only").into(),
        );
    }
    let root = workspace_root()?;
    let preset_full = if Path::new(preset_path).is_absolute() {
        PathBuf::from(preset_path)
    } else {
        root.join(preset_path)
    };
    let preset_bytes = std::fs::read(&preset_full)?;
    let preset_sha = hex::encode(Sha256::digest(&preset_bytes));
    let preset = load_preset(&preset_full)?;

    let mut checks = Vec::new();
    checks.push(Check {
        name: "preset_schema".into(),
        status: "PASS",
        detail: format!(
            "loaded {} schema_version={}",
            preset.name, preset.schema_version
        ),
    });

    let dirty =
        !git_ok(&root, ["diff", "--quiet"]) || !git_ok(&root, ["diff", "--cached", "--quiet"]);
    let commit = git_stdout(&root, ["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());
    let toolchain = Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let target = std::env::var("TARGET").unwrap_or_else(|_| default_target());

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

    let mut worst = WorstResiduals::default();
    let mut corpus_ok = true;
    let mut corpus_detail = String::new();
    for pt in stratified_corpus() {
        let Ok(params) = pt.params() else {
            continue;
        };
        let Ok(geo) = evaluate_kerr_schild(&params, &pt.pos) else {
            continue;
        };
        let id = identity_residual(&geo.metric, &geo.inverse_metric);
        if id > worst.metric_identity {
            worst.metric_identity = id;
            worst.metric_identity_at = [pt.pos.x, pt.pos.y, pt.pos.z];
        }
        if id > 1e-9 {
            corpus_ok = false;
            corpus_detail = format!("metric identity {id} too large");
        }
        if let Ok(an) = inverse_metric_spatial_derivatives(&params, &pt.pos) {
            if let Ok(diff) = crate::inspect::fd_max_public(&params, &pt.pos, &an) {
                if diff > worst.derivative_abs {
                    worst.derivative_abs = diff;
                    worst.derivative_at = [pt.pos.x, pt.pos.y, pt.pos.z];
                }
                if diff > 5e-3 {
                    corpus_ok = false;
                    corpus_detail = format!("derivative abs {diff} exceeds oracle bound");
                }
            }
        }
    }
    checks.push(Check {
        name: "metric_derivative_corpus".into(),
        status: if corpus_ok { "PASS" } else { "FAIL" },
        detail: if corpus_ok {
            format!(
                "seed={CORPUS_SEED} worst_id={:.3e} worst_d={:.3e}",
                worst.metric_identity, worst.derivative_abs
            )
        } else {
            corpus_detail
        },
    });

    let mass = preset.spacetime.mass;
    let spin = preset.spacetime.spin_a_over_m * mass;
    let params = KerrParams::new(mass, spin)?;
    let bl = PositionBl::new(
        0.0,
        preset.observer.boyer_lindquist_r,
        preset.observer.boyer_lindquist_theta_degrees.to_radians(),
        preset.observer.boyer_lindquist_phi_degrees.to_radians(),
    );
    let obs = zamo_observer(&params, &bl)?;
    let g = evaluate_kerr_schild(&params, &obs.event)?.metric;
    let mut ortho = 0.0_f64;
    for a in 0..4 {
        for b in 0..4 {
            let target = if a == b {
                if a == 0 {
                    -1.0
                } else {
                    1.0
                }
            } else {
                0.0
            };
            ortho =
                ortho.max((g.contract(&obs.tetrad.legs[a], &obs.tetrad.legs[b]) - target).abs());
        }
    }
    worst.tetrad_orthonormality = ortho;
    let cam = CameraParams {
        horizontal_fov: preset.camera.horizontal_field_of_view_degrees.to_radians(),
        roll: preset.camera.roll_degrees.to_radians(),
    };
    let ray = initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 })?;
    worst.nullness = ray.chart_null_residual.abs().max(ray.hamiltonian.h.abs());
    let ray_ok = ortho < 1e-10 && worst.nullness < 1e-10 && ray.past_time_component_local < 0.0;
    checks.push(Check {
        name: "baseline_observer_ray".into(),
        status: if ray_ok { "PASS" } else { "FAIL" },
        detail: format!(
            "ortho={ortho:.3e} null={:.3e} past_k0={}",
            worst.nullness, ray.past_time_component_local
        ),
    });

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let result = if all_pass { "PASS" } else { "FAIL" };
    let report = Gate1aReport {
        gate: "gate-1a",
        result,
        commit: commit.trim().to_string(),
        dirty,
        toolchain,
        target,
        features: "default".into(),
        corpus_seed: CORPUS_SEED,
        preset_path: preset_path.to_string(),
        preset_sha256: preset_sha,
        checks,
        worst,
        dependency_versions: vec![
            format!("relativity-core {}", env!("CARGO_PKG_VERSION")),
            "thiserror 2".into(),
            "clap 4".into(),
            "serde/serde_json 1".into(),
            "toml 0.8".into(),
            "sha2 0.10".into(),
        ],
    };

    let out_dir = root.join("artifacts/gate-1a");
    std::fs::create_dir_all(&out_dir)?;
    let json_path = out_dir.join("evaluation.json");
    let md_path = out_dir.join("evaluation.md");
    std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)?;
    std::fs::write(&md_path, render_markdown(&report))?;

    println!("Gate 1A: {result}");
    println!("JSON: {}", json_path.display());
    println!("Markdown: {}", md_path.display());
    if !all_pass {
        std::process::exit(1);
    }
    Ok(())
}

fn render_markdown(r: &Gate1aReport) -> String {
    let mut s = String::new();
    s.push_str("# Gate 1A evaluation\n\n");
    s.push_str(&format!("**Result:** {}\n\n", r.result));
    s.push_str(&format!("- commit: `{}`\n", r.commit));
    s.push_str(&format!("- dirty: {}\n", r.dirty));
    s.push_str(&format!("- toolchain: {}\n", r.toolchain));
    s.push_str(&format!("- target: {}\n", r.target));
    s.push_str(&format!("- corpus seed: {}\n", r.corpus_seed));
    s.push_str(&format!(
        "- preset: {} (`{}`)\n\n",
        r.preset_path, r.preset_sha256
    ));
    s.push_str("## Checks\n\n");
    for c in &r.checks {
        s.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    s.push_str("\n## Worst residuals\n\n");
    s.push_str(&format!(
        "- metric identity: {:.6e} at {:?}\n",
        r.worst.metric_identity, r.worst.metric_identity_at
    ));
    s.push_str(&format!(
        "- derivative abs: {:.6e} at {:?}\n",
        r.worst.derivative_abs, r.worst.derivative_at
    ));
    s.push_str(&format!(
        "- tetrad orthonormality: {:.6e}\n",
        r.worst.tetrad_orthonormality
    ));
    s.push_str(&format!("- nullness: {:.6e}\n", r.worst.nullness));
    s
}

fn run_check(
    checks: &mut Vec<Check>,
    name: &str,
    cmd: &mut Command,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = cmd.output()?;
    let status = if output.status.success() {
        "PASS"
    } else {
        "FAIL"
    };
    let detail = if output.status.success() {
        "ok".into()
    } else {
        format!(
            "exit {:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    checks.push(Check {
        name: name.into(),
        status,
        detail,
    });
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    Ok(dir)
}

fn git_ok(root: &Path, args: impl IntoIterator<Item = &'static str>) -> bool {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn git_stdout(
    root: &Path,
    args: impl IntoIterator<Item = &'static str>,
) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    Ok(String::from_utf8(out.stdout)?)
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
