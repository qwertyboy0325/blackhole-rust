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

#[derive(Debug, Error, Clone, PartialEq)]
pub enum SpectralRenderError {
    #[error("frame length mismatch")]
    FrameLengthMismatch,
    #[error("grid dimension mismatch")]
    GridMismatch,
    #[error("invalid spectrum specification: {0}")]
    InvalidSpectrumSpec(String),
    #[error("invalid spectral grid: {0}")]
    InvalidGrid(String),
    #[error("invalid spectral frequency: {0}")]
    InvalidFrequency(String),
    #[error("invalid spectral intensity: {0}")]
    InvalidIntensity(String),
    #[error("spectral mapping failed at ({col},{row}): {cause}")]
    PixelMappingFailed { col: u32, row: u32, cause: String },
    #[error("spectral verification failed at ({col},{row}): {cause}")]
    VerificationFailed { col: u32, row: u32, cause: String },
    #[error("spectral provenance mismatch: {0}")]
    ProvenanceMismatch(String),
    #[error("spectral flag requires frequency-shift and bolometric frames")]
    FlagRequiresBolometric,
    #[error("spectral flag requires opaque-disk surface set and opaque-disk-mask mode")]
    FlagSurfaceModeMismatch,
    #[error("unsupported spectrum id `{0}`")]
    UnsupportedSpectrumId(String),
    #[error("unsupported spectral grid id `{0}`")]
    UnsupportedGridId(String),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ColorimetryError {
    #[error("frame length mismatch")]
    FrameLengthMismatch,
    #[error("invalid CIE table: {0}")]
    InvalidCieTable(String),
    #[error("unsupported CIE observer `{0}`")]
    UnsupportedCieObserver(String),
    #[error("unsupported RGB space `{0}`")]
    UnsupportedRgbSpace(String),
    #[error("invalid colorimetric matrix: {0}")]
    InvalidMatrix(String),
    #[error("non-finite colorimetry value: {0}")]
    NonFinite(String),
    #[error("invalid colorimetry convention: {0}")]
    InvalidConvention(String),
    #[error("colorimetry mapping failed at ({col},{row}): {cause}")]
    PixelMappingFailed { col: u32, row: u32, cause: String },
    #[error("colorimetry provenance mismatch: {0}")]
    ProvenanceMismatch(String),
    #[error("physical emission error: {0}")]
    Emission(String),
    #[error("spectral error: {0}")]
    Spectral(String),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum PresentationError {
    #[error("invalid presentation specification: {0}")]
    InvalidPresentationSpec(String),
    #[error("invalid exposure: {0}")]
    InvalidExposure(String),
    #[error("non-finite source color: {0}")]
    NonFiniteSourceColor(String),
    #[error("non-finite presentation result: {0}")]
    NonFinitePresentationResult(String),
    #[error("presentation gamut failure: {0}")]
    PresentationGamutFailure(String),
    #[error("tone-map domain error: {0}")]
    ToneMapDomainError(String),
    #[error("tone-map range failure: {0}")]
    ToneMapRangeFailure(String),
    #[error("display encoding failure: {0}")]
    DisplayEncodingFailure(String),
    #[error("quantization failure: {0}")]
    QuantizationFailure(String),
    #[error("frame length mismatch")]
    FrameLengthMismatch,
    #[error("unsupported operator `{0}`")]
    UnsupportedOperator(String),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum AppearanceError {
    #[error("invalid appearance specification: {0}")]
    InvalidSpec(String),
    #[error("frame length mismatch")]
    FrameLengthMismatch,
    #[error("grid dimension mismatch")]
    GridMismatch,
    #[error("scene outcome parity failure at ({col},{row}): {detail}")]
    SceneOutcomeParity { col: u32, row: u32, detail: String },
    #[error("scene numerical failure: affine_limit={affine_limit} failed={failed}")]
    SceneNumericalFailure { affine_limit: u64, failed: u64 },
    #[error("non-finite appearance value: {0}")]
    NonFinite(String),
    #[error("appearance mapping failed at ({col},{row}): {cause}")]
    PixelMappingFailed { col: u32, row: u32, cause: String },
    #[error("colorimetry error: {0}")]
    Colorimetry(String),
    #[error("presentation error: {0}")]
    Presentation(String),
    #[error("emission error: {0}")]
    Emission(String),
}
