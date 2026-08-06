//! Typed domain and conditioning failures for Gate 1A.

use thiserror::Error;

/// Construction or evaluation failure that must not be painted as physics.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CoreError {
    #[error("non-finite input: {context}")]
    NonFinite { context: &'static str },

    #[error("invalid Kerr mass M={mass}: must be finite and > 0")]
    InvalidMass { mass: f64 },

    #[error("invalid Kerr spin a={spin} for M={mass}: require finite |a| <= M")]
    InvalidSpin { mass: f64, spin: f64 },

    #[error("chart domain failure at ({x}, {y}, {z}): {reason}")]
    ChartDomain {
        x: f64,
        y: f64,
        z: f64,
        reason: DomainReason,
    },

    #[error("numerically unresolved quantity: {context}")]
    Unresolved { context: &'static str },

    #[error("ill-conditioned transform: {context}")]
    IllConditioned { context: &'static str },

    #[error("observer model invalid at event: {context}")]
    InvalidObserver { context: &'static str },

    #[error("tetrad construction failed: {context}")]
    TetradFailure { context: &'static str },

    #[error("ray initialization failed: {context}")]
    RayInit { context: &'static str },

    #[error("invalid measured frequency: {context}")]
    InvalidFrequency { context: &'static str },

    #[error("circular equatorial orbit unavailable: {context}")]
    CircularOrbitUnavailable { context: &'static str },
}

/// Chart/domain classification for Cartesian Kerr–Schild evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DomainReason {
    #[error("ring singularity / excluded singular disk (r = 0)")]
    RingSingularityOrExcludedDisk,
    #[error("oblate radius evaluation unresolved")]
    RadiusUnresolved,
    #[error("metric scalar denominator unresolved")]
    MetricDenominatorUnresolved,
    #[error("coordinate singularity in Boyer–Lindquist chart")]
    BoyerLindquistSingular,
}

/// Compact status for diagnostics (finite/domain/conditioning).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalStatus {
    Ok,
    NonFinite,
    DomainFailure,
    IllConditioned,
}
