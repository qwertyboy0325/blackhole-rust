//! Project-owned integration outcomes.

use crate::event::{EventId, EventLocalizationStats};
use crate::state::{AffineParameter, GeodesicState};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IntegrationStats {
    pub accepted_steps: u64,
    pub rejected_steps: u64,
    pub rhs_evaluations: u64,
    pub callback_count: u64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawSolverStop {
    pub lambda: AffineParameter,
    pub state: GeodesicState,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EventHit {
    pub event_id: EventId,
    pub lambda: AffineParameter,
    pub state: GeodesicState,
    pub raw_solver_stop: RawSolverStop,
    pub event_value: f64,
    pub localization: EventLocalizationStats,
    pub integration: IntegrationStats,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IntegrationOutcome {
    Event(EventHit),
    AffineLimit {
        lambda: AffineParameter,
        state: GeodesicState,
        stats: IntegrationStats,
    },
}

impl IntegrationOutcome {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Event(_) => "Event",
            Self::AffineLimit { .. } => "AffineLimit",
        }
    }

    pub fn stats(&self) -> &IntegrationStats {
        match self {
            Self::Event(e) => &e.integration,
            Self::AffineLimit { stats, .. } => stats,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InvariantDiagnostics {
    pub h_initial: f64,
    pub h_final: f64,
    pub h_max_abs_residual: f64,
    pub p_t_initial: f64,
    pub p_t_final: f64,
    pub p_t_max_abs_drift: f64,
    pub non_finite_checks: u64,
    pub raw_vs_localized_lambda_separation: Option<f64>,
    pub relative_tolerance: [f64; 8],
    pub absolute_tolerance: [f64; 8],
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IntegrationReport {
    pub outcome: IntegrationOutcome,
    pub diagnostics: InvariantDiagnostics,
}
