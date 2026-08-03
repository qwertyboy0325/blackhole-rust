//! Sign-changing event surfaces and project-owned root finding.
//!
//! Exclusions (not supported in Gate 1B1):
//! - tangent contact
//! - roots with identical endpoint signs (proximity ≠ event)
//! - discontinuous event functions
//!
//! `event_value_tolerance` is a localization convergence tolerance only.
//! Horizon proximity uses the separate opt-in `HorizonProximityPolicy`.

mod escape;
mod horizon;
mod root;
mod surface;

pub use escape::EscapeSphere;
pub use horizon::OuterHorizon;
pub use root::{localize_sign_change, EventLocalizationStats, LocalizationTermination};
pub use surface::{is_eligible_crossing, is_exact_root, CrossingDirection, EventId, EventSurface};
