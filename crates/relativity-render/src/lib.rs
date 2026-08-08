//! Deterministic diagnostic celestial texture sampling and lensed RGB frames.
//!
//! Gate 2A2: procedural coordinate-grid celestial field sampled through Gate 2A1
//! finite-boundary coordinates. Not physical radiometry, not asymptotic infinity.
//!
//! Gate 2B0: disk-hit frequency-shift kinematics (`g = ν_obs/ν_em`). Not emission.
//! Gate 2B1: diagnostic bolometric emission and `g⁴` transport. Not spectra/RGB.
//! Gate 2B2: diagnostic spectral `I_ν` transport (`g³`). Not physical RGB/OpenEXR.
//! Gate 2C0: physical Page–Thorne thin-disk emission, `T_eff`, Planck `B_ν`, SI Hz `g³`.
//! Gate 2C1: absolute CIE XYZ + scene-linear Rec.709/D65 RGB from emission frame (Arch B).

#![forbid(unsafe_code)]

pub mod bolometric;
pub mod color_space;
pub mod colorimetry;
pub mod error;
pub mod frequency_shift;
pub mod lensed;
pub mod page_thorne;
pub mod physical_disk;
pub mod physical_spectral;
pub mod planck;
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
pub use color_space::{
    SceneLinearRgb, SceneLinearRgbSpace, XyzToRgbMatrix, RGB_MATRIX_REVISION,
    SCENE_LINEAR_RGB_SPACE_ID,
};
pub use colorimetry::{
    blackbody_planckian_direction_ok, build_physical_color_frame, compute_colorimetric_metrics,
    decode_physical_color_pixels, diagnostic_a_vs_b, encode_physical_color_payload,
    integrate_xyz_from_emission, integrate_xyz_from_spectral_cube_diagnostic, outcome_class_code,
    outcome_class_from_code, payload_sha256, physical_color_digest, synthetic_cmf_for_tests,
    verify_payload_matches_frame, BlackbodyChromaticitySample, Cie1931Table, CieObserverId,
    CieSample, ColorDiskHit, ColorPixelProvenance, ColorimetricConvention, ColorimetricMetrics,
    ColorimetricXyz, IntegrationMeasure, PhysicalColorFrame, PhysicalColorPixel,
    CIE_OBSERVER_ID_V1, CIE_RELATIVE_ASSET_PATH, CIE_TABLE_MD5, CIE_TABLE_SHA256,
    CIE_TABLE_SOURCE_DOI, COLORIMETRIC_CONVENTION_ID, KM_LM_PER_W, KM_REVISION,
    PHYSICAL_COLOR_FRAME_SCHEMA, PRODUCTION_BAND_ID, PRODUCTION_LAMBDA_MAX_NM,
    PRODUCTION_LAMBDA_MIN_NM, PRODUCTION_N_SAMPLES, RAW_COLOR_PAYLOAD_MAGIC,
    RAW_COLOR_PAYLOAD_SCHEMA,
};
pub use error::{
    BolometricRenderError, CelestialRenderError, ColorimetryError, FrequencyShiftError,
    SpectralRenderError,
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
pub use page_thorne::{
    newtonian_zero_torque_flux, page_thorne_one_face_flux, page_thorne_one_face_flux_numerical,
    page_thorne_q, PageThorneRoots, ThinDiskFluxModel, FACE_POLICY, FLUX_MODEL_ID,
    NEWTONIAN_ORACLE_ID,
};
pub use physical_disk::{
    build_physical_disk_emission_frame, physical_disk_emission_digest,
    physical_disk_emission_spec_digest, sample_physical_disk_emission,
    validate_physical_emission_provenance, PhysicalDiskEmissionConvention,
    PhysicalDiskEmissionFrame, PhysicalDiskEmissionPixel, PhysicalDiskEmissionSample,
    PhysicalDiskEmissionSpec, PHYSICAL_DISK_EMISSION_CONVENTION_ID, PHYSICAL_EMISSION_CLAIM,
    PHYSICAL_EMISSION_MODEL_ID, PHYSICAL_FLUX_UNITS, PHYSICAL_TEFF_UNITS,
};
pub use physical_spectral::{
    build_physical_spectral_frame, compute_physical_spectral_closure,
    independent_physical_i_nu_obs, parse_physical_spectral_grid_id, physical_spectral_digest,
    physical_spectral_grid_digest, physical_spectral_grid_explore, physical_spectral_grid_v1,
    PhysicalSpectralClosureMetrics, PhysicalSpectralConvention, PhysicalSpectralDiskSample,
    PhysicalSpectralFrame, PhysicalSpectralPixel, PHYSICAL_GRID_EXPLORE_PREFIX,
    PHYSICAL_GRID_NU_MAX_HZ, PHYSICAL_GRID_NU_MIN_HZ, PHYSICAL_GRID_V1_ID, PHYSICAL_GRID_V1_N_BINS,
    PHYSICAL_SPECTRAL_CONVENTION_ID, PHYSICAL_SPECTRAL_UNITS,
};
pub use planck::{
    integrate_pi_b_nu_log_grid, planck_b_lambda_from_b_nu, planck_b_nu, stefan_boltzmann_flux,
    teff_from_one_face_flux, PLANCK_MODEL_ID, TEMPERATURE_MODEL_ID,
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
