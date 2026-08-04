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
pub mod image;
pub mod outcome;
pub mod scene;
pub mod trace;

pub use camera::{pixel_index, sensor_at_pixel_center, TraceGrid};
pub use convergence::{
    run_convergence_probe, ConvergenceCandidateResult, ConvergenceProbeReport,
    ConvergenceProbeStatus,
};
pub use corpus::{camera_corpus, run_camera_corpus, CameraCorpusCase, CorpusId};
pub use diagnostics::{
    build_outcome_map_report, hex_sha, outcome_class_bytes, OutcomeCounts, OutcomeMapReport,
    PixelCoord, RhsDistribution,
};
pub use disk::{ThinDisk, ThinDiskGeometry};
pub use image::{class_rgb, write_outcome_ppm, write_rhs_pgm};
pub use outcome::{
    map_integration_report, AffineLimitOutcome, DiskHit, EscapeHit, OutcomeClass, RayFailure,
    RayOutcome,
};
pub use scene::TraceScene;
pub use trace::{trace_grid, trace_ray_pixel, trace_ray_sensor, TraceBundle};
