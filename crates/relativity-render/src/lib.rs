//! Deterministic diagnostic celestial texture sampling and lensed RGB frames.
//!
//! Gate 2A2: procedural coordinate-grid celestial field sampled through Gate 2A1
//! finite-boundary coordinates. Not physical radiometry, not asymptotic infinity.
//!
//! Gate 2B0: disk-hit frequency-shift kinematics (`g = ν_obs/ν_em`). Not emission.

#![forbid(unsafe_code)]

pub mod error;
pub mod frequency_shift;
pub mod lensed;
pub mod texture;

pub use error::{CelestialRenderError, FrequencyShiftError};
pub use frequency_shift::{
    build_disk_frequency_shift_frame, build_disk_frequency_shift_map_artifact,
    disk_frequency_shift_digest, g_factor_debug_rgb, g_visualization_range_counts,
    shade_g_factor_debug, verify_observer_unit_frequency, DiskFrequencyShiftConvention,
    DiskFrequencyShiftFrame, DiskFrequencyShiftMapArtifact, DiskFrequencyShiftPixel,
    DiskFrequencyShiftSample, DiskVelocityModel, FrequencyShiftRegressionSample,
    ObserverFrequencySource, ObserverFrequencyVerification, RankedFrequencyShiftPixel,
    EQUATORIAL_POLICY_V1, FREQUENCY_SHIFT_CONVENTION_ID, OBSERVER_UNIT_FREQUENCY_TOLERANCE,
};
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
