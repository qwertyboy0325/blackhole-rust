//! Typed celestial / frequency-shift render errors.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CelestialRenderError {
    #[error("invalid procedural texture specification: {0}")]
    InvalidTextureSpec(String),
    #[error("unsupported texture id `{0}`")]
    UnsupportedTextureId(String),
    #[error("frame length mismatch")]
    FrameLengthMismatch,
    #[error("zero render dimensions rejected")]
    ZeroDimensions,
    #[error("mode/surface-set mismatch: mode={mode} surface_set={surface_set}")]
    ModeSurfaceMismatch { mode: String, surface_set: String },
    #[error("disk-omitted mode encountered DiskHit at ({col},{row})")]
    UnexpectedDiskHit { col: u32, row: u32 },
    #[error("celestial sample invalid: {0}")]
    InvalidSample(String),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum FrequencyShiftError {
    #[error("frame length mismatch")]
    FrameLengthMismatch,
    #[error("unsupported disk velocity model")]
    UnsupportedVelocityModel,
    #[error("frequency-shift core mapping failed: {context}")]
    CoreMapping { context: String },
    #[error("frequency-shift mapping failed at ({col},{row}): {cause}")]
    PixelMappingFailed { col: u32, row: u32, cause: String },
    #[error("observer unit-frequency verification failed at ({col},{row}): residual={residual}")]
    ObserverFrequencyVerification { col: u32, row: u32, residual: f64 },
    #[error("frequency-shift flag requires opaque-disk surface set and opaque-disk-mask mode")]
    FlagSurfaceModeMismatch,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum BolometricRenderError {
    #[error("frame length mismatch")]
    FrameLengthMismatch,
    #[error("grid dimension mismatch")]
    GridMismatch,
    #[error("invalid emission specification: {0}")]
    InvalidEmissionSpec(String),
    #[error("invalid display specification: {0}")]
    InvalidDisplaySpec(String),
    #[error("invalid disk bounds: {0}")]
    InvalidDiskBounds(String),
    #[error("radius outside resolved disk annulus: {radius}")]
    RadiusOutsideAnnulus { radius: f64 },
    #[error("invalid bolometric intensity: {0}")]
    InvalidIntensity(String),
    #[error("invalid transport factor: {0}")]
    InvalidTransportFactor(String),
    #[error("bolometric mapping failed at ({col},{row}): {cause}")]
    PixelMappingFailed { col: u32, row: u32, cause: String },
    #[error("bolometric verification failed at ({col},{row}): {cause}")]
    VerificationFailed { col: u32, row: u32, cause: String },
    #[error("composite rejected DiskHit encoded as escaped celestial sample at ({col},{row})")]
    DiskHitAsEscaped { col: u32, row: u32 },
    #[error("bolometric flag requires --emit-disk-frequency-shift")]
    FlagRequiresFrequencyShift,
    #[error("bolometric flag requires opaque-disk surface set and opaque-disk-mask mode")]
    FlagSurfaceModeMismatch,
    #[error("unsupported disk emission model `{0}`")]
    UnsupportedEmissionModel(String),
    #[error("unsupported disk emission claim `{0}`")]
    UnsupportedEmissionClaim(String),
    #[error("celestial texture error: {0}")]
    Celestial(String),
}
