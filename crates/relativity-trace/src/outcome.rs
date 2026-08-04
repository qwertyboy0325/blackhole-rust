//! Project-owned per-ray outcome taxonomy (classification, not radiometry).

use relativity_integrate::{
    AffineParameter, DiskCrossingSide, EventHit, EventId, EventMetadata, GeodesicState,
    IntegrationError, IntegrationOutcome, IntegrationReport, IntegrationStats,
    InvariantDiagnostics, RawSolverStop, SurfaceApproach,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiskHit {
    pub lambda: AffineParameter,
    pub state: GeodesicState,
    pub oblate_radius: f64,
    pub crossing_side: DiskCrossingSide,
    pub raw_solver_stop: RawSolverStop,
    pub integration: IntegrationStats,
    pub diagnostics: InvariantDiagnostics,
    pub event_value: f64,
    pub localization: relativity_integrate::EventLocalizationStats,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EscapeHit {
    pub lambda: AffineParameter,
    pub state: GeodesicState,
    pub raw_solver_stop: RawSolverStop,
    pub integration: IntegrationStats,
    pub diagnostics: InvariantDiagnostics,
    pub event_value: f64,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AffineLimitOutcome {
    pub lambda: AffineParameter,
    pub state: GeodesicState,
    pub integration: IntegrationStats,
    pub diagnostics: InvariantDiagnostics,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RayFailure {
    pub error: IntegrationError,
}

impl RayFailure {
    pub fn class_name(&self) -> &'static str {
        match &self.error {
            IntegrationError::InvalidConfig { .. } => "InvalidConfig",
            IntegrationError::PhysicsDomain { .. } => "PhysicsDomain",
            IntegrationError::EventDomain { .. } => "EventDomain",
            IntegrationError::NonFiniteState { .. } => "NonFiniteState",
            IntegrationError::Solver { .. } => "Solver",
            IntegrationError::StepLimitExceeded { .. } => "StepLimitExceeded",
            IntegrationError::MissingEventOutcome => "MissingEventOutcome",
            IntegrationError::InvalidInterpolantBounds => "InvalidInterpolantBounds",
            IntegrationError::EventLocalizationDidNotConverge { .. } => {
                "EventLocalizationDidNotConverge"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RayOutcome {
    DiskHit(DiskHit),
    Escaped(EscapeHit),
    HorizonEvent(EventHit),
    HorizonApproach(SurfaceApproach),
    AffineLimit(AffineLimitOutcome),
    Failed(RayFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutcomeClass {
    DiskHit,
    Escaped,
    HorizonEvent,
    HorizonApproach,
    AffineLimit,
    Failed,
}

impl RayOutcome {
    pub fn class(&self) -> OutcomeClass {
        match self {
            Self::DiskHit(_) => OutcomeClass::DiskHit,
            Self::Escaped(_) => OutcomeClass::Escaped,
            Self::HorizonEvent(_) => OutcomeClass::HorizonEvent,
            Self::HorizonApproach(_) => OutcomeClass::HorizonApproach,
            Self::AffineLimit(_) => OutcomeClass::AffineLimit,
            Self::Failed(_) => OutcomeClass::Failed,
        }
    }

    pub fn rhs_evaluations(&self) -> u64 {
        match self {
            Self::DiskHit(h) => h.integration.rhs_evaluations,
            Self::Escaped(h) => h.integration.rhs_evaluations,
            Self::HorizonEvent(h) => h.integration.rhs_evaluations,
            Self::HorizonApproach(h) => h.integration.rhs_evaluations,
            Self::AffineLimit(h) => h.integration.rhs_evaluations,
            Self::Failed(_) => 0,
        }
    }

    pub fn state_finite(&self) -> bool {
        let arr = match self {
            Self::DiskHit(h) => h.state.to_array(),
            Self::Escaped(h) => h.state.to_array(),
            Self::HorizonEvent(h) => h.state.to_array(),
            Self::HorizonApproach(h) => h.state.to_array(),
            Self::AffineLimit(h) => h.state.to_array(),
            Self::Failed(_) => return true,
        };
        arr.iter().all(|v| v.is_finite())
    }
}

pub fn map_integration_report(report: IntegrationReport) -> RayOutcome {
    let diagnostics = report.diagnostics;
    match report.outcome {
        IntegrationOutcome::Event(hit) => match hit.event_id {
            EventId::ThinDisk => match hit.metadata {
                EventMetadata::ThinDisk {
                    oblate_radius,
                    crossing_side,
                } => RayOutcome::DiskHit(DiskHit {
                    lambda: hit.lambda,
                    state: hit.state,
                    oblate_radius,
                    crossing_side,
                    raw_solver_stop: hit.raw_solver_stop,
                    integration: hit.integration,
                    diagnostics,
                    event_value: hit.event_value,
                    localization: hit.localization,
                }),
                _ => RayOutcome::Failed(RayFailure {
                    error: IntegrationError::EventDomain {
                        event_id: EventId::ThinDisk,
                        detail: "ThinDisk EventHit missing ThinDisk metadata".into(),
                    },
                }),
            },
            EventId::EscapeSphere => RayOutcome::Escaped(EscapeHit {
                lambda: hit.lambda,
                state: hit.state,
                raw_solver_stop: hit.raw_solver_stop,
                integration: hit.integration,
                diagnostics,
                event_value: hit.event_value,
            }),
            EventId::OuterHorizon => RayOutcome::HorizonEvent(hit),
        },
        IntegrationOutcome::SurfaceApproach(a) => RayOutcome::HorizonApproach(a),
        IntegrationOutcome::AffineLimit {
            lambda,
            state,
            stats,
        } => RayOutcome::AffineLimit(AffineLimitOutcome {
            lambda,
            state,
            integration: stats,
            diagnostics,
        }),
    }
}
