//! Outcome-map diagnostics and deterministic digests.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::outcome::{OutcomeClass, RayOutcome};
use crate::trace::TraceBundle;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelCoord {
    pub col: u32,
    pub row: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeCounts {
    pub disk_hit: u64,
    pub escaped: u64,
    pub horizon_event: u64,
    pub horizon_approach: u64,
    pub affine_limit: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RhsDistribution {
    pub min: u64,
    pub median: u64,
    pub p90: u64,
    pub p99: u64,
    pub max: u64,
    pub mean: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureCount {
    pub class: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeMapReport {
    pub gate: String,
    pub width: u32,
    pub height: u32,
    pub preset_digest: String,
    pub commit: String,
    pub toolchain: String,
    pub target: String,
    pub outcome_class_digest: String,
    pub ppm_digest: String,
    pub pgm_digest: String,
    pub counts: OutcomeCounts,
    pub exact_event_count: u64,
    pub total_accepted_steps: u64,
    pub total_rejected_steps: u64,
    pub total_rhs_evaluations: u64,
    pub rhs: RhsDistribution,
    pub most_expensive_rays: Vec<PixelCoord>,
    pub failure_counts: Vec<FailureCount>,
    pub execution_mode: String,
    /// Wall-clock; excluded from content digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wall_clock_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rays_per_second: Option<f64>,
    pub content_digest_excluding_digest_field: String,
}

fn percentile_sorted(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p * (sorted.len() as f64 - 1.0)).round() as usize).min(sorted.len() - 1);
    sorted[idx]
}

pub fn hex_sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn outcome_class_bytes(bundle: &TraceBundle) -> Vec<u8> {
    bundle
        .outcomes
        .iter()
        .map(|o| match o.class() {
            OutcomeClass::DiskHit => 1u8,
            OutcomeClass::Escaped => 2,
            OutcomeClass::HorizonEvent => 3,
            OutcomeClass::HorizonApproach => 4,
            OutcomeClass::AffineLimit => 5,
            OutcomeClass::Failed => 6,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn build_outcome_map_report(
    bundle: &TraceBundle,
    ppm: &[u8],
    pgm: &[u8],
    preset_digest: &str,
    commit: &str,
    toolchain: &str,
    target: &str,
    wall_clock_seconds: Option<f64>,
) -> OutcomeMapReport {
    let mut counts = OutcomeCounts {
        disk_hit: 0,
        escaped: 0,
        horizon_event: 0,
        horizon_approach: 0,
        affine_limit: 0,
        failed: 0,
    };
    let mut rhs_vals = Vec::with_capacity(bundle.outcomes.len());
    let mut total_acc = 0u64;
    let mut total_rej = 0u64;
    let mut total_rhs = 0u64;
    let mut exact_events = 0u64;
    let mut fail_map: std::collections::BTreeMap<&'static str, u64> =
        std::collections::BTreeMap::new();
    let mut expensive: Vec<(u64, u32, u32)> = Vec::new();

    for row in 0..bundle.grid.height {
        for col in 0..bundle.grid.width {
            let o = bundle.outcome_at(col, row);
            match o.class() {
                OutcomeClass::DiskHit => counts.disk_hit += 1,
                OutcomeClass::Escaped => counts.escaped += 1,
                OutcomeClass::HorizonEvent => {
                    counts.horizon_event += 1;
                    exact_events += 1;
                }
                OutcomeClass::HorizonApproach => counts.horizon_approach += 1,
                OutcomeClass::AffineLimit => counts.affine_limit += 1,
                OutcomeClass::Failed => counts.failed += 1,
            }
            if matches!(o.class(), OutcomeClass::DiskHit | OutcomeClass::Escaped) {
                exact_events += 1;
            }
            let rhs = o.rhs_evaluations();
            rhs_vals.push(rhs);
            total_rhs += rhs;
            match o {
                RayOutcome::DiskHit(h) => {
                    total_acc += h.integration.accepted_steps;
                    total_rej += h.integration.rejected_steps;
                }
                RayOutcome::Escaped(h) => {
                    total_acc += h.integration.accepted_steps;
                    total_rej += h.integration.rejected_steps;
                }
                RayOutcome::HorizonEvent(h) => {
                    total_acc += h.integration.accepted_steps;
                    total_rej += h.integration.rejected_steps;
                }
                RayOutcome::HorizonApproach(h) => {
                    total_acc += h.integration.accepted_steps;
                    total_rej += h.integration.rejected_steps;
                }
                RayOutcome::AffineLimit(h) => {
                    total_acc += h.integration.accepted_steps;
                    total_rej += h.integration.rejected_steps;
                }
                RayOutcome::Failed(f) => {
                    *fail_map.entry(f.class_name()).or_insert(0) += 1;
                }
            }
            expensive.push((rhs, col, row));
        }
    }

    rhs_vals.sort_unstable();
    let mean = if rhs_vals.is_empty() {
        0.0
    } else {
        total_rhs as f64 / rhs_vals.len() as f64
    };
    let rhs = RhsDistribution {
        min: *rhs_vals.first().unwrap_or(&0),
        median: percentile_sorted(&rhs_vals, 0.5),
        p90: percentile_sorted(&rhs_vals, 0.9),
        p99: percentile_sorted(&rhs_vals, 0.99),
        max: *rhs_vals.last().unwrap_or(&0),
        mean,
    };
    expensive.sort_by_key(|b| std::cmp::Reverse(b.0));
    let most_expensive_rays = expensive
        .into_iter()
        .take(8)
        .map(|(_, col, row)| PixelCoord { col, row })
        .collect();
    let failure_counts = fail_map
        .into_iter()
        .map(|(class, count)| FailureCount {
            class: class.into(),
            count,
        })
        .collect();

    let rays_per_second = wall_clock_seconds.and_then(|t| {
        if t > 0.0 {
            Some(bundle.outcomes.len() as f64 / t)
        } else {
            None
        }
    });

    let mut report = OutcomeMapReport {
        gate: "gate-1b2".into(),
        width: bundle.grid.width,
        height: bundle.grid.height,
        preset_digest: preset_digest.into(),
        commit: commit.into(),
        toolchain: toolchain.into(),
        target: target.into(),
        outcome_class_digest: hex_sha(&outcome_class_bytes(bundle)),
        ppm_digest: hex_sha(ppm),
        pgm_digest: hex_sha(pgm),
        counts,
        exact_event_count: exact_events,
        total_accepted_steps: total_acc,
        total_rejected_steps: total_rej,
        total_rhs_evaluations: total_rhs,
        rhs,
        most_expensive_rays,
        failure_counts,
        execution_mode: "serial".into(),
        wall_clock_seconds,
        rays_per_second,
        content_digest_excluding_digest_field: String::new(),
    };
    report.content_digest_excluding_digest_field = content_digest(&report);
    report
}

fn content_digest(report: &OutcomeMapReport) -> String {
    #[derive(Serialize)]
    struct Proj<'a> {
        gate: &'a str,
        width: u32,
        height: u32,
        preset_digest: &'a str,
        commit: &'a str,
        toolchain: &'a str,
        target: &'a str,
        outcome_class_digest: &'a str,
        ppm_digest: &'a str,
        pgm_digest: &'a str,
        counts: &'a OutcomeCounts,
        exact_event_count: u64,
        total_accepted_steps: u64,
        total_rejected_steps: u64,
        total_rhs_evaluations: u64,
        rhs: &'a RhsDistribution,
        most_expensive_rays: &'a [PixelCoord],
        failure_counts: &'a [FailureCount],
        execution_mode: &'a str,
        content_digest_excluding_digest_field: &'a str,
    }
    let proj = Proj {
        gate: &report.gate,
        width: report.width,
        height: report.height,
        preset_digest: &report.preset_digest,
        commit: &report.commit,
        toolchain: &report.toolchain,
        target: &report.target,
        outcome_class_digest: &report.outcome_class_digest,
        ppm_digest: &report.ppm_digest,
        pgm_digest: &report.pgm_digest,
        counts: &report.counts,
        exact_event_count: report.exact_event_count,
        total_accepted_steps: report.total_accepted_steps,
        total_rejected_steps: report.total_rejected_steps,
        total_rhs_evaluations: report.total_rhs_evaluations,
        rhs: &report.rhs,
        most_expensive_rays: &report.most_expensive_rays,
        failure_counts: &report.failure_counts,
        execution_mode: &report.execution_mode,
        content_digest_excluding_digest_field: "",
    };
    let bytes = serde_json::to_vec(&proj).expect("serialize");
    hex_sha(&bytes)
}
