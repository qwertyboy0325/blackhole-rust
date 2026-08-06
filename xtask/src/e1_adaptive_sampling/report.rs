//! E1 experiment reports, Pareto, hypothesis classification.

use crate::e1_adaptive_sampling::metrics::SampleParityReport;
use crate::e1_adaptive_sampling::quadtree::PixelRect;
use crate::e1_adaptive_sampling::score::FeatureVector;
use crate::oracle_benchmark::RgbComparisonMetrics;
use relativity_oracle::OracleComparisonMetrics;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEvent {
    pub step: u64,
    pub requested_target: u64,
    pub actual_unique_rays: u64,
    pub overshoot: u64,
    pub selected: Option<PixelRect>,
    pub score: Option<f64>,
    pub features: Option<FeatureVector>,
    pub newly_traced: Vec<u64>,
    pub leaf_count: u64,
    pub max_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurvePoint {
    pub budget_id: String,
    pub leaf_size: u32,
    pub unique_traced_rays: u64,
    pub ray_fraction: f64,
    pub total_rhs_evaluations: u64,
    pub mean_rhs_per_ray: f64,
    pub maximum_rhs: u64,
    pub scientific: OracleComparisonMetrics,
    pub rgb: RgbComparisonMetrics,
    pub sample_parity: SampleParityReport,
    pub schedule: Vec<ScheduleEvent>,
    pub wall_clock_seconds: f64,
    pub reconstruction_wall_clock_seconds: f64,
    pub metric_wall_clock_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedComparison {
    pub candidate_method: String,
    pub baseline_method: String,
    pub candidate_rays: u64,
    pub matched_baseline_rays: u64,
    pub ray_count_difference: i64,
    pub outcome_rate_dominance: String,
    pub rgb_mse_dominance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodCurve {
    pub method_id: String,
    pub points: Vec<CurvePoint>,
    pub matched: Vec<MatchedComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E1CaseReport {
    pub case_id: String,
    pub is_crop: bool,
    pub methods: Vec<MethodCurve>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E1ExperimentReport {
    pub schema_version: u32,
    pub experiment_id: String,
    pub base_commit: String,
    pub evaluated_commit: String,
    pub dirty: bool,
    pub oracle_lock_digest: String,
    pub oracle_baseline_digest: String,
    pub configuration_digest: String,
    pub evidence_class: String,
    pub cases: Vec<E1CaseReport>,
    pub ablations: Vec<E1CaseReport>,
    pub hypothesis_classification: String,
    pub recommendation: String,
    pub deterministic_content_digest: String,
    pub total_wall_clock_seconds: f64,
    pub oracle_reference_wall_clock_seconds: f64,
    pub canonical: bool,
    pub filters: String,
}

pub fn write_experiment_reports(
    out: &Path,
    report: &E1ExperimentReport,
) -> Result<(), Box<dyn Error>> {
    std::fs::write(
        out.join("experiment-summary.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let md = format!(
        "# E1 experiment summary\n\n\
         - experiment: {}\n\
         - hypothesis: {}\n\
         - recommendation: {}\n\
         - lock digest: {}\n\
         - config digest: {}\n\
         - content digest: {}\n\
         - canonical: {}\n",
        report.experiment_id,
        report.hypothesis_classification,
        report.recommendation,
        report.oracle_lock_digest,
        report.configuration_digest,
        report.deterministic_content_digest,
        report.canonical
    );
    std::fs::write(out.join("experiment-summary.md"), md)?;

    let mut curves = Vec::new();
    let mut csv =
        String::from("case,method,budget,rays,outcome_rate,rgb_mse,angular_rmse,log2_iobs_rmse\n");
    for case in &report.cases {
        for method in &case.methods {
            for p in &method.points {
                let ang = p
                    .scientific
                    .celestial_angular_error_radians
                    .as_ref()
                    .map(|m| m.rmse);
                let iobs = p.scientific.log2_observed_error.as_ref().map(|m| m.rmse);
                curves.push(serde_json::json!({
                    "case": case.case_id,
                    "method": method.method_id,
                    "budget": p.budget_id,
                    "rays": p.unique_traced_rays,
                    "outcome_rate": p.scientific.outcome_disagreement_rate,
                    "rgb_mse": p.rgb.channel_mse,
                    "angular_rmse": ang,
                    "log2_iobs_rmse": iobs,
                }));
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{:?},{:?}\n",
                    case.case_id,
                    method.method_id,
                    p.budget_id,
                    p.unique_traced_rays,
                    p.scientific.outcome_disagreement_rate,
                    p.rgb.channel_mse,
                    ang,
                    iobs
                ));
            }
        }
    }
    std::fs::write(out.join("curves.json"), serde_json::to_vec_pretty(&curves)?)?;
    std::fs::write(out.join("curves.csv"), csv)?;

    let pareto = compute_pareto(&report.cases);
    std::fs::write(out.join("pareto.json"), serde_json::to_vec_pretty(&pareto)?)?;
    std::fs::write(
        out.join("ablations.json"),
        serde_json::to_vec_pretty(&report.ablations)?,
    )?;
    let failures = failure_analysis(report);
    std::fs::write(
        out.join("failure-analysis.json"),
        serde_json::to_vec_pretty(&failures)?,
    )?;
    Ok(())
}

fn compute_pareto(cases: &[E1CaseReport]) -> serde_json::Value {
    let mut out = Vec::new();
    for case in cases {
        for method in &case.methods {
            let frontier = pareto_frontier(&method.points);
            out.push(serde_json::json!({
                "case": case.case_id,
                "method": method.method_id,
                "frontier_budgets": frontier,
            }));
        }
    }
    serde_json::json!(out)
}

fn pareto_frontier(points: &[CurvePoint]) -> Vec<String> {
    let mut keep = Vec::new();
    for (i, a) in points.iter().enumerate() {
        let dominated = points.iter().enumerate().any(|(j, b)| {
            if i == j {
                return false;
            }
            let le_rays = b.unique_traced_rays <= a.unique_traced_rays;
            let le_out =
                b.scientific.outcome_disagreement_rate <= a.scientific.outcome_disagreement_rate;
            let le_mse = b.rgb.channel_mse <= a.rgb.channel_mse;
            let strict = b.unique_traced_rays < a.unique_traced_rays
                || b.scientific.outcome_disagreement_rate < a.scientific.outcome_disagreement_rate
                || b.rgb.channel_mse < a.rgb.channel_mse;
            le_rays && le_out && le_mse && strict
        });
        if !dominated {
            keep.push(a.budget_id.clone());
        }
    }
    keep
}

fn failure_analysis(report: &E1ExperimentReport) -> serde_json::Value {
    let mut worst = Vec::new();
    for case in &report.cases {
        for method in &case.methods {
            for p in &method.points {
                if p.budget_id.ends_with("-1") || p.rgb.exact_match {
                    continue;
                }
                if let Some(ang) = &p.scientific.celestial_angular_error_radians {
                    if ang.maximum_absolute_error > 0.0 {
                        worst.push(serde_json::json!({
                            "case": case.case_id,
                            "method": method.method_id,
                            "budget": p.budget_id,
                            "metric": "celestial_angular",
                            "max_error": ang.maximum_absolute_error,
                            "max_index": ang.maximum_error_index,
                            "rays": p.unique_traced_rays,
                        }));
                    }
                }
                if p.scientific.outcome_disagreement_count > 0 {
                    worst.push(serde_json::json!({
                        "case": case.case_id,
                        "method": method.method_id,
                        "budget": p.budget_id,
                        "metric": "outcome_disagreement",
                        "count": p.scientific.outcome_disagreement_count,
                        "rays": p.unique_traced_rays,
                    }));
                }
            }
        }
    }
    if worst.is_empty() {
        worst.push(serde_json::json!({
            "note": "no non-exact intermediate failures recorded (or only exact finals)"
        }));
    }
    serde_json::json!({ "worst_points": worst })
}

/// Corpus-bounded hypothesis classification (does not affect PASS).
pub fn classify_hypothesis(cases: &[E1CaseReport]) -> String {
    let crops = [
        "kerr0999-edge-opaque-boundary-crop",
        "kerr0999-edge-sky-boundary-crop",
    ];
    let mut crop_ok = 0;
    for crop in crops {
        if case_physics_pareto_beats_baselines(cases, crop, 2) {
            crop_ok += 1;
        }
    }
    let sources = [
        "kerr0999-edge-opaque",
        "kerr0999-edge-sky",
        "kerr0999-midinc-opaque",
        "kerr0999-midinc-sky",
        "kerr050-edge-sky",
        "schwarzschild-edge-sky",
    ];
    let mut source_ok = 0;
    for s in sources {
        if case_physics_pareto_beats_baselines(cases, s, 2) {
            source_ok += 1;
        }
    }
    if crop_ok == 2 && source_ok >= 3 {
        "SUPPORTED_ON_E0_CORPUS".into()
    } else if crop_ok + source_ok > 0 {
        "MIXED_ON_E0_CORPUS".into()
    } else {
        "NOT_SUPPORTED_ON_E0_CORPUS".into()
    }
}

fn case_physics_pareto_beats_baselines(
    cases: &[E1CaseReport],
    case_id: &str,
    min_points: usize,
) -> bool {
    let Some(case) = cases.iter().find(|c| c.case_id == case_id) else {
        return false;
    };
    let physics = case
        .methods
        .iter()
        .find(|m| m.method_id.contains("physics-aware"));
    let uniform = case
        .methods
        .iter()
        .find(|m| m.method_id.contains("uniform"));
    let intensity = case
        .methods
        .iter()
        .find(|m| m.method_id.contains("intensity"));
    let (Some(physics), Some(uniform), Some(intensity)) = (physics, uniform, intensity) else {
        return false;
    };
    let mut wins = 0;
    for p in &physics.points {
        if p.rgb.exact_match {
            continue; // skip full-ray
        }
        let u = matched_point(&uniform.points, p.unique_traced_rays);
        let i = matched_point(&intensity.points, p.unique_traced_rays);
        if beats(p, u) && beats(p, i) {
            wins += 1;
        }
    }
    wins >= min_points
}

fn matched_point(points: &[CurvePoint], rays: u64) -> Option<&CurvePoint> {
    points
        .iter()
        .filter(|p| p.unique_traced_rays <= rays)
        .max_by_key(|p| p.unique_traced_rays)
}

fn beats(cand: &CurvePoint, base: Option<&CurvePoint>) -> bool {
    let Some(base) = base else {
        return false;
    };
    let out_ok =
        cand.scientific.outcome_disagreement_rate <= base.scientific.outcome_disagreement_rate;
    let mse_better = cand.rgb.channel_mse < base.rgb.channel_mse;
    out_ok && mse_better
}
