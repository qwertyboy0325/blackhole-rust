//! Production CPU `f64` DOP853 geodesic integration adapter.
//!
//! Isolates `ivp` behind project-owned types. No image, GPU, GUI, or disk
//! emission dependencies.
//!
//! # Event exclusions (Gate 1B1)
//!
//! Not supported or claimed:
//! - tangent contact
//! - roots with identical endpoint signs (proximity ≠ EventHit)
//! - discontinuous event functions
//!
//! Exact events require a strict sign-changing bracket or an exact endpoint
//! root (`f == 0.0`). Opt-in `HorizonProximityPolicy` may yield
//! `SurfaceApproach` for OuterHorizon only — never an `EventHit`.

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
pub use config::{Dop853Config, HorizonProximityPolicy};
pub use corpus::{
    build_canonical_corpus_report, canonical_corpus_json, determinism_record, run_and_check,
    run_corpus_case, CanonicalCaseRecord, CanonicalCorpusReport, CorpusCase, CorpusId,
    DeterminismRecord, ErrorClass, ExpectedOutcome, CORPUS,
};
pub use error::{IntegrationError, IntegrationStage};
pub use event::{
    is_eligible_crossing, is_exact_root, localization_nonconvergence_self_check, CrossingDirection,
    EscapeSphere, EventId, EventLocalizationStats, EventSurface,
    LocalizationNonconvergenceEvidence, LocalizationTermination, OuterHorizon,
    MAX_LOCALIZATION_ITERS,
};
pub use outcome::{
    EventHit, IntegrationOutcome, IntegrationReport, IntegrationStats, InvariantDiagnostics,
    RawSolverStop, SurfaceApproach, SurfaceApproachReason,
};
pub use state::{AffineParameter, GeodesicState};
