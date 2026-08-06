//! E1 physics-aware adaptive quadtree sampling (experimental track).

pub mod config;
pub mod experiment;
pub mod metrics;
pub mod quadtree;
pub mod reconstruct;
pub mod reference_session;
pub mod report;
pub mod sample;
pub mod score;

pub use experiment::{run, ExperimentFilters, ExperimentOptions, LadderMode, WriteArtifacts};
pub use score::MethodId;
