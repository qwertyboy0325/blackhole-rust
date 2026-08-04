//! Thin-disk ray termination, outcome classification, and CPU outcome maps.
//!
//! Gate 1B2: diagnostic classification image only — no radiometry, textures,
//! OpenEXR, GPU, or GUI.
//!
//! Gate 2A1 adds finite celestial-boundary coordinate mapping (UV from escape
//! positions). It does not sample textures or compute radiance.
//!
//! # Event limitations
//!
//! Only roots visible via accepted-step endpoint sign change or exact endpoint
//! root are detected. Not claimed: even numbers of plane crossings in one step,
//! same-sign multiple roots, tangent disk contact.

#![forbid(unsafe_code)]

pub mod camera;
pub mod celestial;
pub mod convergence;
pub mod corpus;
pub mod diagnostics;
pub mod disk;
pub mod execution;
pub mod image;
pub mod outcome;
pub mod scene;
pub mod shade;
pub mod surface_set;
pub mod trace;
pub mod trace_digest;

pub use camera::{pixel_index, sensor_at_pixel_center, TraceGrid};
pub use celestial::{
    build_celestial_coordinate_frame, build_celestial_coordinate_map_artifact,
    build_celestial_regression_corpus, celestial_coordinate_digest, celestial_sample_from_escape,
    celestial_sample_from_position, shade_celestial_uv_debug, validate_celestial_seam,
    worst_boundary_residual_pixels, wrap_psi_0_2pi, CelestialBoundarySample,
    CelestialCoordinateConvention, CelestialCoordinateFrame, CelestialCoordinateMapArtifact,
    CelestialCoordinatePixel, CelestialCoordinatePixelRecord, CelestialDirectionSource,
    CelestialMappingError, CelestialRegressionSample, CelestialUv, ACCEPTED_SEAM,
    CELESTIAL_CONVENTION_ID, RADIUS_POLICY_GATE_1B2_CAP,
};
pub use convergence::{
    run_convergence_probe, ConvergenceCandidateResult, ConvergenceProbeReport,
    ConvergenceProbeStatus,
};
pub use corpus::{camera_corpus, run_camera_corpus, CameraCorpusCase, CorpusId};
pub use diagnostics::{
    build_outcome_map_report, hex_sha, outcome_class_bytes, FailureCount, OutcomeCounts,
    OutcomeMapReport, PixelCoord, RhsDistribution,
};
pub use disk::{ThinDisk, ThinDiskGeometry};
pub use execution::{TraceExecution, TraceExecutionMetadata, TraceExecutionMode};
pub use image::{class_rgb, encode_ppm, write_outcome_ppm, write_rhs_pgm};
pub use outcome::{
    map_integration_report, AffineLimitOutcome, DiskHit, EscapeHit, OutcomeClass, RayFailure,
    RayOutcome,
};
pub use scene::TraceScene;
pub use shade::{
    categorical_rgb, disk_suppressed_rgb, rgb_frame_diff_count, shade_diagnostic, shade_many,
    shade_outcome, shade_trace_bundle, DiagnosticShadeStyle, RgbFrame, ShadedFrame,
};
pub use surface_set::TraceSurfaceSet;
pub use trace::{
    fold_indexed_results, trace_grid, trace_grid_with_execution,
    trace_grid_with_execution_and_surface_set, trace_ray_pixel, trace_ray_pixel_with_surface_set,
    trace_ray_sensor, trace_ray_sensor_with_surface_set, TraceBundle,
};
pub use trace_digest::trace_data_digest;
