//! E1 adaptive sampling evaluator (PASS independent of hypothesis).

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::e1_adaptive_sampling::config::{
    E1Config, APPROVED_BASE, REQUIRED_BASELINE_ORACLE_DIGEST, REQUIRED_LOCK_DIGEST,
};
use crate::e1_adaptive_sampling::metrics::final_scientific_exact;
use crate::e1_adaptive_sampling::report::{
    case_optional_metric_consistency, classify_hypothesis, E1ExperimentReport,
};
use relativity_trace::hex_sha;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const SOURCE_CASES: [&str; 6] = [
    "kerr0999-edge-opaque",
    "kerr0999-edge-sky",
    "kerr0999-midinc-opaque",
    "kerr0999-midinc-sky",
    "kerr050-edge-sky",
    "schwarzschild-edge-sky",
];
const CROP_CASES: [&str; 2] = [
    "kerr0999-edge-opaque-boundary-crop",
    "kerr0999-edge-sky-boundary-crop",
];
const PRIMARY_METHOD_DIRS: [&str; 3] = ["uniform", "intensity-only", "physics-aware"];
const ABLATION_IDS: [&str; 5] = [
    "physics-no-outcome",
    "physics-no-lens-map",
    "physics-no-g",
    "physics-no-radiance",
    "physics-no-trace-cost",
];
const ABLATION_CASES: [&str; 3] = [
    "kerr0999-edge-opaque",
    "kerr0999-edge-opaque-boundary-crop",
    "kerr0999-edge-sky-boundary-crop",
];

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
    let smoke_cmp = compare_case_method_tree(
        &smoke_a.join("cases/kerr0999-edge-sky-boundary-crop"),
        &smoke_b.join("cases/kerr0999-edge-sky-boundary-crop"),
        &PRIMARY_METHOD_DIRS,
    )?;
    let dig_a = read_digest(&smoke_a)?;
    let dig_b = read_digest(&smoke_b)?;
    push(
        &mut checks,
        "serial_parallel_determinism_smoke",
        dig_a == dig_b && smoke_cmp.is_empty(),
        if dig_a == dig_b && smoke_cmp.is_empty() {
            format!("digest={dig_a}")
        } else {
            format!("digest {dig_a} vs {dig_b}; artifact diffs={smoke_cmp:?}")
        },
    );

    // Full canonical experiment
    let full = root.join("artifacts/e1-adaptive-sampling");
    // Preserve smoke dirs under full by writing canonical into a staging then merging?
    // run_experiment wipes output dir — smokes live as siblings under e1-adaptive-sampling/
    // so wipe would delete them. Use dedicated canonical dir then copy summaries up? Spec
    // uses artifacts/e1-adaptive-sampling/. Keep smokes outside wipe by using subdir:
    let canonical = root.join("artifacts/e1-adaptive-sampling/canonical");
    run_experiment(&root, &canonical, threads, &[])?;
    // Publish canonical tree as the main artifact root contents (without deleting smokes).
    publish_canonical(&canonical, &full)?;

    let summary: E1ExperimentReport =
        serde_json::from_slice(&std::fs::read(full.join("experiment-summary.json"))?)?;
    let matrix = validate_matrix(&summary, &full)?;
    push(
        &mut checks,
        "full_matrix_8x3x5",
        matrix.ok,
        matrix.detail.clone(),
    );
    push(
        &mut checks,
        "sample_parity_zero_mismatches",
        matrix.parity_ok,
        matrix.parity_detail.clone(),
    );
    push(
        &mut checks,
        "metrics_finite",
        matrix.finite_ok,
        matrix.finite_detail.clone(),
    );
    push(
        &mut checks,
        "final_full_ray_exact",
        matrix.final_exact_ok,
        matrix.final_exact_detail.clone(),
    );
    push(
        &mut checks,
        "ablations_complete",
        matrix.ablations_ok,
        matrix.ablations_detail.clone(),
    );
    push(
        &mut checks,
        "failure_analysis_semantic",
        matrix.failure_ok,
        matrix.failure_detail.clone(),
    );
    push(
        &mut checks,
        "pareto_includes_scientific_dimensions",
        matrix.pareto_ok,
        matrix.pareto_detail.clone(),
    );
    push(
        &mut checks,
        "baseline_oracle_digest",
        summary.oracle_baseline_digest == REQUIRED_BASELINE_ORACLE_DIGEST,
        summary.oracle_baseline_digest.clone(),
    );
    let recomputed = classify_hypothesis(&summary.cases);
    let hypothesis_ok = summary.hypothesis_classification == recomputed
        && matches!(
            recomputed.as_str(),
            "SUPPORTED_ON_E0_CORPUS" | "MIXED_ON_E0_CORPUS" | "NOT_SUPPORTED_ON_E0_CORPUS"
        );
    push(
        &mut checks,
        "hypothesis_classification_recorded",
        hypothesis_ok,
        if hypothesis_ok {
            recomputed
        } else {
            format!(
                "summary={} recomputed={}",
                summary.hypothesis_classification, recomputed
            )
        },
    );
    push(
        &mut checks,
        "optional_metric_consistency",
        matrix.optional_ok,
        matrix.optional_detail.clone(),
    );

    // Repeat both boundary crops, physics-aware full ladder, compare to canonical.
    let mut repeat_ok = true;
    let mut repeat_detail = Vec::new();
    for crop in CROP_CASES {
        let rep = full.join(format!("repeat-{crop}"));
        run_experiment(
            &root,
            &rep,
            threads,
            &[
                "--case",
                crop,
                "--method",
                "physics-aware",
                "--skip-ablations",
            ],
        )?;
        let diffs = compare_case_method_tree(
            &full.join("cases").join(crop),
            &rep.join("cases").join(crop),
            &["physics-aware"],
        )?;
        if diffs.is_empty() {
            repeat_detail.push(format!("{crop}: identical"));
        } else {
            repeat_ok = false;
            repeat_detail.push(format!("{crop}: {diffs:?}"));
        }
        // Also compare deterministic digest of the single-case summary method curves
        // against the canonical case slice after stripping timings.
        let canon_case = summary
            .cases
            .iter()
            .find(|c| c.case_id == crop)
            .ok_or("missing crop in canonical summary")?;
        let rep_summary: E1ExperimentReport =
            serde_json::from_slice(&std::fs::read(rep.join("experiment-summary.json"))?)?;
        let rep_case = rep_summary
            .cases
            .iter()
            .find(|c| c.case_id == crop)
            .ok_or("missing crop in repeat summary")?;
        let d1 = case_method_digest(canon_case, "physics-aware")?;
        let d2 = case_method_digest(rep_case, "physics-aware")?;
        if d1 != d2 {
            repeat_ok = false;
            repeat_detail.push(format!("{crop}: curve digest mismatch"));
        }
    }
    push(
        &mut checks,
        "repeat_crop_determinism",
        repeat_ok,
        repeat_detail.join("; "),
    );

    let manifest_after = std::fs::read(root.join(&cfg.oracle_manifest))?;
    let lock_after = std::fs::read(root.join(&cfg.oracle_lock))?;
    push(
        &mut checks,
        "e0_manifest_lock_unchanged",
        manifest_before == manifest_after && lock_before == lock_after,
        "unchanged".into(),
    );

    let exclusions = scope_exclusion_scan(&root)?;
    push(
        &mut checks,
        "scope_exclusions",
        exclusions.ok,
        exclusions.detail,
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

    std::fs::create_dir_all(&full)?;
    std::fs::write(
        full.join("evaluation.json"),
        serde_json::to_vec_pretty(&eval)?,
    )?;
    std::fs::write(
        full.join("evaluation.content_digest.sha256"),
        format!("{digest}\n"),
    )?;
    let md = format!(
        "# E1 evaluation\n\nresult: {}\nauthoritative: {}\nhypothesis: {}\ndigest: {}\n\n",
        eval.result, eval.authoritative, summary.hypothesis_classification, digest
    );
    std::fs::write(full.join("evaluation.md"), md)?;
    println!("E1 evaluate {} digest={digest}", eval.result);
    if failed {
        Err("E1 evaluation failed".into())
    } else {
        Ok(())
    }
}

struct MatrixValidation {
    ok: bool,
    detail: String,
    parity_ok: bool,
    parity_detail: String,
    finite_ok: bool,
    finite_detail: String,
    final_exact_ok: bool,
    final_exact_detail: String,
    ablations_ok: bool,
    ablations_detail: String,
    failure_ok: bool,
    failure_detail: String,
    pareto_ok: bool,
    pareto_detail: String,
    optional_ok: bool,
    optional_detail: String,
}

fn validate_matrix(
    summary: &E1ExperimentReport,
    full: &Path,
) -> Result<MatrixValidation, Box<dyn std::error::Error>> {
    let mut missing = Vec::new();
    for id in SOURCE_CASES.iter().chain(CROP_CASES.iter()) {
        let case = summary.cases.iter().find(|c| c.case_id == *id);
        if case.is_none() {
            missing.push(format!("case:{id}"));
            continue;
        }
        let case = case.unwrap();
        if case.methods.len() != 3 {
            missing.push(format!("{id}:methods={}", case.methods.len()));
        }
        for m in &PRIMARY_METHOD_DIRS {
            let method = case.methods.iter().find(|x| {
                x.method_id.contains(m)
                    || x.method_id.ends_with(m)
                    || method_dir_match(&x.method_id, m)
            });
            let Some(method) = method else {
                missing.push(format!("{id}:{m}"));
                continue;
            };
            if method.points.len() != 5 {
                missing.push(format!("{id}:{m}:points={}", method.points.len()));
            }
            for p in &method.points {
                let dir = full
                    .join("cases")
                    .join(id)
                    .join(artifact_method_dir(&method.method_id))
                    .join(&p.budget_id);
                for name in [
                    "reconstruction.ppm",
                    "sample-mask.pgm",
                    "leaf-depth.pgm",
                    "outcome-disagreement.pgm",
                    "scientific-error-summary.json",
                    "schedule-summary.json",
                ] {
                    if !dir.join(name).is_file() {
                        missing.push(format!("{id}/{m}/{}/{name}", p.budget_id));
                    }
                }
            }
        }
    }
    let ok = missing.is_empty();
    let detail = if ok {
        "8 cases × 3 methods × 5 budgets".into()
    } else {
        format!("missing={}", missing.join(","))
    };

    let mut parity_bad = Vec::new();
    let mut finite_bad = Vec::new();
    let mut final_bad = Vec::new();
    let mut optional_bad = Vec::new();
    for case in &summary.cases {
        if let Err(e) = case_optional_metric_consistency(case) {
            optional_bad.push(e);
        }
        for method in &case.methods {
            for p in &method.points {
                if p.sample_parity.selected_sample_mismatch_count != 0 {
                    parity_bad.push(format!(
                        "{}/{}/{}",
                        case.case_id, method.method_id, p.budget_id
                    ));
                }
                if !metrics_finite(p) {
                    finite_bad.push(format!(
                        "{}/{}/{}",
                        case.case_id, method.method_id, p.budget_id
                    ));
                }
                let is_final = p.leaf_size == 1 || (!case.is_crop && p.leaf_size == 2);
                if is_final {
                    if let Err(detail) = final_scientific_exact(
                        case.is_crop,
                        p.unique_traced_rays,
                        &p.scientific,
                        &p.rgb,
                        &p.sample_parity,
                    ) {
                        final_bad.push(format!(
                            "{}/{}/{}: {detail}",
                            case.case_id, method.method_id, p.budget_id
                        ));
                    }
                }
            }
        }
    }

    let mut abl_missing = Vec::new();
    for case in ABLATION_CASES {
        for abl in ABLATION_IDS {
            let found = summary.ablations.iter().any(|a| {
                a.case_id == case
                    && a.methods
                        .iter()
                        .any(|m| m.method_id == abl && m.points.len() == 5)
            });
            if !found {
                // also accept filesystem under ablations/
                let dir = full.join("ablations").join(case).join(abl);
                let n = std::fs::read_dir(&dir)
                    .map(|rd| rd.filter_map(|e| e.ok()).count())
                    .unwrap_or(0);
                if n < 5 {
                    abl_missing.push(format!("{case}/{abl}"));
                }
            }
        }
    }

    let failure_raw = std::fs::read(full.join("failure-analysis.json"))?;
    let failure: serde_json::Value = serde_json::from_slice(&failure_raw)?;
    let worst = failure
        .get("worst_points")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let required_categories = [
        "unresolved_outcome_islands",
        "thin_celestial_features",
        "high_angular_error_regions",
        "radiance_failures",
        "intensity_only_beats_physics_aware",
        "trace_cost_over_focus",
        "ablation_regressions",
    ];
    let categories = failure.get("categories").and_then(|c| c.as_object());
    let categories_ok = categories.is_some_and(|cats| {
        required_categories.iter().all(|name| {
            cats.get(*name).is_some_and(|entry| {
                matches!(
                    entry.get("status").and_then(|s| s.as_str()),
                    Some("observed") | Some("not-observed")
                ) && entry.get("evidence").and_then(|e| e.as_array()).is_some()
            })
        })
    });
    let failure_ok = !worst.is_empty()
        && categories_ok
        && worst.iter().any(|w| {
            w.get("leaf_rectangle").is_some()
                || w.get("note").and_then(|n| n.as_str())
                    == Some("no non-exact intermediate worst-pixel records")
        })
        && (worst.iter().all(|w| w.get("note").is_some())
            || worst.iter().any(|w| {
                w.get("target_local").is_some()
                    && w.get("provenance_source_index").is_some()
                    && w.get("leaf_depth").is_some()
            }));

    let pareto_raw = std::fs::read(full.join("pareto.json"))?;
    let pareto: serde_json::Value = serde_json::from_slice(&pareto_raw)?;
    let pareto_ok = pareto.as_array().is_some_and(|arr| {
        !arr.is_empty()
            && arr.iter().all(|e| {
                e.get("dimensions")
                    .and_then(|d| d.as_array())
                    .is_some_and(|dims| {
                        dims.iter()
                            .any(|x| x.as_str() == Some("celestial_angular_rmse"))
                            && dims.iter().any(|x| x.as_str() == Some("log2_iobs_rmse"))
                    })
            })
    });

    Ok(MatrixValidation {
        ok,
        detail,
        parity_ok: parity_bad.is_empty(),
        parity_detail: if parity_bad.is_empty() {
            "all zero".into()
        } else {
            parity_bad.join(",")
        },
        finite_ok: finite_bad.is_empty(),
        finite_detail: if finite_bad.is_empty() {
            "all finite".into()
        } else {
            finite_bad.join(",")
        },
        final_exact_ok: final_bad.is_empty(),
        final_exact_detail: if final_bad.is_empty() {
            "all finals exact".into()
        } else {
            final_bad.join(",")
        },
        ablations_ok: abl_missing.is_empty(),
        ablations_detail: if abl_missing.is_empty() {
            "3 cases × 5 ablations × 5 budgets".into()
        } else {
            abl_missing.join(",")
        },
        failure_ok,
        failure_detail: if failure_ok {
            format!(
                "worst_points={} categories=observed/not-observed",
                worst.len()
            )
        } else {
            "missing coordinates/leaf/feature/category semantics".into()
        },
        pareto_ok,
        pareto_detail: if pareto_ok {
            "angular+iobs dimensions present".into()
        } else {
            "scientific dimensions missing".into()
        },
        optional_ok: optional_bad.is_empty(),
        optional_detail: if optional_bad.is_empty() {
            "case-level optional metrics consistent".into()
        } else {
            optional_bad.join(",")
        },
    })
}

fn method_dir_match(method_id: &str, dir: &str) -> bool {
    match dir {
        "uniform" => method_id.contains("uniform"),
        "intensity-only" => method_id.contains("intensity"),
        "physics-aware" => method_id.contains("physics-aware"),
        _ => method_id == dir,
    }
}

fn artifact_method_dir(method_id: &str) -> &'static str {
    if method_id.contains("uniform") {
        "uniform"
    } else if method_id.contains("intensity") {
        "intensity-only"
    } else {
        // physics-aware primary + ablation method IDs
        "physics-aware"
    }
}

fn metrics_finite(p: &crate::e1_adaptive_sampling::report::CurvePoint) -> bool {
    let sci = &p.scientific;
    let mut vals = vec![
        sci.outcome_disagreement_rate,
        sci.rhs_absolute_error.mae,
        sci.rhs_absolute_error.rmse,
        p.rgb.channel_mse,
        p.ray_fraction,
        p.mean_rhs_per_ray,
    ];
    for m in [
        &sci.celestial_angular_error_radians,
        &sci.celestial_wrap_u_error,
        &sci.celestial_v_error,
        &sci.log2_g_error,
        &sci.log2_emitted_error,
        &sci.log2_observed_error,
    ]
    .into_iter()
    .flatten()
    {
        vals.extend([m.mae, m.rmse, m.maximum_absolute_error]);
    }
    vals.into_iter().all(|v| v.is_finite()) && p.rgb.psnr_db.as_ref().is_none_or(|v| v.is_finite())
}

fn compare_case_method_tree(
    a_case: &Path,
    b_case: &Path,
    methods: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut diffs = Vec::new();
    for method in methods {
        let a_root = a_case.join(method);
        let b_root = b_case.join(method);
        if !a_root.is_dir() || !b_root.is_dir() {
            diffs.push(format!("missing method dir {method}"));
            continue;
        }
        let mut budgets = std::fs::read_dir(&a_root)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        budgets.sort();
        for budget in budgets {
            for name in [
                "reconstruction.ppm",
                "sample-mask.pgm",
                "leaf-depth.pgm",
                "outcome-disagreement.pgm",
                "scientific-error-summary.json",
                "schedule-summary.json",
            ] {
                let pa = a_root.join(&budget).join(name);
                let pb = b_root.join(&budget).join(name);
                if !pa.is_file() || !pb.is_file() {
                    diffs.push(format!("{method}/{budget}/{name}: missing"));
                    continue;
                }
                let ba = std::fs::read(&pa)?;
                let bb = std::fs::read(&pb)?;
                if ba != bb {
                    diffs.push(format!(
                        "{method}/{budget}/{name}: digest {} vs {}",
                        hex_sha(&ba),
                        hex_sha(&bb)
                    ));
                }
            }
        }
    }
    Ok(diffs)
}

fn case_method_digest(
    case: &crate::e1_adaptive_sampling::report::E1CaseReport,
    method_needle: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let method = case
        .methods
        .iter()
        .find(|m| m.method_id.contains(method_needle))
        .ok_or("method missing")?;
    // Hash only per-budget scientific schedule/metrics. Exclude `matched`, which
    // depends on sibling baseline methods present in the same experiment run.
    let mut points = Vec::new();
    for p in &method.points {
        let mut v = serde_json::to_value(p)?;
        if let Some(o) = v.as_object_mut() {
            o.remove("wall_clock_seconds");
            o.remove("reconstruction_wall_clock_seconds");
            o.remove("metric_wall_clock_seconds");
        }
        points.push(v);
    }
    let payload = serde_json::json!({
        "method_id": method.method_id,
        "points": points,
    });
    Ok(hex_sha(&Sha256::digest(serde_json::to_vec(&payload)?)))
}

fn publish_canonical(canonical: &Path, full: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(full)?;
    for name in [
        "experiment-summary.json",
        "experiment-summary.md",
        "curves.json",
        "curves.csv",
        "pareto.json",
        "ablations.json",
        "failure-analysis.json",
    ] {
        let src = canonical.join(name);
        if src.is_file() {
            std::fs::copy(&src, full.join(name))?;
        }
    }
    for dir in ["cases", "ablations", "reference"] {
        let src = canonical.join(dir);
        let dst = full.join(dir);
        if src.is_dir() {
            let _ = std::fs::remove_dir_all(&dst);
            copy_dir_recursive(&src, &dst)?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

struct ExclusionScan {
    ok: bool,
    detail: String,
}

fn scope_exclusion_scan(root: &Path) -> Result<ExclusionScan, Box<dyn std::error::Error>> {
    // Dependency / import level only. Comment mentions of excluded future work are allowed.
    let mut hits = Vec::new();
    for toml_name in ["Cargo.toml", "xtask/Cargo.toml"] {
        let text = std::fs::read_to_string(root.join(toml_name)).unwrap_or_default();
        for dep in ["wgpu", "egui", "eframe", "winit", "openexr", "exr"] {
            // crude Cargo dep line match
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with(&format!("{dep} "))
                    || t.starts_with(&format!("{dep}="))
                    || t.starts_with(&format!("{dep} ="))
                {
                    hits.push(format!("{toml_name}:{dep}"));
                }
            }
        }
    }
    // Match real import lines only (not string-literal needles in this evaluator).
    let import_prefixes = [
        "use wgpu",
        "use egui",
        "use eframe",
        "pub use wgpu",
        "pub use egui",
        "pub use eframe",
        "extern crate wgpu",
        "extern crate egui",
        "extern crate eframe",
    ];
    for dir in ["crates", "xtask/src"] {
        for path in walkdir_rs(&root.join(dir))? {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            for (lineno, raw) in text.lines().enumerate() {
                let code = raw.split("//").next().unwrap_or("").trim();
                for pat in import_prefixes {
                    if !code.starts_with(pat) {
                        continue;
                    }
                    let boundary_ok = code
                        .as_bytes()
                        .get(pat.len())
                        .is_none_or(|b| matches!(b, b':' | b';' | b' ' | b'\t'));
                    if boundary_ok {
                        hits.push(format!(
                            "{}:{}:{pat}",
                            path.strip_prefix(root)?.display(),
                            lineno + 1
                        ));
                    }
                }
            }
        }
    }
    Ok(ExclusionScan {
        ok: hits.is_empty(),
        detail: if hits.is_empty() {
            "no forbidden GPU/GUI/OpenEXR dependencies or imports".into()
        } else {
            hits.join(",")
        },
    })
}

fn walkdir_rs(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out)?;
            } else {
                out.push(path);
            }
        }
        Ok(())
    }
    walk(root, &mut out)?;
    Ok(out)
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
