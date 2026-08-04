//! Typed celestial render errors.

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
