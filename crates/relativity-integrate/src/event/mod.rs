//! Sign-changing event surfaces and project-owned root finding.
//!
//! Exclusions (not supported in Gate 1B1):
//! - tangent contact
//! - roots with identical endpoint signs
//! - discontinuous event functions

mod escape;
mod horizon;
mod root;
mod surface;

pub use escape::EscapeSphere;
pub use horizon::OuterHorizon;
pub use root::{localize_sign_change, EventLocalizationStats};
pub use surface::{
    is_eligible_crossing, is_eligible_crossing_tol, CrossingDirection, EventId, EventSurface,
};
