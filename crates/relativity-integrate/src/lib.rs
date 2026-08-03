//! Production CPU `f64` DOP853 geodesic integration adapter.
//!
//! Isolates `ivp` behind project-owned types. No image, GPU, GUI, or disk
//! emission dependencies.
//!
//! # Event exclusions (Gate 1B1)
//!
//! Not supported or claimed:
//! - tangent contact
//! - roots with identical endpoint signs
//! - discontinuous event functions

#![forbid(unsafe_code)]

pub mod adapter;
pub mod config;
pub mod corpus;
pub mod error;
pub mod event;
pub mod outcome;
pub mod rhs;
pub mod state;

pub use adapter::{integrate, integrate_to_affine_limit};
pub use config::Dop853Config;
pub use corpus::{
    determinism_record, run_and_check, run_corpus_case, CorpusCase, CorpusId, DeterminismRecord,
    ErrorClass, ExpectedOutcome, CORPUS,
};
pub use error::{IntegrationError, IntegrationStage};
pub use event::{
    is_eligible_crossing, is_eligible_crossing_tol, CrossingDirection, EscapeSphere, EventId,
    EventLocalizationStats, EventSurface, OuterHorizon,
};
pub use outcome::{
    EventHit, IntegrationOutcome, IntegrationReport, IntegrationStats, InvariantDiagnostics,
    RawSolverStop,
};
pub use state::{AffineParameter, GeodesicState};
