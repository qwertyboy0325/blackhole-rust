//! Deterministic diagnostic celestial texture sampling and lensed RGB frames.
//!
//! Gate 2A2: procedural coordinate-grid celestial field sampled through Gate 2A1
//! finite-boundary coordinates. Not physical radiometry, not asymptotic infinity.

#![forbid(unsafe_code)]

pub mod error;
pub mod lensed;
pub mod texture;

pub use error::CelestialRenderError;
pub use lensed::{
    render_lensed_celestial, validate_mode_surface_set, verify_lensed_celestial_frame,
    LensedCelestialFrame, LensedCelestialMode, OutcomeColorCounts, AFFINE_LIMIT_RGB, FAILED_RGB,
    HORIZON_RGB, OPAQUE_DISK_MASK_RGB,
};
pub use texture::{
    procedural_coordinate_grid_v1, procedural_texture_spec_digest,
    render_procedural_texture_reference, sample_procedural_celestial,
    ProceduralCelestialTextureSpec, TEXTURE_ID_V1,
};
