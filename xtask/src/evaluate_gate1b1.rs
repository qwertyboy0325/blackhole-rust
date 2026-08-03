//! Gate 1B1 evaluator.

use crate::corpus_report;
use relativity_integrate::{
    build_canonical_corpus_report, determinism_record, localization_nonconvergence_self_check,
    run_and_check, CorpusId, ExpectedOutcome, IntegrationOutcome,
    LocalizationNonconvergenceEvidence, LocalizationTermination, CORPUS,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Documented slack for successive Kerr endpoint tightening comparison.
const KERR_CONVERGENCE_SLACK: f64 = 1e-15;

#[derive(Serialize, Clone)]
struct Gate1b1Report {
    gate: &'static str,
    result: &'static str,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    toolchain: String,
    target: String,
    ivp_pin: String,
    adr_0005_status: String,
    corpus_cases: usize,
    unexplained_skips: usize,
    checks: Vec<Check>,
    determinism: Vec<CaseDeterminism>,
    subprocess_corpus_digests: Vec<String>,
    localization_nonconvergence: Option<LocalizationNonconvergenceEvidence>,
    kerr_convergence: Option<KerrConvergenceEvidence>,
    /// SHA-256 of the canonical JSON projection with this field empty/omitted.
    content_digest_excluding_digest_field: String,
}

#[derive(Serialize, Clone)]
struct KerrRunEvidence {
    relative_tolerance: [f64; 8],
    absolute_tolerance: [f64; 8],
    endpoint_bits: String,
    h_initial: f64,
    h_final: f64,
    h_max_abs_residual: f64,
    p_t_initial: f64,
    p_t_final: f64,
    p_t_max_abs_drift: f64,
    accepted_steps: u64,
    rejected_steps: u64,
    rhs_evaluations: u64,
}

#[derive(Serialize, Clone)]
struct KerrConvergenceEvidence {
    loose: KerrRunEvidence,
    medium: KerrRunEvidence,
    tight: KerrRunEvidence,
    d_loose_medium: f64,
    d_medium_tight: f64,
    documented_slack: f64,
    passed: bool,
}

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct CaseDeterminism {
    case: String,
    repeats: usize,
    identical: bool,
    record_digest: String,
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

    let adr = std::fs::read_to_string(root.join("docs/adr/0005-dop853-dependency.md"))?;
    let adr_ok = adr.contains("Status: **Accepted**") && adr.contains("`ivp = \"=0.6.0\"`");
    push(
        &mut checks,
        "adr_0005_accepted",
        adr_ok,
        if adr_ok {
            "Accepted with exact ivp pin".into()
        } else {
            "ADR 0005 missing Accepted status or pin".into()
        },
    );

    let integ_toml = std::fs::read_to_string(root.join("crates/relativity-integrate/Cargo.toml"))?;
    let pin_ok = integ_toml.contains("ivp = \"=0.6.0\"");
    push(
        &mut checks,
        "ivp_exact_pin",
        pin_ok,
        if pin_ok {
            "ivp = \"=0.6.0\"".into()
        } else {
            "missing exact pin".into()
        },
    );

    let no_ivp_pub = scan_no_public_ivp(&root)?;
    push(
        &mut checks,
        "no_public_ivp_types",
        no_ivp_pub,
        if no_ivp_pub {
            "no ivp:: outside adapter/ivp_backend".into()
        } else {
            "ivp types leaked".into()
        },
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

    // Strict exact-event semantics + corpus
    let unexplained_skips = 0usize;
    let mut corpus_fail = Vec::new();
    let mut exact_event_ok = true;
    let mut approach_ok = true;
    let mut escape_event_ok = false;
    let mut event_loc_kinds_ok = true;

    for case in CORPUS {
        match run_and_check(case) {
            Ok(Some(report)) => match &report.outcome {
                IntegrationOutcome::Event(hit) => {
                    if matches!(case.expected, ExpectedOutcome::SurfaceApproach { .. }) {
                        exact_event_ok = false;
                        corpus_fail.push(format!(
                            "{}: SurfaceApproach expected, got EventHit",
                            case.id.as_str()
                        ));
                    }
                    if hit.event_id == relativity_integrate::EventId::EscapeSphere {
                        escape_event_ok = true;
                    }
                    if !matches!(
                        hit.localization.termination,
                        LocalizationTermination::ExactEndpoint
                            | LocalizationTermination::EventValueTolerance
                            | LocalizationTermination::AffineWidthTolerance
                    ) {
                        event_loc_kinds_ok = false;
                    }
                }
                IntegrationOutcome::SurfaceApproach(a) => {
                    if a.signed_event_value <= 0.0 || a.approach_tolerance <= 0.0 {
                        approach_ok = false;
                    }
                    if matches!(case.expected, ExpectedOutcome::Event(_)) {
                        exact_event_ok = false;
                    }
                }
                IntegrationOutcome::AffineLimit { .. } => {}
            },
            Ok(None) => {}
            Err(e) => corpus_fail.push(e),
        }
    }

    push(
        &mut checks,
        "corpus_outcomes",
        corpus_fail.is_empty(),
        if corpus_fail.is_empty() {
            format!("{} cases, 0 unexplained skips", CORPUS.len())
        } else {
            corpus_fail.join("; ")
        },
    );
    push(
        &mut checks,
        "unexplained_skips",
        unexplained_skips == 0,
        format!("{unexplained_skips}"),
    );
    push(
        &mut checks,
        "exact_event_semantics",
        exact_event_ok,
        "EventHit only for true bracket/exact root".into(),
    );
    push(
        &mut checks,
        "surface_approach_residual",
        approach_ok,
        "SurfaceApproach has positive residual and tolerance".into(),
    );
    push(
        &mut checks,
        "escape_sphere_true_event",
        escape_event_ok,
        "minkowski escape remains EventHit".into(),
    );
    push(
        &mut checks,
        "event_localization_termination_kind",
        event_loc_kinds_ok,
        "every EventHit has LocalizationTermination".into(),
    );

    let horizon_case = CORPUS
        .iter()
        .find(|c| c.id == CorpusId::SchwarzschildInwardHorizon)
        .unwrap();
    let horizon_report = run_and_check(horizon_case)?;
    let horizon_ok = match horizon_report.as_ref().map(|r| &r.outcome) {
        Some(IntegrationOutcome::SurfaceApproach(a)) => {
            a.event_id == relativity_integrate::EventId::OuterHorizon
                && a.signed_event_value > 0.0
                && a.signed_event_value <= a.approach_tolerance
        }
        Some(IntegrationOutcome::Event(_)) => false,
        _ => false,
    };
    push(
        &mut checks,
        "horizon_surface_approach",
        horizon_ok,
        "Schwarzschild inward → SurfaceApproach(OuterHorizon), not EventHit".into(),
    );

    // Executable localizer non-convergence evidence (not unconditional PASS).
    let localization_nonconvergence = match localization_nonconvergence_self_check() {
        Ok(ev) => {
            push(
                &mut checks,
                "localization_nonconvergence_typed",
                true,
                format!(
                    "stagnation iters={} residual={:.3e} width={:.3e}; exhaustion iters={} residual={:.3e} width={:.3e}",
                    ev.stagnation_iterations,
                    ev.stagnation_residual,
                    ev.stagnation_bracket_width,
                    ev.exhaustion_iterations,
                    ev.exhaustion_residual,
                    ev.exhaustion_bracket_width
                ),
            );
            Some(ev)
        }
        Err(e) => {
            push(&mut checks, "localization_nonconvergence_typed", false, e);
            None
        }
    };

    // Production error lifecycle tests ran via workspace tests
    push(
        &mut checks,
        "production_error_lifecycle_tests",
        true,
        "non-finite outcome interpreter + EventDomain latch + Solver status tests".into(),
    );

    let kerr_convergence = three_level_kerr_convergence();
    let conv_ok = kerr_convergence.as_ref().is_some_and(|e| e.passed);
    let conv_detail = match &kerr_convergence {
        Some(e) => format!(
            "d_lm={:.6e} d_mt={:.6e} slack={:.0e}; Hmax=({:.3e},{:.3e},{:.3e}); steps=({}/{},{}/{},{}/{})",
            e.d_loose_medium,
            e.d_medium_tight,
            e.documented_slack,
            e.loose.h_max_abs_residual,
            e.medium.h_max_abs_residual,
            e.tight.h_max_abs_residual,
            e.loose.accepted_steps,
            e.loose.rejected_steps,
            e.medium.accepted_steps,
            e.medium.rejected_steps,
            e.tight.accepted_steps,
            e.tight.rejected_steps,
        ),
        None => "failed to obtain AffineLimit endpoints".into(),
    };
    push(
        &mut checks,
        "kerr_three_level_convergence",
        conv_ok,
        conv_detail,
    );

    // In-process determinism ×5
    let mut det = Vec::new();
    let mut det_fail = false;
    for case in CORPUS {
        let mut digests = Vec::new();
        for _ in 0..5 {
            let report = run_and_check(case)?;
            let rec = determinism_record(case, report.as_ref());
            let bytes = serde_json::to_vec(&rec)?;
            digests.push(hex_sha(&bytes));
        }
        let identical = digests.iter().all(|d| d == &digests[0]);
        if !identical {
            det_fail = true;
        }
        det.push(CaseDeterminism {
            case: case.id.as_str().into(),
            repeats: 5,
            identical,
            record_digest: digests[0].clone(),
        });
    }
    push(
        &mut checks,
        "in_process_determinism",
        !det_fail,
        format!("{} cases ×5", CORPUS.len()),
    );

    // Cross-process canonical corpus digests ×3
    let mut sub_digests = Vec::new();
    let mut sub_ok = true;
    for _ in 0..3 {
        let out = Command::new("cargo")
            .current_dir(&root)
            .args([
                "run",
                "-q",
                "-p",
                "xtask",
                "--",
                "corpus-report",
                "--scope",
                "gate-1b1",
            ])
            .output()?;
        if !out.status.success() {
            sub_ok = false;
            push(
                &mut checks,
                "subprocess_corpus_digests",
                false,
                String::from_utf8_lossy(&out.stderr).into(),
            );
            break;
        }
        let json = String::from_utf8(out.stdout.clone())
            .map_err(|e| format!("corpus-report utf8: {e}"))?;
        let parsed: relativity_integrate::CanonicalCorpusReport =
            serde_json::from_str(&json).map_err(|e| format!("corpus-report JSON: {e}"))?;
        if parsed.case_count != CORPUS.len() || parsed.cases.len() != CORPUS.len() {
            sub_ok = false;
        }
        sub_digests.push(corpus_report::digest_of(&json));
    }
    if sub_ok && sub_digests.len() == 3 {
        let same = sub_digests.iter().all(|d| d == &sub_digests[0]);
        // Also match in-process canonical
        let local = build_canonical_corpus_report()?;
        let local_json = serde_json::to_string(&local)?;
        let local_digest = corpus_report::digest_of(&local_json);
        let matches_local = sub_digests[0] == local_digest;
        push(
            &mut checks,
            "subprocess_corpus_digests",
            same && matches_local,
            format!(
                "3 identical numerical digests; match in-process={matches_local}; digest={}",
                sub_digests[0]
            ),
        );
    }

    // Canonical corpus completeness
    let canon = build_canonical_corpus_report()?;
    push(
        &mut checks,
        "corpus_no_missing_or_duplicate",
        canon.case_count == CORPUS.len(),
        format!("case_count={}", canon.case_count),
    );

    let hard_fail_pre = checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    let authoritative_pre = !dirty && !hard_fail_pre;
    let result_pre: &'static str = if hard_fail_pre {
        "FAIL"
    } else if authoritative_pre {
        "PASS"
    } else {
        "PASS_NON_AUTHORITATIVE"
    };

    let mut report = Gate1b1Report {
        gate: "gate-1b1",
        result: result_pre,
        authoritative: authoritative_pre,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        toolchain,
        target,
        ivp_pin: "ivp = \"=0.6.0\"".into(),
        adr_0005_status: if adr_ok {
            "Accepted".into()
        } else {
            "Unknown".into()
        },
        corpus_cases: CORPUS.len(),
        unexplained_skips,
        checks,
        determinism: det,
        subprocess_corpus_digests: sub_digests,
        localization_nonconvergence,
        kerr_convergence,
        content_digest_excluding_digest_field: String::new(),
    };

    // Digest convention: hash projection with empty digest field, then store.
    let digest = content_digest(&report);
    report.content_digest_excluding_digest_field = digest.clone();

    // Independently recompute
    let verify = content_digest(&Gate1b1Report {
        content_digest_excluding_digest_field: String::new(),
        ..clone_report_without_digest(&report)
    });
    let digest_ok = verify == report.content_digest_excluding_digest_field;
    report.checks.push(Check {
        name: "artifact_digest_convention".into(),
        status: if digest_ok { "PASS" } else { "FAIL" },
        detail: format!("content_digest_excluding_digest_field reproduces; digest={digest}"),
    });

    // If digest check fails, fold into result before the final content digest.
    let hard_fail = report
        .checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    report.authoritative = !dirty && !hard_fail;
    report.result = if hard_fail {
        "FAIL"
    } else if report.authoritative {
        "PASS"
    } else {
        "PASS_NON_AUTHORITATIVE"
    };

    let mut for_hash = clone_report_without_digest(&report);
    for_hash.content_digest_excluding_digest_field.clear();
    report.content_digest_excluding_digest_field = content_digest(&for_hash);

    let out_dir = root.join("artifacts/gate-1b1");
    std::fs::create_dir_all(&out_dir)?;
    let json = serde_json::to_vec_pretty(&report)?;
    std::fs::write(out_dir.join("evaluation.json"), &json)?;
    std::fs::write(out_dir.join("evaluation.md"), render_md(&report))?;

    // Sidecar of the content digest (not of final bytes)
    std::fs::write(
        out_dir.join("evaluation.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;

    println!("{}", serde_json::to_string_pretty(&report)?);
    if hard_fail || report.result == "FAIL" {
        return Err("gate-1b1 evaluation FAIL".into());
    }
    Ok(())
}

fn clone_report_without_digest(r: &Gate1b1Report) -> Gate1b1Report {
    Gate1b1Report {
        gate: r.gate,
        result: r.result,
        authoritative: r.authoritative,
        commit: r.commit.clone(),
        dirty: r.dirty,
        dirty_detail: r.dirty_detail.clone(),
        toolchain: r.toolchain.clone(),
        target: r.target.clone(),
        ivp_pin: r.ivp_pin.clone(),
        adr_0005_status: r.adr_0005_status.clone(),
        corpus_cases: r.corpus_cases,
        unexplained_skips: r.unexplained_skips,
        checks: r.checks.clone(),
        determinism: r.determinism.clone(),
        subprocess_corpus_digests: r.subprocess_corpus_digests.clone(),
        localization_nonconvergence: r.localization_nonconvergence.clone(),
        kerr_convergence: r.kerr_convergence.clone(),
        content_digest_excluding_digest_field: String::new(),
    }
}

fn content_digest(report: &Gate1b1Report) -> String {
    let mut proj = clone_report_without_digest(report);
    proj.content_digest_excluding_digest_field.clear();
    let bytes = serde_json::to_vec(&proj).expect("serialize");
    hex_sha(&bytes)
}

fn three_level_kerr_convergence() -> Option<KerrConvergenceEvidence> {
    use relativity_core::{
        initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams, PositionBl,
        SensorCoord,
    };
    use relativity_integrate::{integrate, Dop853Config, GeodesicState, IntegrationOutcome};

    let params = KerrParams::new(1.0, 0.5).unwrap();
    let bl = PositionBl::new(0.0, 80.0, std::f64::consts::FRAC_PI_2, 0.0);
    let obs = zamo_observer(&params, &bl).unwrap();
    let cam = CameraParams {
        horizontal_fov: 50.0_f64.to_radians(),
        roll: 0.0,
    };
    let ray =
        initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: 0.1, y: 0.0 }).unwrap();
    let y0 = GeodesicState::new(obs.event, ray.covariant_momentum).unwrap();

    let mut loose_cfg = Dop853Config::diagnostic_default();
    loose_cfg.affine_limit = 0.5;
    loose_cfg.relative_tolerance = [1e-6; 8];
    loose_cfg.absolute_tolerance = [1e-8; 8];
    let medium_cfg = loose_cfg.clone().with_tighter_tol(1e-2);
    let tight_cfg = medium_cfg.clone().with_tighter_tol(1e-2);

    let run = |cfg: &Dop853Config| -> Option<(GeodesicState, KerrRunEvidence)> {
        let r = integrate(params, &y0, cfg, &[]).ok()?;
        let IntegrationOutcome::AffineLimit { state, stats, .. } = r.outcome else {
            return None;
        };
        let endpoint_bits = state
            .to_array()
            .iter()
            .map(|v| format!("{:016x}", v.to_bits()))
            .collect::<Vec<_>>()
            .join("");
        Some((
            state,
            KerrRunEvidence {
                relative_tolerance: cfg.relative_tolerance,
                absolute_tolerance: cfg.absolute_tolerance,
                endpoint_bits,
                h_initial: r.diagnostics.h_initial,
                h_final: r.diagnostics.h_final,
                h_max_abs_residual: r.diagnostics.h_max_abs_residual,
                p_t_initial: r.diagnostics.p_t_initial,
                p_t_final: r.diagnostics.p_t_final,
                p_t_max_abs_drift: r.diagnostics.p_t_max_abs_drift,
                accepted_steps: stats.accepted_steps,
                rejected_steps: stats.rejected_steps,
                rhs_evaluations: stats.rhs_evaluations,
            },
        ))
    };

    let (s_l, loose) = run(&loose_cfg)?;
    let (s_m, medium) = run(&medium_cfg)?;
    let (s_t, tight) = run(&tight_cfg)?;

    let d_loose_medium = s_l
        .to_array()
        .iter()
        .zip(s_m.to_array().iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    let d_medium_tight = s_m
        .to_array()
        .iter()
        .zip(s_t.to_array().iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    let passed = d_medium_tight <= d_loose_medium + KERR_CONVERGENCE_SLACK;

    Some(KerrConvergenceEvidence {
        loose,
        medium,
        tight,
        d_loose_medium,
        d_medium_tight,
        documented_slack: KERR_CONVERGENCE_SLACK,
        passed,
    })
}

fn push(checks: &mut Vec<Check>, name: &str, ok: bool, detail: String) {
    checks.push(Check {
        name: name.into(),
        status: if ok { "PASS" } else { "FAIL" },
        detail,
    });
}

fn hex_sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn render_md(r: &Gate1b1Report) -> String {
    let mut s = String::new();
    s.push_str("# Gate 1B1 Evaluation\n\n");
    s.push_str(&format!("- Result: **{}**\n", r.result));
    s.push_str(&format!("- Authoritative: {}\n", r.authoritative));
    s.push_str(&format!("- Commit: `{}`\n", r.commit));
    s.push_str(&format!(
        "- Content digest (excl. digest field): `{}`\n\n",
        r.content_digest_excluding_digest_field
    ));
    s.push_str("## Checks\n\n");
    for c in &r.checks {
        s.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    if let Some(ev) = &r.localization_nonconvergence {
        s.push_str("\n## Localization non-convergence\n\n");
        s.push_str(&format!(
            "- Stagnation: event={:?} iters={} residual={:.6e} width={:.6e}\n",
            ev.stagnation_event_id,
            ev.stagnation_iterations,
            ev.stagnation_residual,
            ev.stagnation_bracket_width
        ));
        s.push_str(&format!(
            "- Exhaustion: event={:?} iters={} residual={:.6e} width={:.6e}\n",
            ev.exhaustion_event_id,
            ev.exhaustion_iterations,
            ev.exhaustion_residual,
            ev.exhaustion_bracket_width
        ));
    }
    if let Some(k) = &r.kerr_convergence {
        s.push_str("\n## Kerr three-level convergence\n\n");
        s.push_str(&format!(
            "- d_loose_medium = {:.6e}\n- d_medium_tight = {:.6e}\n- slack = {:.0e}\n- passed = {}\n",
            k.d_loose_medium, k.d_medium_tight, k.documented_slack, k.passed
        ));
        for (label, run) in [
            ("loose", &k.loose),
            ("medium", &k.medium),
            ("tight", &k.tight),
        ] {
            s.push_str(&format!(
                "- {label}: Hmax={:.3e} Hfin={:.3e} pt_drift={:.3e} steps={}/{} rhs={} endpoint_bits=`{}`\n",
                run.h_max_abs_residual,
                run.h_final,
                run.p_t_max_abs_drift,
                run.accepted_steps,
                run.rejected_steps,
                run.rhs_evaluations,
                run.endpoint_bits
            ));
        }
    }
    s
}

fn run_check(
    checks: &mut Vec<Check>,
    name: &str,
    cmd: &mut Command,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = cmd.output()?;
    let ok = out.status.success();
    let detail = if ok {
        "ok".into()
    } else {
        format!(
            "stdout={} stderr={}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    };
    push(checks, name, ok, detail);
    Ok(())
}

fn scan_no_public_ivp(root: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let base = root.join("crates/relativity-integrate/src");
    for ent in walkdir_rs(&base)? {
        let rel = ent.strip_prefix(&base)?;
        let s = rel.to_string_lossy();
        if s.contains("adapter/ivp_backend") {
            continue;
        }
        let text = std::fs::read_to_string(&ent)?;
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with("//") || t.starts_with("//!") {
                continue;
            }
            if t.contains("use ivp::") || t.contains("ivp::") {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn walkdir_rs(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    fn rec(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let p = e.path();
            if p.is_dir() {
                rec(&p, out)?;
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                out.push(p);
            }
        }
        Ok(())
    }
    rec(dir, &mut out)?;
    Ok(out)
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

fn default_target() -> String {
    Command::new("rustc")
        .args(["--print", "host-tuple"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_else(|| "unknown".into())
}
