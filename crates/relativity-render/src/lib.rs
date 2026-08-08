//! Deterministic diagnostic celestial texture sampling and lensed RGB frames.
//!
//! Gate 2A2: procedural coordinate-grid celestial field sampled through Gate 2A1
//! finite-boundary coordinates. Not physical radiometry, not asymptotic infinity.
//!
//! Gate 2B0: disk-hit frequency-shift kinematics (`g = ν_obs/ν_em`). Not emission.
//! Gate 2B1: diagnostic bolometric emission and `g⁴` transport. Not spectra/RGB.
//! Gate 2B2: diagnostic spectral `I_ν` transport (`g³`). Not physical RGB/OpenEXR.

#![forbid(unsafe_code)]

pub mod bolometric;
pub mod error;
pub mod frequency_shift;
pub mod lensed;
pub mod spectral;
pub mod texture;

pub use bolometric::{
    bolometric_debug_display_spec_digest, bolometric_debug_display_v1,
    bolometric_display_range_counts, bolometric_intensity_debug_rgb, build_disk_bolometric_frame,
    build_disk_bolometric_map_artifact, canonical_g_fourth,
    diagnostic_bolometric_emission_spec_digest, diagnostic_bolometric_emission_v1,
    disk_bolometric_digest, render_bolometric_celestial_composite,
    sample_diagnostic_bolometric_emission, shade_emitted_bolometric_debug,
    shade_observed_bolometric_debug, transport_bolometric_specific_intensity,
    validate_disk_emission_provenance, verify_disk_bolometric_frame, BolometricDebugDisplaySpec,
    BolometricRegressionSample, BolometricSpecificIntensity, BolometricTransportFactor,
    DiagnosticAngularEmissionModel, DiagnosticBolometricEmissionSpec, DiskBolometricConvention,
    DiskBolometricFrame, DiskBolometricMapArtifact, DiskBolometricPixel, DiskBolometricSample,
    EmissionNormalizationRadiusSource, RankedBolometricPixel, ResolvedDiskBounds,
    BOLOMETRIC_CONVENTION_ID, CANONICAL_DISK_EMISSION_CLAIM, CANONICAL_DISK_EMISSION_MODEL,
    DISK_BOUNDS_SOURCE_V1, DISPLAY_ID_V1, EMISSION_PROFILE_ID_V1,
};
pub use error::{
    BolometricRenderError, CelestialRenderError, FrequencyShiftError, SpectralRenderError,
};
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
pub use spectral::{
    build_disk_spectral_frame, compute_bolometric_closure, continuum_mass_on_interval,
    continuum_normalization, diagnostic_gaussian_line_v1, diagnostic_lognormal_continuum_v1,
    diagnostic_spectrum_spec_digest, disk_spectral_digest, evaluate_continuum_phi,
    evaluate_line_fixture, independent_i_nu_obs, spectral_grid_digest, DiagnosticLineFixture,
    DiagnosticSpectrumSpec, DiskSpectralConvention, SpectralClosureMetrics, SpectralDiskSample,
    SpectralFrame, SpectralPixel, CONTINUUM_SPECTRUM_ID, EMITTER_DOMAIN_POLICY, LINE_FIXTURE_ID,
    SPECTRAL_CONVENTION_ID, SPECTRAL_UNITS_V1,
};
pub use texture::{
    procedural_coordinate_grid_v1, procedural_texture_spec_digest,
    render_procedural_texture_reference, sample_procedural_celestial,
    ProceduralCelestialTextureSpec, TEXTURE_ID_V1,
};
