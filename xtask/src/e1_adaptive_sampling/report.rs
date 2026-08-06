//! E1 experiment reports, Pareto, hypothesis classification.

use crate::e1_adaptive_sampling::metrics::SampleParityReport;
use crate::e1_adaptive_sampling::quadtree::{PixelRect, QuadCell};
use crate::e1_adaptive_sampling::reconstruct::{
    find_leaf, AdaptiveReconstructedPixel, AdaptiveReconstruction,
};
use crate::e1_adaptive_sampling::score::FeatureVector;
use crate::oracle_benchmark::RgbComparisonMetrics;
use relativity_oracle::{OracleComparisonMetrics, OracleFrame, OraclePixel};
use relativity_trace::OutcomeClass;
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
pub struct WorstPixelRecord {
    pub metric: String,
    pub error: f64,
    pub target_local_col: u32,
    pub target_local_row: u32,
    pub target_source_col: u32,
    pub target_source_row: u32,
    pub provenance_source_index: u64,
    pub provenance_source_col: u32,
    pub provenance_source_row: u32,
    pub oracle_outcome: OutcomeClass,
    pub candidate_outcome: OutcomeClass,
    pub leaf_rectangle: PixelRect,
    pub leaf_depth: u32,
    pub last_split_rectangle: Option<PixelRect>,
    pub feature_vector_at_last_split: Option<FeatureVector>,
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
    pub worst_pixels: Vec<WorstPixelRecord>,
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
    pub angular_rmse_dominance: String,
    pub log2_iobs_rmse_dominance: String,
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

/// Case-level applicability of optional scientific Pareto dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplicableScientificDims {
    pub celestial_angular: bool,
    pub log2_iobs: bool,
}

/// Infer applicable scientific dims from observed metrics on a case (all methods).
pub fn applicable_dims_for_case(case: &E1CaseReport) -> ApplicableScientificDims {
    let mut celestial_angular = false;
    let mut log2_iobs = false;
    for method in &case.methods {
        for p in &method.points {
            if p.scientific.celestial_angular_error_radians.is_some()
                || p.scientific.celestial_pair_count > 0
            {
                celestial_angular = true;
            }
            if p.scientific.log2_observed_error.is_some() || p.scientific.disk_pair_count > 0 {
                log2_iobs = true;
            }
        }
    }
    ApplicableScientificDims {
        celestial_angular,
        log2_iobs,
    }
}

/// Validate optional-metric presence at full-coverage finals against case applicability.
pub fn case_optional_metric_consistency(case: &E1CaseReport) -> Result<(), String> {
    let dims = applicable_dims_for_case(case);
    for method in &case.methods {
        for p in &method.points {
            let is_final = p.leaf_size == 1 || (!case.is_crop && p.leaf_size == 2);
            if !is_final {
                continue;
            }
            let has_ang = p.scientific.celestial_angular_error_radians.is_some();
            let has_iobs = p.scientific.log2_observed_error.is_some();
            if dims.celestial_angular && !has_ang && p.scientific.celestial_pair_count > 0 {
                return Err(format!(
                    "{}/{}/{}: angular applicable with pairs but metric None",
                    case.case_id, method.method_id, p.budget_id
                ));
            }
            if dims.log2_iobs && !has_iobs && p.scientific.disk_pair_count > 0 {
                return Err(format!(
                    "{}/{}/{}: log2_iobs applicable with pairs but metric None",
                    case.case_id, method.method_id, p.budget_id
                ));
            }
            if !dims.celestial_angular && has_ang {
                return Err(format!(
                    "{}/{}/{}: angular not applicable but metric Some",
                    case.case_id, method.method_id, p.budget_id
                ));
            }
            if !dims.log2_iobs && has_iobs {
                return Err(format!(
                    "{}/{}/{}: log2_iobs not applicable but metric Some",
                    case.case_id, method.method_id, p.budget_id
                ));
            }
        }
    }
    Ok(())
}

/// Primary objective vector for Pareto:
/// (rays, outcome_rate, rgb_mse, angular_rmse?, log2_iobs_rmse?).
#[derive(Debug, Clone, Copy)]
struct PrimaryErrors {
    rays: u64,
    outcome_rate: f64,
    rgb_mse: f64,
    angular_rmse: Option<f64>,
    log2_iobs_rmse: Option<f64>,
}

fn primary_errors(p: &CurvePoint) -> PrimaryErrors {
    PrimaryErrors {
        rays: p.unique_traced_rays,
        outcome_rate: p.scientific.outcome_disagreement_rate,
        rgb_mse: p.rgb.channel_mse,
        angular_rmse: p
            .scientific
            .celestial_angular_error_radians
            .as_ref()
            .map(|m| m.rmse),
        log2_iobs_rmse: p.scientific.log2_observed_error.as_ref().map(|m| m.rmse),
    }
}

/// Compare optional error dims under case-level applicability.
/// Returns `None` when mixed Some/None would silently favor incomplete data.
fn optional_le_strict(bv: Option<f64>, av: Option<f64>, applicable: bool) -> Option<(bool, bool)> {
    if !applicable {
        return match (bv, av) {
            (None, None) => Some((true, false)),
            _ => None, // data bug: metric present when not applicable
        };
    }
    match (bv, av) {
        (Some(b), Some(a)) => Some((b <= a, b < a)),
        (None, None) => Some((true, false)), // jointly absent at this budget
        _ => None,                           // mixed: incomplete, not "not worse"
    }
}

/// Same-method frontier dominance: fewer-or-equal rays plus no-worse applicable errors.
pub fn dominates(b: &CurvePoint, a: &CurvePoint, dims: ApplicableScientificDims) -> bool {
    let bb = primary_errors(b);
    let aa = primary_errors(a);
    if bb.rays > aa.rays {
        return false;
    }
    let mut le_all = bb.outcome_rate <= aa.outcome_rate && bb.rgb_mse <= aa.rgb_mse;
    let mut strict =
        bb.rays < aa.rays || bb.outcome_rate < aa.outcome_rate || bb.rgb_mse < aa.rgb_mse;
    let Some((ang_le, ang_st)) =
        optional_le_strict(bb.angular_rmse, aa.angular_rmse, dims.celestial_angular)
    else {
        return false;
    };
    le_all &= ang_le;
    strict |= ang_st;
    let Some((iobs_le, iobs_st)) =
        optional_le_strict(bb.log2_iobs_rmse, aa.log2_iobs_rmse, dims.log2_iobs)
    else {
        return false;
    };
    le_all &= iobs_le;
    strict |= iobs_st;
    le_all && strict
}

fn compute_pareto(cases: &[E1CaseReport]) -> serde_json::Value {
    let mut out = Vec::new();
    for case in cases {
        let dims = applicable_dims_for_case(case);
        for method in &case.methods {
            let frontier = pareto_frontier(&method.points, dims);
            out.push(serde_json::json!({
                "case": case.case_id,
                "method": method.method_id,
                "applicable_dimensions": {
                    "celestial_angular_rmse": dims.celestial_angular,
                    "log2_iobs_rmse": dims.log2_iobs,
                },
                "dimensions": [
                    "unique_traced_rays",
                    "outcome_disagreement_rate",
                    "rgb_mse",
                    "celestial_angular_rmse",
                    "log2_iobs_rmse"
                ],
                "frontier_budgets": frontier,
            }));
        }
    }
    serde_json::json!(out)
}

fn pareto_frontier(points: &[CurvePoint], dims: ApplicableScientificDims) -> Vec<String> {
    let mut keep = Vec::new();
    for (i, a) in points.iter().enumerate() {
        let dominated = points
            .iter()
            .enumerate()
            .any(|(j, b)| i != j && dominates(b, a, dims));
        if !dominated {
            keep.push(a.budget_id.clone());
        }
    }
    keep
}

fn category_entry(observed: bool, evidence: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "status": if observed { "observed" } else { "not-observed" },
        "evidence": evidence,
    })
}

fn failure_analysis(report: &E1ExperimentReport) -> serde_json::Value {
    let mut worst = Vec::new();
    let mut categories = serde_json::Map::new();
    for case in &report.cases {
        for method in &case.methods {
            for p in &method.points {
                if p.rgb.exact_match {
                    continue;
                }
                for w in &p.worst_pixels {
                    worst.push(serde_json::json!({
                        "case": case.case_id,
                        "method": method.method_id,
                        "budget": p.budget_id,
                        "rays": p.unique_traced_rays,
                        "metric": w.metric,
                        "error": w.error,
                        "target_local": [w.target_local_col, w.target_local_row],
                        "target_source": [w.target_source_col, w.target_source_row],
                        "provenance_source_index": w.provenance_source_index,
                        "provenance_source": [w.provenance_source_col, w.provenance_source_row],
                        "oracle_outcome": format!("{:?}", w.oracle_outcome),
                        "candidate_outcome": format!("{:?}", w.candidate_outcome),
                        "leaf_rectangle": w.leaf_rectangle,
                        "leaf_depth": w.leaf_depth,
                        "last_split_rectangle": w.last_split_rectangle,
                        "feature_vector_at_last_split": w.feature_vector_at_last_split,
                    }));
                    let key = w.metric.clone();
                    let n = categories.get(&key).and_then(|v| v.as_u64()).unwrap_or(0);
                    categories.insert(key, serde_json::json!(n + 1));
                }
            }
        }
    }
    if worst.is_empty() {
        worst.push(serde_json::json!({
            "note": "no non-exact intermediate worst-pixel records"
        }));
    }

    let structured = build_failure_categories(report, &worst);
    serde_json::json!({
        "worst_points": worst,
        "metric_counts": categories,
        "categories": structured,
        "known_blind_spots": [
            "corners+forced interior probes can miss thin oracle structure",
            "intensity-only may beat physics-aware on some budgets",
            "trace-cost can over-focus expensive low-visual-impact rays",
            "leaf-local nearest reconstruction is intentionally crude"
        ]
    })
}

fn build_failure_categories(
    report: &E1ExperimentReport,
    worst: &[serde_json::Value],
) -> serde_json::Value {
    let outcome_ev: Vec<_> = worst
        .iter()
        .filter(|w| w.get("metric").and_then(|m| m.as_str()) == Some("outcome_disagreement"))
        .cloned()
        .collect();
    let angular_ev: Vec<_> = worst
        .iter()
        .filter(|w| w.get("metric").and_then(|m| m.as_str()) == Some("celestial_angular"))
        .cloned()
        .collect();
    let radiance_ev: Vec<_> = worst
        .iter()
        .filter(|w| w.get("metric").and_then(|m| m.as_str()) == Some("log2_iobs"))
        .cloned()
        .collect();

    let mut thin_celestial = Vec::new();
    for case in &report.cases {
        for method in &case.methods {
            for p in &method.points {
                if p.rgb.exact_match {
                    continue;
                }
                if p.scientific.celestial_presence_mismatch_count > 0 {
                    thin_celestial.push(serde_json::json!({
                        "case": case.case_id,
                        "method": method.method_id,
                        "budget": p.budget_id,
                        "celestial_presence_mismatch_count":
                            p.scientific.celestial_presence_mismatch_count,
                    }));
                }
            }
        }
    }

    let mut intensity_beats = Vec::new();
    for case in &report.cases {
        let dims = applicable_dims_for_case(case);
        let physics = case
            .methods
            .iter()
            .find(|m| m.method_id.contains("physics-aware"));
        let intensity = case
            .methods
            .iter()
            .find(|m| m.method_id.contains("intensity"));
        if let (Some(physics), Some(intensity)) = (physics, intensity) {
            for p in &physics.points {
                if p.rgb.exact_match {
                    continue;
                }
                if let Some(i) = matched_point(&intensity.points, p.unique_traced_rays) {
                    if error_improves_at_matched_budget(i, p, dims) {
                        intensity_beats.push(serde_json::json!({
                            "case": case.case_id,
                            "physics_budget": p.budget_id,
                            "physics_rays": p.unique_traced_rays,
                            "intensity_budget": i.budget_id,
                            "intensity_rays": i.unique_traced_rays,
                        }));
                    }
                }
            }
        }
    }

    let mut trace_cost = Vec::new();
    for case in &report.cases {
        for method in &case.methods {
            if !method.method_id.contains("physics-aware") {
                continue;
            }
            for p in &method.points {
                if p.rgb.exact_match {
                    continue;
                }
                for w in &p.worst_pixels {
                    if let Some(f) = &w.feature_vector_at_last_split {
                        let cost = f.cost_component;
                        let others = f
                            .luma_component
                            .max(f.outcome_component)
                            .max(f.angular_component)
                            .max(f.uv_component)
                            .max(f.g_component)
                            .max(f.radiance_component);
                        if cost > others && cost > 0.0 {
                            trace_cost.push(serde_json::json!({
                                "case": case.case_id,
                                "method": method.method_id,
                                "budget": p.budget_id,
                                "metric": w.metric,
                                "trace_cost": cost,
                                "max_other_feature": others,
                            }));
                        }
                    }
                }
            }
        }
    }

    let mut ablation_regressions = Vec::new();
    for abl_case in &report.ablations {
        let Some(full) = report.cases.iter().find(|c| c.case_id == abl_case.case_id) else {
            continue;
        };
        let dims = applicable_dims_for_case(full);
        let Some(physics) = full
            .methods
            .iter()
            .find(|m| m.method_id.contains("physics-aware"))
        else {
            continue;
        };
        for abl_method in &abl_case.methods {
            for ap in &abl_method.points {
                if ap.rgb.exact_match {
                    continue;
                }
                if let Some(pp) = matched_point(&physics.points, ap.unique_traced_rays) {
                    // Ablation regression: full physics improves on the ablation at matched budget.
                    if error_improves_at_matched_budget(pp, ap, dims) {
                        ablation_regressions.push(serde_json::json!({
                            "case": abl_case.case_id,
                            "ablation": abl_method.method_id,
                            "ablation_budget": ap.budget_id,
                            "physics_budget": pp.budget_id,
                        }));
                    }
                }
            }
        }
    }

    serde_json::json!({
        "unresolved_outcome_islands": category_entry(!outcome_ev.is_empty(), outcome_ev),
        "thin_celestial_features": category_entry(!thin_celestial.is_empty(), thin_celestial),
        "high_angular_error_regions": category_entry(!angular_ev.is_empty(), angular_ev),
        "radiance_failures": category_entry(!radiance_ev.is_empty(), radiance_ev),
        "intensity_only_beats_physics_aware": category_entry(
            !intensity_beats.is_empty(),
            intensity_beats
        ),
        "trace_cost_over_focus": category_entry(!trace_cost.is_empty(), trace_cost),
        "ablation_regressions": category_entry(
            !ablation_regressions.is_empty(),
            ablation_regressions
        ),
    })
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
    let dims = applicable_dims_for_case(case);
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
            continue;
        }
        let Some(u) = matched_point(&uniform.points, p.unique_traced_rays) else {
            continue;
        };
        let Some(i) = matched_point(&intensity.points, p.unique_traced_rays) else {
            continue;
        };
        if error_improves_at_matched_budget(p, u, dims)
            && error_improves_at_matched_budget(p, i, dims)
        {
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

/// Cross-method hypothesis compare: error improvement at matched budget.
/// Ray accounting is encoded by the match (`base.rays <= cand.rays`); do not
/// also require `cand.rays <= base.rays`.
pub fn error_improves_at_matched_budget(
    cand: &CurvePoint,
    base: &CurvePoint,
    dims: ApplicableScientificDims,
) -> bool {
    let bb = primary_errors(cand);
    let aa = primary_errors(base);
    if bb.outcome_rate > aa.outcome_rate {
        return false;
    }
    let mut le_all = bb.rgb_mse <= aa.rgb_mse && bb.outcome_rate <= aa.outcome_rate;
    let mut strict = bb.rgb_mse < aa.rgb_mse || bb.outcome_rate < aa.outcome_rate;
    let Some((ang_le, ang_st)) =
        optional_le_strict(bb.angular_rmse, aa.angular_rmse, dims.celestial_angular)
    else {
        return false;
    };
    le_all &= ang_le;
    strict |= ang_st;
    let Some((iobs_le, iobs_st)) =
        optional_le_strict(bb.log2_iobs_rmse, aa.log2_iobs_rmse, dims.log2_iobs)
    else {
        return false;
    };
    le_all &= iobs_le;
    strict |= iobs_st;
    le_all && strict
}

pub fn build_worst_pixel_records(
    oracle: &OracleFrame,
    recon: &AdaptiveReconstruction,
    leaves: &[QuadCell],
    schedule: &[ScheduleEvent],
    sci: &OracleComparisonMetrics,
    reference_ppm: &[u8],
) -> Vec<WorstPixelRecord> {
    let mut out = Vec::new();
    if let Some(m) = &sci.celestial_angular_error_radians {
        if m.maximum_absolute_error > 0.0 {
            if let Some(r) = record_at(
                oracle,
                recon,
                leaves,
                schedule,
                "celestial_angular",
                m.maximum_absolute_error,
                m.maximum_error_index,
            ) {
                out.push(r);
            }
        }
    }
    if let Some(m) = &sci.log2_observed_error {
        if m.maximum_absolute_error > 0.0 {
            if let Some(r) = record_at(
                oracle,
                recon,
                leaves,
                schedule,
                "log2_iobs",
                m.maximum_absolute_error,
                m.maximum_error_index,
            ) {
                out.push(r);
            }
        }
    }
    if sci.outcome_disagreement_count > 0 {
        if let Some(idx) = first_outcome_disagreement(oracle, recon) {
            if let Some(r) = record_at(
                oracle,
                recon,
                leaves,
                schedule,
                "outcome_disagreement",
                1.0,
                idx,
            ) {
                out.push(r);
            }
        }
    }
    if let Some(r) = worst_rgb_record(oracle, recon, leaves, schedule, reference_ppm) {
        out.push(r);
    }
    out
}

fn first_outcome_disagreement(oracle: &OracleFrame, recon: &AdaptiveReconstruction) -> Option<u64> {
    for (i, (o, c)) in oracle.pixels.iter().zip(&recon.pixels).enumerate() {
        if o.outcome_class != c.outcome_class {
            return Some(i as u64);
        }
    }
    None
}

fn record_at(
    oracle: &OracleFrame,
    recon: &AdaptiveReconstruction,
    leaves: &[QuadCell],
    schedule: &[ScheduleEvent],
    metric: &str,
    error: f64,
    index: u64,
) -> Option<WorstPixelRecord> {
    let i = index as usize;
    let o: &OraclePixel = oracle.pixels.get(i)?;
    let c: &AdaptiveReconstructedPixel = recon.pixels.get(i)?;
    let leaf = find_leaf(leaves, c.local_col, c.local_row)?;
    let (last_split, features) = last_split_for(schedule, c.local_col, c.local_row);
    let (prov_col, prov_row) = source_index_to_col_row(
        c.provenance_source_index,
        /* width from oracle source? */ infer_source_width(oracle),
    );
    Some(WorstPixelRecord {
        metric: metric.into(),
        error,
        target_local_col: c.local_col,
        target_local_row: c.local_row,
        target_source_col: c.source_col,
        target_source_row: c.source_row,
        provenance_source_index: c.provenance_source_index,
        provenance_source_col: prov_col,
        provenance_source_row: prov_row,
        oracle_outcome: o.outcome_class,
        candidate_outcome: c.outcome_class,
        leaf_rectangle: leaf.rect,
        leaf_depth: leaf.depth,
        last_split_rectangle: last_split,
        feature_vector_at_last_split: features,
    })
}

fn infer_source_width(_oracle: &OracleFrame) -> u32 {
    // E0 corpus sources are always 128×128; crops retain source indices into that grid.
    128
}

fn source_index_to_col_row(source_index: u64, source_width: u32) -> (u32, u32) {
    let w = u64::from(source_width);
    ((source_index % w) as u32, (source_index / w) as u32)
}

fn last_split_for(
    schedule: &[ScheduleEvent],
    local_col: u32,
    local_row: u32,
) -> (Option<PixelRect>, Option<FeatureVector>) {
    for ev in schedule.iter().rev() {
        if let Some(sel) = &ev.selected {
            if sel.contains_local(local_col, local_row) {
                return (Some(*sel), ev.features.clone());
            }
        }
    }
    (None, None)
}

/// Build RGB worst-pixel record using reference PPM channel errors.
pub fn worst_rgb_record(
    oracle: &OracleFrame,
    recon: &AdaptiveReconstruction,
    leaves: &[QuadCell],
    schedule: &[ScheduleEvent],
    reference_ppm: &[u8],
) -> Option<WorstPixelRecord> {
    let payload = ppm_payload_offset(reference_ppm)?;
    let mut best: Option<(u64, u8)> = None;
    for (i, c) in recon.pixels.iter().enumerate() {
        let off = payload + ((c.local_row * recon.width + c.local_col) * 3) as usize;
        if off + 2 >= reference_ppm.len() {
            continue;
        }
        let r = [
            reference_ppm[off],
            reference_ppm[off + 1],
            reference_ppm[off + 2],
        ];
        let d = r[0]
            .abs_diff(c.rgb[0])
            .max(r[1].abs_diff(c.rgb[1]))
            .max(r[2].abs_diff(c.rgb[2]));
        if d == 0 {
            continue;
        }
        let idx = i as u64;
        if best.is_none_or(|(bi, bd)| d > bd || (d == bd && idx < bi)) {
            best = Some((idx, d));
        }
    }
    let (idx, err) = best?;
    record_at(
        oracle,
        recon,
        leaves,
        schedule,
        "rgb_channel_abs",
        f64::from(err),
        idx,
    )
}

fn ppm_payload_offset(ppm: &[u8]) -> Option<usize> {
    let mut n = 0;
    for (i, b) in ppm.iter().enumerate() {
        if *b == b'\n' {
            n += 1;
            if n == 3 {
                return Some(i + 1);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e1_adaptive_sampling::metrics::SampleParityReport;

    fn point(rays: u64, outcome: f64, mse: f64, ang: Option<f64>, iobs: Option<f64>) -> CurvePoint {
        use relativity_oracle::{IntegerErrorMetrics, ScalarErrorMetrics};
        CurvePoint {
            budget_id: format!("r{rays}"),
            leaf_size: 8,
            unique_traced_rays: rays,
            ray_fraction: 0.0,
            total_rhs_evaluations: 0,
            mean_rhs_per_ray: 0.0,
            maximum_rhs: 0,
            scientific: OracleComparisonMetrics {
                compared_pixels: 1,
                outcome_disagreement_count: if outcome > 0.0 { 1 } else { 0 },
                outcome_disagreement_rate: outcome,
                rhs_absolute_error: IntegerErrorMetrics {
                    mae: 0.0,
                    rmse: 0.0,
                    maximum_absolute_error: 0,
                    maximum_error_index: 0,
                },
                celestial_pair_count: u64::from(ang.is_some()),
                celestial_presence_mismatch_count: 0,
                celestial_angular_error_radians: ang.map(|rmse| ScalarErrorMetrics {
                    mae: rmse,
                    rmse,
                    maximum_absolute_error: rmse,
                    maximum_error_index: 0,
                }),
                celestial_wrap_u_error: None,
                celestial_v_error: None,
                disk_pair_count: u64::from(iobs.is_some()),
                disk_presence_mismatch_count: 0,
                log2_g_error: None,
                log2_emitted_error: None,
                log2_observed_error: iobs.map(|rmse| ScalarErrorMetrics {
                    mae: rmse,
                    rmse,
                    maximum_absolute_error: rmse,
                    maximum_error_index: 0,
                }),
            },
            rgb: RgbComparisonMetrics {
                pixel_count: 1,
                channel_mse: mse,
                maximum_absolute_channel_error: 0,
                exact_match: mse == 0.0,
                psnr_db: None,
            },
            sample_parity: SampleParityReport {
                selected_sample_count: 0,
                selected_sample_exact_count: 0,
                selected_sample_mismatch_count: 0,
            },
            schedule: vec![],
            worst_pixels: vec![],
            wall_clock_seconds: 0.0,
            reconstruction_wall_clock_seconds: 0.0,
            metric_wall_clock_seconds: 0.0,
        }
    }

    #[test]
    fn pareto_uses_angular_and_iobs_dimensions() {
        let dims = ApplicableScientificDims {
            celestial_angular: true,
            log2_iobs: true,
        };
        let a = point(100, 0.0, 10.0, Some(0.5), Some(0.5));
        let b = point(100, 0.0, 10.0, Some(0.1), Some(0.5)); // better angular, same else
        assert!(dominates(&b, &a, dims));
        let c = point(100, 0.0, 10.0, Some(0.5), Some(0.1));
        assert!(dominates(&c, &a, dims));
        let d = point(90, 0.0, 10.0, Some(0.9), Some(0.5)); // fewer rays but worse angular
        assert!(!dominates(&d, &a, dims));
    }

    #[test]
    fn matched_budget_allows_overshoot_ray_counts() {
        let dims = ApplicableScientificDims {
            celestial_angular: true,
            log2_iobs: false,
        };
        let cand = point(148, 0.01, 5.0, Some(0.1), None);
        let base = point(144, 0.02, 8.0, Some(0.2), None);
        assert!(error_improves_at_matched_budget(&cand, &base, dims));
        // Frontier dominates still requires cand.rays <= base.rays
        assert!(!dominates(&cand, &base, dims));
    }

    #[test]
    fn mixed_optional_metric_blocks_dominance_and_matched_win() {
        let dims = ApplicableScientificDims {
            celestial_angular: true,
            log2_iobs: true,
        };
        let a = point(100, 0.0, 10.0, Some(0.5), Some(0.5));
        let b = point(100, 0.0, 9.0, Some(0.4), None); // better rgb/ang but missing iobs
        assert!(!dominates(&b, &a, dims));
        assert!(!error_improves_at_matched_budget(&b, &a, dims));
    }

    #[test]
    fn outcome_regression_blocks_matched_win() {
        let dims = ApplicableScientificDims {
            celestial_angular: false,
            log2_iobs: false,
        };
        let cand = point(148, 0.05, 1.0, None, None);
        let base = point(144, 0.02, 8.0, None, None);
        assert!(!error_improves_at_matched_budget(&cand, &base, dims));
    }
}
