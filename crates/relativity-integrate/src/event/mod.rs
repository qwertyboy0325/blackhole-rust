//! Sign-changing event surfaces and project-owned root finding.
//!
//! Exclusions (Gate 1B1 / 1B2):
//! - tangent contact
//! - roots with identical endpoint signs (proximity ≠ event)
//! - discontinuous event functions
//! - even number of plane crossings inside one accepted step
//! - same-sign endpoint multiple roots
//!
//! `event_value_tolerance` is a localization convergence tolerance only.
//! Horizon proximity uses the separate opt-in `HorizonProximityPolicy`.
//! Annulus filtering uses [`EventSurface::classify_localized_hit`].

mod escape;
mod horizon;
mod metadata;
mod root;
mod surface;

pub use escape::EscapeSphere;
pub use horizon::OuterHorizon;
pub use metadata::{DiskCrossingSide, EventMetadata, LocalizedSurfaceHit};
pub use root::{
    localization_nonconvergence_self_check, localize_sign_change, EventLocalizationStats,
    LocalizationNonconvergenceEvidence, LocalizationTermination, MAX_LOCALIZATION_ITERS,
};
pub use surface::{is_eligible_crossing, is_exact_root, CrossingDirection, EventId, EventSurface};
