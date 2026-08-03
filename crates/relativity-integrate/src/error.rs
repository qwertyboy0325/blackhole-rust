//! Project-owned integration errors (no `ivp` types).

use relativity_core::CoreError;
use thiserror::Error;

use crate::event::EventId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationStage {
    Config,
    InitialState,
    Rhs,
    Callback,
    Localization,
    Outcome,
}

#[derive(Debug, Error, Clone)]
pub enum IntegrationError {
    #[error("invalid config field `{field}`")]
    InvalidConfig { field: &'static str },

    #[error("physics domain: {source}")]
    PhysicsDomain { source: CoreError },

    #[error("event domain for {event_id:?}: {detail}")]
    EventDomain { event_id: EventId, detail: String },

    #[error("non-finite state at stage {stage:?}")]
    NonFiniteState { stage: IntegrationStage },

    #[error("solver failure: {detail}")]
    Solver { detail: String },

    #[error("accepted step limit exceeded ({accepted_steps})")]
    StepLimitExceeded { accepted_steps: u64 },

    #[error("missing event outcome after interrupt")]
    MissingEventOutcome,

    #[error("invalid interpolant bounds")]
    InvalidInterpolantBounds,

    #[error(
        "event localization did not converge for {event_id:?}: iterations={iterations}, residual={residual}, bracket_width={bracket_width}"
    )]
    EventLocalizationDidNotConverge {
        event_id: EventId,
        iterations: u64,
        residual: f64,
        bracket_width: f64,
    },
}

impl IntegrationError {
    pub fn from_core(source: CoreError) -> Self {
        Self::PhysicsDomain { source }
    }
}
