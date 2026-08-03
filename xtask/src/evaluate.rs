//! Gate 1A evaluator: schema, fmt/clippy/tests, total corpus, diagnostics, reports.

use crate::preset::load_preset;
use relativity_core::{
    evaluate_kerr_schild, identity_residual, initialize_rectilinear_ray,
    inverse_metric_spatial_derivatives, matrix_inverse_oracle, stratified_corpus, zamo_observer,
    CameraParams, CoreError, CorpusTag, ExpectedOutcome, KerrParams, MetricTensor, PositionBl,
    SensorCoord, Vector, CORPUS_SEED,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Remediation-reviewed geometry head (PR #1 owner review); evaluator may run on a
/// later documentation-only commit.
const REVIEWED_HEAD: &str = "37d5e59afb974e0d5d36a5ee1481570b6951cf17";

#[derive(Debug, Serialize)]
struct Provenance {
    reviewed_head: String,
    evaluator_commit: String,
    commits_between_reviewed_and_evaluator: Vec<String>,
    between_commits_documentation_only: bool,
}

#[derive(Debug, Serialize)]
struct Gate1aReport {
    gate: &'static str,
    result: &'static str,
    authoritative: bool,
    provenance: Provenance,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    toolchain: String,
    target: String,
    features: String,
    corpus_seed: u64,
    preset_path: String,
    preset_sha256: String,
    corpus_coverage: CorpusCoverage,
    checks: Vec<Check>,
    worst: WorstResiduals,
    dependency_versions: Vec<String>,
}

#[derive(Debug, Serialize, Default)]
struct CorpusCoverage {
    expected_points: usize,
    evaluated_valid: usize,
    expected_failures: usize,
    unexpected_failures: usize,
    unexplained_skips: usize,
    derivative_components: usize,
    by_tag: BTreeMap<String, usize>,
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
    raw_inverse_asymmetry: f64,
    eta_ll: f64,
    g_ll: f64,
    det_plus_one: f64,
    derivative_abs: f64,
    derivative_rel_at_worst_abs: f64,
    derivative_analytic_at_worst_abs: f64,
    derivative_fd_at_worst_abs: f64,
    derivative_scale_at_worst_abs: f64,
    derivative_at: [f64; 3],
    derivative_tag: String,
    derivative_axis: usize,
    derivative_alpha: usize,
    derivative_beta: usize,
    tetrad_orthonormality: f64,
    zamo_u_phi: f64,
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

    let (dirty, dirty_detail) = porcelain_dirty(&root)?;
    let commit = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());
    let toolchain = Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let target = std::env::var("TARGET").unwrap_or_else(|_| default_target());

    // Dirty tree cannot emit authoritative PASS.
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

    let (coverage, worst_corpus, corpus_ok, corpus_detail) = run_total_corpus()?;
    checks.push(Check {
        name: "metric_derivative_corpus".into(),
        status: if corpus_ok { "PASS" } else { "FAIL" },
        detail: corpus_detail,
    });

    let mut worst = worst_corpus;
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
    worst.zamo_u_phi = obs.bl_u_phi.unwrap_or(f64::NAN).abs();
    let cam = CameraParams {
        horizontal_fov: preset.camera.horizontal_field_of_view_degrees.to_radians(),
        roll: preset.camera.roll_degrees.to_radians(),
    };
    let ray = initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 })?;
    worst.nullness = ray.chart_null_residual.abs().max(ray.hamiltonian.h.abs());
    let look_bl = relativity_core::vector_ks_to_bl(&params, &bl, &obs.tetrad.legs[3].scale(-1.0))?;
    let ray_ok = ortho < 1e-10
        && worst.nullness < 1e-10
        && ray.past_time_component_local < 0.0
        && worst.zamo_u_phi < 1e-10
        && look_bl.x < 0.0;
    checks.push(Check {
        name: "baseline_observer_ray".into(),
        status: if ray_ok { "PASS" } else { "FAIL" },
        detail: format!(
            "ortho={ortho:.3e} null={:.3e} u_phi={:.3e} look_r={:.3e} past_k0={}",
            worst.nullness, worst.zamo_u_phi, look_bl.x, ray.past_time_component_local
        ),
    });

    let all_checks_pass = checks.iter().all(|c| c.status == "PASS");
    let authoritative = !dirty && all_checks_pass;
    let result = if authoritative {
        "PASS"
    } else if all_checks_pass {
        // Should not happen: dirty forces worktree_clean FAIL.
        "FAIL"
    } else {
        "FAIL"
    };

    let provenance = build_provenance(&root, commit.trim())?;

    let report = Gate1aReport {
        gate: "gate-1a",
        result,
        authoritative,
        provenance,
        commit: commit.trim().to_string(),
        dirty,
        dirty_detail,
        toolchain,
        target,
        features: "default".into(),
        corpus_seed: CORPUS_SEED,
        preset_path: preset_path.to_string(),
        preset_sha256: preset_sha,
        corpus_coverage: coverage,
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

    println!("Gate 1A: {result} (authoritative={authoritative})");
    println!("JSON: {}", json_path.display());
    println!("Markdown: {}", md_path.display());
    if result != "PASS" {
        std::process::exit(1);
    }
    Ok(())
}

fn run_total_corpus(
) -> Result<(CorpusCoverage, WorstResiduals, bool, String), Box<dyn std::error::Error>> {
    let mut coverage = CorpusCoverage::default();
    let mut worst = WorstResiduals::default();
    let mut ok = true;
    let mut detail = String::new();
    let abs_tol = 5e-3;
    let rel_tol = 2e-3;

    let pts = stratified_corpus();
    coverage.expected_points = pts.len();
    for pt in &pts {
        *coverage.by_tag.entry(format!("{:?}", pt.tag)).or_insert(0) += 1;
    }

    for pt in pts {
        let params = match pt.params() {
            Ok(p) => p,
            Err(e) => {
                coverage.unexpected_failures += 1;
                ok = false;
                detail = format!("params failed at {:?}: {e}", pt.pos);
                continue;
            }
        };

        match pt.expected {
            ExpectedOutcome::ExpectedDomainFailure(reason) => {
                match evaluate_kerr_schild(&params, &pt.pos) {
                    Err(CoreError::ChartDomain { reason: r, .. }) if r == reason => {
                        coverage.expected_failures += 1;
                    }
                    Err(e) => {
                        coverage.unexpected_failures += 1;
                        ok = false;
                        detail = format!("unexpected domain err at {:?}: {e}", pt.pos);
                    }
                    Ok(_) => {
                        coverage.unexpected_failures += 1;
                        ok = false;
                        detail = format!("expected domain failure missing at {:?}", pt.pos);
                    }
                }
            }
            ExpectedOutcome::Valid => {
                let geo = match evaluate_kerr_schild(&params, &pt.pos) {
                    Ok(g) => g,
                    Err(e) => {
                        coverage.unexpected_failures += 1;
                        ok = false;
                        detail = format!("metric failed at {:?}: {e}", pt.pos);
                        continue;
                    }
                };
                coverage.evaluated_valid += 1;

                let id = identity_residual(&geo.metric, &geo.inverse_metric);
                if id > worst.metric_identity {
                    worst.metric_identity = id;
                    worst.metric_identity_at = [pt.pos.x, pt.pos.y, pt.pos.z];
                }
                if id > 1e-9 {
                    ok = false;
                    detail = format!("metric identity {id} too large");
                }

                let ell = Vector::from_components(geo.ell_con);
                let eta = MetricTensor::minkowski();
                let eta_ll = eta.contract(&ell, &ell).abs();
                let g_ll = geo.metric.contract(&ell, &ell).abs();
                let det_res = (geo.metric.determinant() + 1.0).abs();
                worst.eta_ll = worst.eta_ll.max(eta_ll);
                worst.g_ll = worst.g_ll.max(g_ll);
                worst.det_plus_one = worst.det_plus_one.max(det_res);
                if eta_ll > 1e-10 || g_ll > 1e-10 || det_res > 1e-8 {
                    ok = false;
                    detail = format!("KS invariants failed at {:?}", pt.pos);
                }

                match matrix_inverse_oracle(&geo.metric) {
                    Ok(oracle) => {
                        worst.raw_inverse_asymmetry =
                            worst.raw_inverse_asymmetry.max(oracle.raw_asymmetry);
                        if oracle.raw_asymmetry > 1e-9 || oracle.identity_residual > 1e-9 {
                            ok = false;
                            detail = format!("inverse oracle failed at {:?}", pt.pos);
                        }
                    }
                    Err(e) => {
                        coverage.unexpected_failures += 1;
                        ok = false;
                        detail = format!("inverse oracle err at {:?}: {e}", pt.pos);
                        continue;
                    }
                }

                let analytic = match inverse_metric_spatial_derivatives(&params, &pt.pos) {
                    Ok(a) => a,
                    Err(e) => {
                        coverage.unexpected_failures += 1;
                        ok = false;
                        detail = format!("analytic ∂ failed at {:?}: {e}", pt.pos);
                        continue;
                    }
                };

                for axis in 0..3 {
                    let fd = match crate::inspect::fd_partial_public(&params, &pt.pos, axis) {
                        Ok(f) => f,
                        Err(e) => {
                            coverage.unexpected_failures += 1;
                            ok = false;
                            detail = format!("FD axis {axis} failed at {:?}: {e}", pt.pos);
                            continue;
                        }
                    };
                    for a in 0..4 {
                        for b in 0..4 {
                            coverage.derivative_components += 1;
                            let an = analytic.spatial[axis][a][b];
                            let diff = (an - fd[a][b]).abs();
                            let scale = an.abs().max(fd[a][b].abs()).max(1e-12);
                            let rel = diff / scale;
                            if diff > worst.derivative_abs {
                                worst.derivative_abs = diff;
                                worst.derivative_rel_at_worst_abs = rel;
                                worst.derivative_analytic_at_worst_abs = an;
                                worst.derivative_fd_at_worst_abs = fd[a][b];
                                worst.derivative_scale_at_worst_abs = scale;
                                worst.derivative_at = [pt.pos.x, pt.pos.y, pt.pos.z];
                                worst.derivative_tag = format!("{:?}", pt.tag);
                                worst.derivative_axis = axis;
                                worst.derivative_alpha = a;
                                worst.derivative_beta = b;
                            }
                            if diff > abs_tol && rel > rel_tol {
                                ok = false;
                                detail = format!(
                                    "derivative mismatch abs={diff} rel={rel} tag={:?} axis={axis} αβ=({a},{b})",
                                    pt.tag
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Authoritative accounting: every point must be Valid-evaluated or expected-failure.
    let accounted = coverage.evaluated_valid + coverage.expected_failures;
    coverage.unexplained_skips = coverage
        .expected_points
        .saturating_sub(accounted + coverage.unexpected_failures);
    // If we hit unexpected failures, those points are accounted as failures not skips.
    // Unexplained skips only if loops somehow omitted points without recording.
    if coverage.unexplained_skips > 0 || coverage.unexpected_failures > 0 {
        ok = false;
        if detail.is_empty() {
            detail = format!(
                "coverage incomplete: expected={} valid={} exp_fail={} unexpected={} skips={}",
                coverage.expected_points,
                coverage.evaluated_valid,
                coverage.expected_failures,
                coverage.unexpected_failures,
                coverage.unexplained_skips
            );
        }
    }

    if ok {
        detail = format!(
            "seed={CORPUS_SEED} expected={} valid={} exp_fail={} unexpected=0 skips=0 deriv_components={} worst_id={:.3e} worst_d_abs={:.3e} worst_d_rel_at_worst_abs={:.3e} @{} axis={} αβ=({},{})",
            coverage.expected_points,
            coverage.evaluated_valid,
            coverage.expected_failures,
            coverage.derivative_components,
            worst.metric_identity,
            worst.derivative_abs,
            worst.derivative_rel_at_worst_abs,
            worst.derivative_tag,
            worst.derivative_axis,
            worst.derivative_alpha,
            worst.derivative_beta
        );
    }

    // Required: Valid points each contribute 3*16 derivative components.
    let expected_deriv = coverage.evaluated_valid * 3 * 16;
    if coverage.derivative_components != expected_deriv {
        ok = false;
        detail = format!(
            "derivative component count {} != expected {expected_deriv}",
            coverage.derivative_components
        );
    }

    let _ = CorpusTag::WeakField; // keep import meaningful for Debug tags
    Ok((coverage, worst, ok, detail))
}

fn build_provenance(
    root: &Path,
    evaluator_commit: &str,
) -> Result<Provenance, Box<dyn std::error::Error>> {
    let log_range = format!("{REVIEWED_HEAD}..HEAD");
    let between = git_stdout(root, &["log", "--oneline", log_range.as_str()])?
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let between_docs_only = if between.is_empty() {
        true
    } else {
        between_commits_touch_only_allowed_paths(root, REVIEWED_HEAD, evaluator_commit)?
    };
    Ok(Provenance {
        reviewed_head: REVIEWED_HEAD.to_string(),
        evaluator_commit: evaluator_commit.to_string(),
        commits_between_reviewed_and_evaluator: between,
        between_commits_documentation_only: between_docs_only,
    })
}

/// True when every file changed between `from..to` lies under docs/ or xtask/.
fn between_commits_touch_only_allowed_paths(
    root: &Path,
    from: &str,
    to: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let diff_range = format!("{from}..{to}");
    let out = Command::new("git")
        .current_dir(root)
        .args(["diff", "--name-only", diff_range.as_str()])
        .output()?;
    let paths = String::from_utf8(out.stdout)?;
    if paths.trim().is_empty() {
        return Ok(true);
    }
    for line in paths.lines() {
        let p = line.trim();
        if p.is_empty() {
            continue;
        }
        if p.starts_with("docs/") || p.starts_with("xtask/") {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()?;
    let text = String::from_utf8(out.stdout)?;
    let dirty = !text.trim().is_empty();
    let detail = if dirty {
        text.lines().take(12).collect::<Vec<_>>().join("; ")
    } else {
        String::new()
    };
    Ok((dirty, detail))
}

fn render_markdown(r: &Gate1aReport) -> String {
    let mut s = String::new();
    s.push_str("# Gate 1A evaluation\n\n");
    s.push_str(&format!(
        "**Result:** {} (authoritative={})\n\n",
        r.result, r.authoritative
    ));
    s.push_str(&format!("- commit: `{}`\n", r.commit));
    s.push_str(&format!("- dirty: {} {}\n", r.dirty, r.dirty_detail));
    s.push_str(&format!("- toolchain: {}\n", r.toolchain));
    s.push_str(&format!("- target: {}\n", r.target));
    s.push_str(&format!("- corpus seed: {}\n", r.corpus_seed));
    s.push_str(&format!(
        "- preset: {} (`{}`)\n\n",
        r.preset_path, r.preset_sha256
    ));
    s.push_str("## Corpus coverage\n\n");
    s.push_str(&format!(
        "- expected/evaluated_valid/expected_failures/unexpected/skips: {}/{}/{}/{}/{}\n",
        r.corpus_coverage.expected_points,
        r.corpus_coverage.evaluated_valid,
        r.corpus_coverage.expected_failures,
        r.corpus_coverage.unexpected_failures,
        r.corpus_coverage.unexplained_skips
    ));
    s.push_str(&format!(
        "- derivative components: {}\n",
        r.corpus_coverage.derivative_components
    ));
    s.push_str(&format!("- by tag: {:?}\n\n", r.corpus_coverage.by_tag));
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
        "- raw inverse asymmetry: {:.6e}\n",
        r.worst.raw_inverse_asymmetry
    ));
    s.push_str(&format!("- η(ℓ,ℓ): {:.6e}\n", r.worst.eta_ll));
    s.push_str(&format!("- g(ℓ,ℓ): {:.6e}\n", r.worst.g_ll));
    s.push_str(&format!("- |det(g)+1|: {:.6e}\n", r.worst.det_plus_one));
    s.push_str(&format!(
        "- derivative worst abs: {:.6e} at {:?} tag={} axis={} αβ=({},{}) analytic={:.6e} fd={:.6e} scale={:.6e}\n",
        r.worst.derivative_abs,
        r.worst.derivative_at,
        r.worst.derivative_tag,
        r.worst.derivative_axis,
        r.worst.derivative_alpha,
        r.worst.derivative_beta,
        r.worst.derivative_analytic_at_worst_abs,
        r.worst.derivative_fd_at_worst_abs,
        r.worst.derivative_scale_at_worst_abs
    ));
    s.push_str(&format!(
        "- derivative rel at worst abs: {:.6e}\n",
        r.worst.derivative_rel_at_worst_abs
    ));
    s.push_str(&format!(
        "- provenance: reviewed_head=`{}` evaluator=`{}` between_docs_only={}\n",
        r.provenance.reviewed_head,
        r.provenance.evaluator_commit,
        r.provenance.between_commits_documentation_only
    ));
    s.push_str(&format!(
        "- tetrad orthonormality: {:.6e}\n",
        r.worst.tetrad_orthonormality
    ));
    s.push_str(&format!("- ZAMO |u_φ|: {:.6e}\n", r.worst.zamo_u_phi));
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

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
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
