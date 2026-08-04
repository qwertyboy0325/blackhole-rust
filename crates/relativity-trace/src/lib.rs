//! Thin-disk ray termination, outcome classification, and CPU outcome maps.
//!
//! Gate 1B2: diagnostic classification image only — no radiometry, textures,
//! OpenEXR, GPU, or GUI.
//!
//! # Event limitations
//!
//! Only roots visible via accepted-step endpoint sign change or exact endpoint
//! root are detected. Not claimed: even numbers of plane crossings in one step,
//! same-sign multiple roots, tangent disk contact.

#![forbid(unsafe_code)]

pub mod camera;
pub mod convergence;
pub mod corpus;
pub mod diagnostics;
pub mod disk;
pub mod execution;
pub mod image;
pub mod outcome;
pub mod scene;
pub mod shade;
pub mod trace;
pub mod trace_digest;

pub use camera::{pixel_index, sensor_at_pixel_center, TraceGrid};
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
pub use trace::{
    fold_indexed_results, trace_grid, trace_grid_with_execution, trace_ray_pixel, trace_ray_sensor,
    TraceBundle,
};
pub use trace_digest::trace_data_digest;
