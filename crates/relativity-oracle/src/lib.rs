//! Stable CPU `f64` oracle frame schema and comparison metrics.
//!
//! This crate is pure in-process data assembly over accepted trace/render
//! scientific frames. It does not trace, render to files, measure time, spawn
//! subprocesses, or depend on any frontend/GPU crate.

#![forbid(unsafe_code)]

use relativity_render::{
    DiskBolometricFrame, DiskBolometricPixel, DiskFrequencyShiftFrame, DiskFrequencyShiftPixel,
};
use relativity_trace::{
    pixel_index, sensor_at_pixel_center, CelestialCoordinateFrame, CelestialCoordinatePixel,
    OutcomeClass, RayOutcome, TraceBundle, TraceGrid, TraceSurfaceSet,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const ORACLE_SCHEMA_VERSION: u32 = 1;
pub const ORACLE_ID_V1: &str = "cpu-f64-relativistic-oracle-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OracleChannelSet {
    GeometryCelestial,
    FullBolometricDisk,
}

impl OracleChannelSet {
    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::GeometryCelestial => "oracle-channel-set:geometry-celestial",
            Self::FullBolometricDisk => "oracle-channel-set:full-bolometric-disk",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SensorWindow {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl SensorWindow {
    pub const fn full_frame() -> Self {
        Self {
            x_min: -1.0,
            x_max: 1.0,
            y_min: -1.0,
            y_max: 1.0,
        }
    }

    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Result<Self, OracleError> {
        let out = Self {
            x_min,
            x_max,
            y_min,
            y_max,
        };
        out.validate()?;
        Ok(out)
    }

    pub fn validate(self) -> Result<(), OracleError> {
        let all = [self.x_min, self.x_max, self.y_min, self.y_max];
        if !all.iter().all(|v| v.is_finite()) {
            return Err(OracleError::InvalidSensorWindow("non-finite bound"));
        }
        if !(self.x_min < self.x_max) {
            return Err(OracleError::InvalidSensorWindow(
                "x_min must be less than x_max",
            ));
        }
        if !(self.y_min < self.y_max) {
            return Err(OracleError::InvalidSensorWindow(
                "y_min must be less than y_max",
            ));
        }
        if all.iter().any(|v| *v < -1.0 || *v > 1.0) {
            return Err(OracleError::InvalidSensorWindow(
                "bounds must lie inside [-1,1]",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleCelestialSample {
    pub boundary_oblate_radius: f64,
    pub theta: f64,
    pub psi: f64,
    pub unit_coordinate_direction: [f64; 3],
    pub u: f64,
    pub v: f64,
    pub escape_event_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleDiskSample {
    pub radius: f64,
    pub azimuth: f64,
    pub g_factor: f64,
    pub log2_g: f64,
    pub g_fourth: f64,
    pub emitted_bolometric_intensity: f64,
    pub observed_bolometric_intensity: f64,
    pub disk_event_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OraclePixel {
    pub local_index: u64,
    pub col: u32,
    pub row: u32,
    pub source_index: u64,
    pub source_col: u32,
    pub source_row: u32,
    pub sensor_x: f64,
    pub sensor_y: f64,
    pub outcome_class: OutcomeClass,
    pub rhs_evaluations: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub celestial: Option<OracleCelestialSample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<OracleDiskSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleSourceDigests {
    pub numerical_profile_digest: String,
    pub trace_data_digest: String,
    pub outcome_class_digest: String,
    pub celestial_coordinate_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frequency_shift_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bolometric_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleScientificClaim {
    pub trace_backend: String,
    pub celestial_direction_claim: String,
    pub disk_frequency_claim: String,
    pub bolometric_claim: String,
    pub spectral_status: String,
    pub physical_rgb_status: String,
}

impl OracleScientificClaim {
    pub fn v1() -> Self {
        Self {
            trace_backend: "cpu-f64-dop853-cartesian-kerr-schild".into(),
            celestial_direction_claim: "finite-oblate-escape-boundary-coordinate-not-null-infinity"
                .into(),
            disk_frequency_claim: "backward-covector-circular-equatorial-emitter-g-factor".into(),
            bolometric_claim:
                "diagnostic-arbitrary-unit-specific-intensity-with-g-fourth-transport".into(),
            spectral_status: "not-implemented".into(),
            physical_rgb_status: "not-implemented".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OracleFrame {
    pub schema_version: u32,
    pub oracle_id: String,
    pub width: u32,
    pub height: u32,
    pub sensor_window: SensorWindow,
    pub surface_set: TraceSurfaceSet,
    pub channel_set: OracleChannelSet,
    pub scientific_claim: OracleScientificClaim,
    pub source_digests: OracleSourceDigests,
    pub pixels: Vec<OraclePixel>,
    pub scientific_digest: String,
}

impl<'de> Deserialize<'de> for OracleFrame {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            schema_version: u32,
            oracle_id: String,
            width: u32,
            height: u32,
            sensor_window: SensorWindow,
            surface_set: TraceSurfaceSet,
            channel_set: OracleChannelSet,
            scientific_claim: OracleScientificClaim,
            source_digests: OracleSourceDigests,
            pixels: Vec<OraclePixel>,
            scientific_digest: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        let frame = OracleFrame {
            schema_version: raw.schema_version,
            oracle_id: raw.oracle_id,
            width: raw.width,
            height: raw.height,
            sensor_window: raw.sensor_window,
            surface_set: raw.surface_set,
            channel_set: raw.channel_set,
            scientific_claim: raw.scientific_claim,
            source_digests: raw.source_digests,
            pixels: raw.pixels,
            scientific_digest: raw.scientific_digest,
        };
        frame.validate().map_err(serde::de::Error::custom)?;
        Ok(frame)
    }
}

impl OracleFrame {
    /// Full public invariant. Never clamps; rejects malformed frames.
    pub fn validate(&self) -> Result<(), OracleError> {
        if self.schema_version != ORACLE_SCHEMA_VERSION {
            return Err(OracleError::InvalidFrame(format!(
                "schema_version {} != {ORACLE_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.oracle_id != ORACLE_ID_V1 {
            return Err(OracleError::InvalidFrame(format!(
                "oracle_id `{}` != `{ORACLE_ID_V1}`",
                self.oracle_id
            )));
        }
        if self.width == 0 || self.height == 0 {
            return Err(OracleError::InvalidFrame(
                "width and height must be > 0".into(),
            ));
        }
        let expected_len = (self.width as usize)
            .checked_mul(self.height as usize)
            .ok_or_else(|| OracleError::InvalidFrame("dimension overflow".into()))?;
        if self.pixels.len() != expected_len {
            return Err(OracleError::LengthMismatch {
                frame: "oracle",
                len: self.pixels.len(),
                expected: expected_len,
            });
        }
        self.sensor_window.validate()?;
        if self.scientific_claim != OracleScientificClaim::v1() {
            return Err(OracleError::InvalidFrame(
                "scientific_claim must equal OracleScientificClaim::v1()".into(),
            ));
        }
        validate_stored_source_digests(&self.source_digests, self.channel_set)?;
        if self.channel_set == OracleChannelSet::FullBolometricDisk
            && self.surface_set != TraceSurfaceSet::OpaqueDiskHorizonEscape
        {
            return Err(OracleError::InvalidSurfaceSet {
                channel_set: self.channel_set,
                surface_set: self.surface_set,
            });
        }
        for row in 0..self.height {
            for col in 0..self.width {
                let idx = pixel_index(
                    TraceGrid {
                        width: self.width,
                        height: self.height,
                    },
                    col,
                    row,
                );
                let pixel = &self.pixels[idx];
                let expected_local = u64::from(row) * u64::from(self.width) + u64::from(col);
                if pixel.local_index != expected_local || pixel.col != col || pixel.row != row {
                    return Err(OracleError::InvalidFrame(format!(
                        "row-major index mismatch at ({col},{row}): local_index={} col={} row={}",
                        pixel.local_index, pixel.col, pixel.row
                    )));
                }
                if !pixel.sensor_x.is_finite()
                    || !pixel.sensor_y.is_finite()
                    || pixel.sensor_x < self.sensor_window.x_min
                    || pixel.sensor_x > self.sensor_window.x_max
                    || pixel.sensor_y < self.sensor_window.y_min
                    || pixel.sensor_y > self.sensor_window.y_max
                {
                    return Err(OracleError::InvalidFrame(format!(
                        "sensor coordinates outside window at ({col},{row})"
                    )));
                }
                validate_outcome_channel_consistency(pixel, self.channel_set)?;
                validate_pixel_finite(pixel)?;
            }
        }
        let recomputed = oracle_scientific_digest(self);
        if self.scientific_digest != recomputed {
            return Err(OracleError::ScientificDigestMismatch);
        }
        Ok(())
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> Result<&OraclePixel, OracleError> {
        if col >= self.width || row >= self.height {
            return Err(OracleError::PixelIndexOutOfRange { index: usize::MAX });
        }
        let idx = pixel_index(
            TraceGrid {
                width: self.width,
                height: self.height,
            },
            col,
            row,
        );
        self.pixels
            .get(idx)
            .ok_or(OracleError::PixelIndexOutOfRange { index: idx })
    }
}

pub struct OracleFrameInputs<'a> {
    pub trace: &'a TraceBundle,
    pub celestial: &'a CelestialCoordinateFrame,
    pub frequency: Option<&'a DiskFrequencyShiftFrame>,
    pub bolometric: Option<&'a DiskBolometricFrame>,
    pub sensor_window: SensorWindow,
    pub surface_set: TraceSurfaceSet,
    pub channel_set: OracleChannelSet,
    pub source_digests: OracleSourceDigests,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum OracleError {
    #[error("invalid sensor window: {0}")]
    InvalidSensorWindow(&'static str),
    #[error("frame dimension mismatch: {frame} has {width}x{height}, expected {expected_width}x{expected_height}")]
    DimensionMismatch {
        frame: &'static str,
        width: u32,
        height: u32,
        expected_width: u32,
        expected_height: u32,
    },
    #[error("frame length mismatch: {frame} has {len}, expected {expected}")]
    LengthMismatch {
        frame: &'static str,
        len: usize,
        expected: usize,
    },
    #[error("missing required frame: {0}")]
    MissingFrame(&'static str),
    #[error("missing required source digest: {0}")]
    MissingSourceDigest(&'static str),
    #[error("invalid surface set for channel set {channel_set:?}: {surface_set:?}")]
    InvalidSurfaceSet {
        channel_set: OracleChannelSet,
        surface_set: TraceSurfaceSet,
    },
    #[error("pixel ({col},{row}) mismatch: {reason}")]
    PixelMismatch { col: u32, row: u32, reason: String },
    #[error("non-finite exported value at pixel ({col},{row}): {field}")]
    NonFiniteValue {
        col: u32,
        row: u32,
        field: &'static str,
    },
    #[error("invalid positive scientific value at pixel ({col},{row}): {field}={value}")]
    InvalidPositiveValue {
        col: u32,
        row: u32,
        field: &'static str,
        value: f64,
    },
    #[error("invalid crop: {0}")]
    InvalidCrop(&'static str),
    #[error("comparison requires identical layouts: {0}")]
    IncompatibleComparison(&'static str),
    #[error("invalid oracle frame: {0}")]
    InvalidFrame(String),
    #[error("stored scientific digest does not match recomputed digest")]
    ScientificDigestMismatch,
    #[error("oracle pixel index out of range: {index}")]
    PixelIndexOutOfRange { index: usize },
}

pub fn build_oracle_frame(inputs: OracleFrameInputs<'_>) -> Result<OracleFrame, OracleError> {
    inputs.sensor_window.validate()?;
    let grid = inputs.trace.grid;
    let expected_len = grid.pixel_count();
    validate_len("trace", inputs.trace.outcomes.len(), expected_len)?;
    validate_grid("celestial", inputs.celestial.grid(), grid)?;
    validate_len("celestial", inputs.celestial.pixels().len(), expected_len)?;
    validate_channel_inputs(&inputs)?;

    if let Some(frequency) = inputs.frequency {
        validate_grid("frequency", frequency.grid(), grid)?;
        validate_len("frequency", frequency.pixels().len(), expected_len)?;
    }
    if let Some(bolometric) = inputs.bolometric {
        validate_grid("bolometric", bolometric.grid(), grid)?;
        validate_len("bolometric", bolometric.pixels().len(), expected_len)?;
    }

    let mut pixels = Vec::with_capacity(expected_len);
    for row in 0..grid.height {
        for col in 0..grid.width {
            let local_index = pixel_index(grid, col, row) as u64;
            let trace = inputs.trace.outcome_at(col, row);
            let outcome_class = trace.class();
            let sensor = sensor_at_pixel_center(grid, col, row);
            let celestial =
                assemble_celestial(col, row, outcome_class, inputs.celestial.pixel_at(col, row))?;
            let disk = assemble_disk(
                col,
                row,
                outcome_class,
                inputs.channel_set,
                inputs.frequency.map(|f| f.pixel_at(col, row)),
                inputs.bolometric.map(|b| b.pixel_at(col, row)),
            )?;
            let pixel = OraclePixel {
                local_index,
                col,
                row,
                source_index: local_index,
                source_col: col,
                source_row: row,
                sensor_x: sensor.x,
                sensor_y: sensor.y,
                outcome_class,
                rhs_evaluations: trace.rhs_evaluations(),
                failure_class: failure_class(trace),
                celestial,
                disk,
            };
            validate_pixel_finite(&pixel)?;
            pixels.push(pixel);
        }
    }

    let mut frame = OracleFrame {
        schema_version: ORACLE_SCHEMA_VERSION,
        oracle_id: ORACLE_ID_V1.into(),
        width: grid.width,
        height: grid.height,
        sensor_window: inputs.sensor_window,
        surface_set: inputs.surface_set,
        channel_set: inputs.channel_set,
        scientific_claim: OracleScientificClaim::v1(),
        source_digests: inputs.source_digests,
        pixels,
        scientific_digest: String::new(),
    };
    frame.scientific_digest = oracle_scientific_digest(&frame);
    frame.validate()?;
    Ok(frame)
}

fn validate_stored_source_digests(
    digests: &OracleSourceDigests,
    channel_set: OracleChannelSet,
) -> Result<(), OracleError> {
    if digests.numerical_profile_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest("numerical_profile_digest"));
    }
    if digests.trace_data_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest("trace_data_digest"));
    }
    if digests.outcome_class_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest("outcome_class_digest"));
    }
    if digests.celestial_coordinate_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest(
            "celestial_coordinate_digest",
        ));
    }
    match channel_set {
        OracleChannelSet::FullBolometricDisk => {
            if digests
                .frequency_shift_digest
                .as_ref()
                .is_none_or(|s| s.is_empty())
            {
                return Err(OracleError::MissingSourceDigest("frequency_shift_digest"));
            }
            if digests
                .bolometric_digest
                .as_ref()
                .is_none_or(|s| s.is_empty())
            {
                return Err(OracleError::MissingSourceDigest("bolometric_digest"));
            }
        }
        OracleChannelSet::GeometryCelestial => {
            if digests.frequency_shift_digest.is_some() || digests.bolometric_digest.is_some() {
                return Err(OracleError::InvalidFrame(
                    "geometry-celestial frames must omit frequency/bolometric source digests"
                        .into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_outcome_channel_consistency(
    pixel: &OraclePixel,
    channel_set: OracleChannelSet,
) -> Result<(), OracleError> {
    match pixel.outcome_class {
        OutcomeClass::Escaped => {
            if pixel.celestial.is_none() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "escaped pixel missing celestial sample".into(),
                ));
            }
            if pixel.disk.is_some() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "escaped pixel must not carry disk sample".into(),
                ));
            }
            if pixel.failure_class.is_some() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "escaped pixel must not carry failure_class".into(),
                ));
            }
        }
        OutcomeClass::DiskHit => {
            if pixel.celestial.is_some() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "disk-hit pixel must not carry celestial sample".into(),
                ));
            }
            match channel_set {
                OracleChannelSet::FullBolometricDisk => {
                    if pixel.disk.is_none() {
                        return Err(pixel_mismatch(
                            pixel.col,
                            pixel.row,
                            "full-bolometric disk-hit missing disk sample".into(),
                        ));
                    }
                }
                OracleChannelSet::GeometryCelestial => {
                    if pixel.disk.is_some() {
                        return Err(pixel_mismatch(
                            pixel.col,
                            pixel.row,
                            "geometry-celestial disk-hit must not carry disk sample".into(),
                        ));
                    }
                }
            }
            if pixel.failure_class.is_some() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "disk-hit pixel must not carry failure_class".into(),
                ));
            }
        }
        OutcomeClass::Failed => {
            if pixel.celestial.is_some() || pixel.disk.is_some() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "failed pixel must not carry celestial/disk samples".into(),
                ));
            }
            if pixel.failure_class.is_none() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "failed pixel missing failure_class".into(),
                ));
            }
        }
        OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach | OutcomeClass::AffineLimit => {
            if pixel.celestial.is_some() || pixel.disk.is_some() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "non-escaped/non-disk pixel must not carry celestial/disk samples".into(),
                ));
            }
            if pixel.failure_class.is_some() {
                return Err(pixel_mismatch(
                    pixel.col,
                    pixel.row,
                    "non-failed pixel must not carry failure_class".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_len(frame: &'static str, len: usize, expected: usize) -> Result<(), OracleError> {
    if len == expected {
        Ok(())
    } else {
        Err(OracleError::LengthMismatch {
            frame,
            len,
            expected,
        })
    }
}

fn validate_grid(
    frame: &'static str,
    found: TraceGrid,
    expected: TraceGrid,
) -> Result<(), OracleError> {
    if found == expected {
        Ok(())
    } else {
        Err(OracleError::DimensionMismatch {
            frame,
            width: found.width,
            height: found.height,
            expected_width: expected.width,
            expected_height: expected.height,
        })
    }
}

fn validate_channel_inputs(inputs: &OracleFrameInputs<'_>) -> Result<(), OracleError> {
    if inputs.source_digests.numerical_profile_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest("numerical_profile_digest"));
    }
    if inputs.source_digests.trace_data_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest("trace_data_digest"));
    }
    if inputs.source_digests.outcome_class_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest("outcome_class_digest"));
    }
    if inputs.source_digests.celestial_coordinate_digest.is_empty() {
        return Err(OracleError::MissingSourceDigest(
            "celestial_coordinate_digest",
        ));
    }
    if inputs.channel_set == OracleChannelSet::FullBolometricDisk {
        if inputs.surface_set != TraceSurfaceSet::OpaqueDiskHorizonEscape {
            return Err(OracleError::InvalidSurfaceSet {
                channel_set: inputs.channel_set,
                surface_set: inputs.surface_set,
            });
        }
        if inputs.frequency.is_none() {
            return Err(OracleError::MissingFrame("frequency"));
        }
        if inputs.bolometric.is_none() {
            return Err(OracleError::MissingFrame("bolometric"));
        }
        if inputs.source_digests.frequency_shift_digest.is_none() {
            return Err(OracleError::MissingSourceDigest("frequency_shift_digest"));
        }
        if inputs.source_digests.bolometric_digest.is_none() {
            return Err(OracleError::MissingSourceDigest("bolometric_digest"));
        }
    }
    Ok(())
}

fn failure_class(trace: &RayOutcome) -> Option<String> {
    match trace {
        RayOutcome::Failed(f) => Some(f.class_name().into()),
        _ => None,
    }
}

fn assemble_celestial(
    col: u32,
    row: u32,
    outcome_class: OutcomeClass,
    pixel: &CelestialCoordinatePixel,
) -> Result<Option<OracleCelestialSample>, OracleError> {
    match (outcome_class, pixel) {
        (OutcomeClass::Escaped, CelestialCoordinatePixel::Escaped(s)) => {
            Ok(Some(OracleCelestialSample {
                boundary_oblate_radius: s.oblate_radius,
                theta: s.theta,
                psi: s.psi,
                unit_coordinate_direction: s.unit_coordinate_direction,
                u: s.uv.u,
                v: s.uv.v,
                escape_event_value: s.escape_event_value,
            }))
        }
        (OutcomeClass::Escaped, CelestialCoordinatePixel::NotEscaped { outcome_class }) => {
            Err(pixel_mismatch(
                col,
                row,
                format!(
                    "escaped trace has no celestial sample; celestial outcome is {outcome_class:?}"
                ),
            ))
        }
        (_, CelestialCoordinatePixel::Escaped(_)) => Err(pixel_mismatch(
            col,
            row,
            "non-escaped trace received celestial sample".into(),
        )),
        (trace_class, CelestialCoordinatePixel::NotEscaped { outcome_class })
            if trace_class != *outcome_class =>
        {
            Err(pixel_mismatch(
                col,
                row,
                format!("celestial outcome {outcome_class:?} disagrees with trace {trace_class:?}"),
            ))
        }
        _ => Ok(None),
    }
}

fn assemble_disk(
    col: u32,
    row: u32,
    outcome_class: OutcomeClass,
    channel_set: OracleChannelSet,
    frequency: Option<&DiskFrequencyShiftPixel>,
    bolometric: Option<&DiskBolometricPixel>,
) -> Result<Option<OracleDiskSample>, OracleError> {
    if channel_set == OracleChannelSet::GeometryCelestial {
        if matches!(outcome_class, OutcomeClass::DiskHit) {
            return Ok(None);
        }
        return Ok(None);
    }
    match (outcome_class, frequency, bolometric) {
        (
            OutcomeClass::DiskHit,
            Some(DiskFrequencyShiftPixel::DiskHit(f)),
            Some(DiskBolometricPixel::DiskHit(b)),
        ) => {
            if f.g_factor.to_bits() != b.g_factor.to_bits() {
                return Err(pixel_mismatch(
                    col,
                    row,
                    "frequency and bolometric g_factor differ bit-for-bit".into(),
                ));
            }
            Ok(Some(OracleDiskSample {
                radius: f.radius,
                azimuth: f.azimuth,
                g_factor: f.g_factor,
                log2_g: f.log2_g,
                g_fourth: b.g_fourth,
                emitted_bolometric_intensity: b.emitted_bolometric_intensity,
                observed_bolometric_intensity: b.observed_bolometric_intensity,
                disk_event_value: f.disk_event_value,
            }))
        }
        (OutcomeClass::DiskHit, _, _) => Err(pixel_mismatch(
            col,
            row,
            "disk hit lacks required frequency or bolometric sample".into(),
        )),
        (trace_class, Some(DiskFrequencyShiftPixel::DiskHit(_)), _) => Err(pixel_mismatch(
            col,
            row,
            format!("non-disk trace {trace_class:?} received frequency sample"),
        )),
        (trace_class, _, Some(DiskBolometricPixel::DiskHit(_))) => Err(pixel_mismatch(
            col,
            row,
            format!("non-disk trace {trace_class:?} received bolometric sample"),
        )),
        (trace_class, Some(DiskFrequencyShiftPixel::NotDiskHit { outcome_class }), _)
            if trace_class != *outcome_class =>
        {
            Err(pixel_mismatch(
                col,
                row,
                format!("frequency outcome {outcome_class:?} disagrees with trace {trace_class:?}"),
            ))
        }
        (trace_class, _, Some(DiskBolometricPixel::NotDiskHit { outcome_class }))
            if trace_class != *outcome_class =>
        {
            Err(pixel_mismatch(
                col,
                row,
                format!(
                    "bolometric outcome {outcome_class:?} disagrees with trace {trace_class:?}"
                ),
            ))
        }
        _ => Ok(None),
    }
}

fn pixel_mismatch(col: u32, row: u32, reason: String) -> OracleError {
    OracleError::PixelMismatch { col, row, reason }
}

fn validate_pixel_finite(pixel: &OraclePixel) -> Result<(), OracleError> {
    finite(pixel.col, pixel.row, "sensor_x", pixel.sensor_x)?;
    finite(pixel.col, pixel.row, "sensor_y", pixel.sensor_y)?;
    if let Some(c) = &pixel.celestial {
        finite(
            pixel.col,
            pixel.row,
            "boundary_oblate_radius",
            c.boundary_oblate_radius,
        )?;
        finite(pixel.col, pixel.row, "theta", c.theta)?;
        finite(pixel.col, pixel.row, "psi", c.psi)?;
        for value in c.unit_coordinate_direction {
            finite(pixel.col, pixel.row, "unit_coordinate_direction", value)?;
        }
        finite(pixel.col, pixel.row, "u", c.u)?;
        finite(pixel.col, pixel.row, "v", c.v)?;
        finite(
            pixel.col,
            pixel.row,
            "escape_event_value",
            c.escape_event_value,
        )?;
    }
    if let Some(d) = &pixel.disk {
        finite(pixel.col, pixel.row, "disk.radius", d.radius)?;
        finite(pixel.col, pixel.row, "disk.azimuth", d.azimuth)?;
        positive(pixel.col, pixel.row, "disk.g_factor", d.g_factor)?;
        finite(pixel.col, pixel.row, "disk.log2_g", d.log2_g)?;
        positive(pixel.col, pixel.row, "disk.g_fourth", d.g_fourth)?;
        positive(
            pixel.col,
            pixel.row,
            "disk.emitted_bolometric_intensity",
            d.emitted_bolometric_intensity,
        )?;
        positive(
            pixel.col,
            pixel.row,
            "disk.observed_bolometric_intensity",
            d.observed_bolometric_intensity,
        )?;
        finite(
            pixel.col,
            pixel.row,
            "disk.disk_event_value",
            d.disk_event_value,
        )?;
    }
    Ok(())
}

fn finite(col: u32, row: u32, field: &'static str, value: f64) -> Result<(), OracleError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(OracleError::NonFiniteValue { col, row, field })
    }
}

fn positive(col: u32, row: u32, field: &'static str, value: f64) -> Result<(), OracleError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(OracleError::InvalidPositiveValue {
            col,
            row,
            field,
            value,
        })
    }
}

pub fn oracle_scientific_digest(frame: &OracleFrame) -> String {
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"oracle-frame-v1-scientific-digest");
    h.update(frame.schema_version.to_le_bytes());
    update_tagged_str(&mut h, b"oracle-id", &frame.oracle_id);
    hash_claim(&mut h, &frame.scientific_claim);
    h.update(frame.width.to_le_bytes());
    h.update(frame.height.to_le_bytes());
    hash_window(&mut h, frame.sensor_window);
    update_tagged_str(&mut h, b"surface-set", frame.surface_set.digest_tag());
    update_tagged_str(&mut h, b"channel-set", frame.channel_set.digest_tag());
    hash_source_digests(&mut h, &frame.source_digests);
    update_tagged_bytes(&mut h, b"domain", b"pixels-row-major");
    for pixel in &frame.pixels {
        hash_pixel(&mut h, pixel);
    }
    hex_sha(&h.finalize())
}

fn hash_claim(h: &mut Sha256, claim: &OracleScientificClaim) {
    update_tagged_str(h, b"trace-backend", &claim.trace_backend);
    update_tagged_str(
        h,
        b"celestial-direction-claim",
        &claim.celestial_direction_claim,
    );
    update_tagged_str(h, b"disk-frequency-claim", &claim.disk_frequency_claim);
    update_tagged_str(h, b"bolometric-claim", &claim.bolometric_claim);
    update_tagged_str(h, b"spectral-status", &claim.spectral_status);
    update_tagged_str(h, b"physical-rgb-status", &claim.physical_rgb_status);
}

fn hash_window(h: &mut Sha256, window: SensorWindow) {
    h.update(window.x_min.to_bits().to_le_bytes());
    h.update(window.x_max.to_bits().to_le_bytes());
    h.update(window.y_min.to_bits().to_le_bytes());
    h.update(window.y_max.to_bits().to_le_bytes());
}

fn hash_source_digests(h: &mut Sha256, digests: &OracleSourceDigests) {
    update_tagged_str(
        h,
        b"numerical-profile-digest",
        &digests.numerical_profile_digest,
    );
    update_tagged_str(h, b"trace-data-digest", &digests.trace_data_digest);
    update_tagged_str(h, b"outcome-class-digest", &digests.outcome_class_digest);
    update_tagged_str(
        h,
        b"celestial-coordinate-digest",
        &digests.celestial_coordinate_digest,
    );
    hash_optional_str(
        h,
        b"frequency-shift-digest",
        digests.frequency_shift_digest.as_deref(),
    );
    hash_optional_str(
        h,
        b"bolometric-digest",
        digests.bolometric_digest.as_deref(),
    );
}

fn hash_optional_str(h: &mut Sha256, tag: &[u8], value: Option<&str>) {
    match value {
        Some(value) => {
            h.update([1]);
            update_tagged_str(h, tag, value);
        }
        None => h.update([0]),
    }
}

fn hash_pixel(h: &mut Sha256, pixel: &OraclePixel) {
    h.update(pixel.local_index.to_le_bytes());
    h.update(pixel.col.to_le_bytes());
    h.update(pixel.row.to_le_bytes());
    h.update(pixel.source_index.to_le_bytes());
    h.update(pixel.source_col.to_le_bytes());
    h.update(pixel.source_row.to_le_bytes());
    h.update(pixel.sensor_x.to_bits().to_le_bytes());
    h.update(pixel.sensor_y.to_bits().to_le_bytes());
    update_tagged_str(h, b"outcome-class", pixel.outcome_class.digest_tag());
    h.update(pixel.rhs_evaluations.to_le_bytes());
    hash_optional_str(h, b"failure-class", pixel.failure_class.as_deref());
    match &pixel.celestial {
        Some(c) => {
            h.update([1]);
            h.update(c.boundary_oblate_radius.to_bits().to_le_bytes());
            h.update(c.theta.to_bits().to_le_bytes());
            h.update(c.psi.to_bits().to_le_bytes());
            for value in c.unit_coordinate_direction {
                h.update(value.to_bits().to_le_bytes());
            }
            h.update(c.u.to_bits().to_le_bytes());
            h.update(c.v.to_bits().to_le_bytes());
            h.update(c.escape_event_value.to_bits().to_le_bytes());
        }
        None => h.update([0]),
    }
    match &pixel.disk {
        Some(d) => {
            h.update([1]);
            h.update(d.radius.to_bits().to_le_bytes());
            h.update(d.azimuth.to_bits().to_le_bytes());
            h.update(d.g_factor.to_bits().to_le_bytes());
            h.update(d.log2_g.to_bits().to_le_bytes());
            h.update(d.g_fourth.to_bits().to_le_bytes());
            h.update(d.emitted_bolometric_intensity.to_bits().to_le_bytes());
            h.update(d.observed_bolometric_intensity.to_bits().to_le_bytes());
            h.update(d.disk_event_value.to_bits().to_le_bytes());
        }
        None => h.update([0]),
    }
}

fn update_tagged_str(h: &mut Sha256, tag: &[u8], value: &str) {
    update_tagged_bytes(h, tag, value.as_bytes());
}

fn update_tagged_bytes(h: &mut Sha256, tag: &[u8], value: &[u8]) {
    h.update((tag.len() as u64).to_le_bytes());
    h.update(tag);
    h.update((value.len() as u64).to_le_bytes());
    h.update(value);
}

fn hex_sha(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelCrop {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

pub fn crop_oracle_frame(
    source: &OracleFrame,
    crop: PixelCrop,
) -> Result<OracleFrame, OracleError> {
    source.validate()?;
    if crop.width == 0 || crop.height == 0 {
        return Err(OracleError::InvalidCrop("crop must be non-empty"));
    }
    if crop.left > source.width
        || crop.top > source.height
        || crop.width > source.width.saturating_sub(crop.left)
        || crop.height > source.height.saturating_sub(crop.top)
    {
        return Err(OracleError::InvalidCrop("crop must lie inside source"));
    }
    let sx0 = source.sensor_window.x_min;
    let sx1 = source.sensor_window.x_max;
    let sy0 = source.sensor_window.y_min;
    let sy1 = source.sensor_window.y_max;
    let sensor_window = SensorWindow::new(
        sx0 + (sx1 - sx0) * f64::from(crop.left) / f64::from(source.width),
        sx0 + (sx1 - sx0) * f64::from(crop.left + crop.width) / f64::from(source.width),
        sy1 - (sy1 - sy0) * f64::from(crop.top + crop.height) / f64::from(source.height),
        sy1 - (sy1 - sy0) * f64::from(crop.top) / f64::from(source.height),
    )?;
    let mut pixels = Vec::with_capacity((crop.width as usize) * (crop.height as usize));
    for row in 0..crop.height {
        for col in 0..crop.width {
            let source_col = crop.left + col;
            let source_row = crop.top + row;
            let mut pixel = source.pixel_at(source_col, source_row)?.clone();
            let local_index = u64::from(row) * u64::from(crop.width) + u64::from(col);
            pixel.local_index = local_index;
            pixel.col = col;
            pixel.row = row;
            pixels.push(pixel);
        }
    }
    let mut frame = OracleFrame {
        schema_version: source.schema_version,
        oracle_id: source.oracle_id.clone(),
        width: crop.width,
        height: crop.height,
        sensor_window,
        surface_set: source.surface_set,
        channel_set: source.channel_set,
        scientific_claim: source.scientific_claim.clone(),
        source_digests: source.source_digests.clone(),
        pixels,
        scientific_digest: String::new(),
    };
    frame.scientific_digest = oracle_scientific_digest(&frame);
    frame.validate()?;
    Ok(frame)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntegerErrorMetrics {
    pub mae: f64,
    pub rmse: f64,
    pub maximum_absolute_error: u64,
    pub maximum_error_index: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarErrorMetrics {
    pub mae: f64,
    pub rmse: f64,
    pub maximum_absolute_error: f64,
    pub maximum_error_index: u64,
}

pub type OptionalScalarErrorMetrics = Option<ScalarErrorMetrics>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleComparisonMetrics {
    pub compared_pixels: u64,
    pub outcome_disagreement_count: u64,
    pub outcome_disagreement_rate: f64,
    pub rhs_absolute_error: IntegerErrorMetrics,
    pub celestial_pair_count: u64,
    pub celestial_presence_mismatch_count: u64,
    pub celestial_angular_error_radians: OptionalScalarErrorMetrics,
    pub celestial_wrap_u_error: OptionalScalarErrorMetrics,
    pub celestial_v_error: OptionalScalarErrorMetrics,
    pub disk_pair_count: u64,
    pub disk_presence_mismatch_count: u64,
    pub log2_g_error: OptionalScalarErrorMetrics,
    pub log2_emitted_error: OptionalScalarErrorMetrics,
    pub log2_observed_error: OptionalScalarErrorMetrics,
}

pub fn compare_oracle_frames(
    reference: &OracleFrame,
    candidate: &OracleFrame,
) -> Result<OracleComparisonMetrics, OracleError> {
    reference.validate()?;
    candidate.validate()?;
    validate_comparison_layout(reference, candidate)?;
    let mut outcome_disagreement_count = 0;
    let mut rhs_errors = ErrorAccumulator::new();
    let mut celestial_presence_mismatch_count = 0;
    let mut celestial_angle = ErrorAccumulator::new();
    let mut celestial_u = ErrorAccumulator::new();
    let mut celestial_v = ErrorAccumulator::new();
    let mut celestial_pair_count = 0;
    let mut disk_presence_mismatch_count = 0;
    let mut log2_g = ErrorAccumulator::new();
    let mut log2_emitted = ErrorAccumulator::new();
    let mut log2_observed = ErrorAccumulator::new();
    let mut disk_pair_count = 0;

    for (idx, (r, c)) in reference.pixels.iter().zip(&candidate.pixels).enumerate() {
        let idx = idx as u64;
        let outcomes_compatible = r.outcome_class == c.outcome_class;
        if !outcomes_compatible {
            outcome_disagreement_count += 1;
        }
        rhs_errors.push(idx, r.rhs_evaluations.abs_diff(c.rhs_evaluations) as f64);

        match (&r.celestial, &c.celestial) {
            (Some(a), Some(b)) => {
                if outcomes_compatible {
                    celestial_pair_count += 1;
                    let dot = a.unit_coordinate_direction[0] * b.unit_coordinate_direction[0]
                        + a.unit_coordinate_direction[1] * b.unit_coordinate_direction[1]
                        + a.unit_coordinate_direction[2] * b.unit_coordinate_direction[2];
                    celestial_angle.push(idx, dot.clamp(-1.0, 1.0).acos());
                    let du_raw = (a.u - b.u).abs();
                    celestial_u.push(idx, du_raw.min(1.0 - du_raw));
                    celestial_v.push(idx, (a.v - b.v).abs());
                }
            }
            (None, None) => {}
            _ => celestial_presence_mismatch_count += 1,
        }

        match (&r.disk, &c.disk) {
            (Some(a), Some(b)) => {
                if outcomes_compatible {
                    disk_pair_count += 1;
                    log2_g.push(idx, (b.g_factor.log2() - a.g_factor.log2()).abs());
                    log2_emitted.push(
                        idx,
                        (b.emitted_bolometric_intensity.log2()
                            - a.emitted_bolometric_intensity.log2())
                        .abs(),
                    );
                    log2_observed.push(
                        idx,
                        (b.observed_bolometric_intensity.log2()
                            - a.observed_bolometric_intensity.log2())
                        .abs(),
                    );
                }
            }
            (None, None) => {}
            _ => disk_presence_mismatch_count += 1,
        }
    }

    let compared_pixels = reference.pixels.len() as u64;
    Ok(OracleComparisonMetrics {
        compared_pixels,
        outcome_disagreement_count,
        outcome_disagreement_rate: outcome_disagreement_count as f64 / compared_pixels as f64,
        rhs_absolute_error: rhs_errors.into_integer(),
        celestial_pair_count,
        celestial_presence_mismatch_count,
        celestial_angular_error_radians: celestial_angle.into_scalar(),
        celestial_wrap_u_error: celestial_u.into_scalar(),
        celestial_v_error: celestial_v.into_scalar(),
        disk_pair_count,
        disk_presence_mismatch_count,
        log2_g_error: log2_g.into_scalar(),
        log2_emitted_error: log2_emitted.into_scalar(),
        log2_observed_error: log2_observed.into_scalar(),
    })
}

fn validate_comparison_layout(
    reference: &OracleFrame,
    candidate: &OracleFrame,
) -> Result<(), OracleError> {
    if reference.width != candidate.width || reference.height != candidate.height {
        return Err(OracleError::IncompatibleComparison("dimensions"));
    }
    if reference.sensor_window != candidate.sensor_window {
        return Err(OracleError::IncompatibleComparison("sensor window"));
    }
    for (r, c) in reference.pixels.iter().zip(&candidate.pixels) {
        if r.local_index != c.local_index
            || r.col != c.col
            || r.row != c.row
            || r.source_index != c.source_index
            || r.source_col != c.source_col
            || r.source_row != c.source_row
            || r.sensor_x.to_bits() != c.sensor_x.to_bits()
            || r.sensor_y.to_bits() != c.sensor_y.to_bits()
        {
            return Err(OracleError::IncompatibleComparison("pixel layout"));
        }
    }
    Ok(())
}

#[derive(Default)]
struct ErrorAccumulator {
    count: u64,
    sum_abs: f64,
    sum_sq: f64,
    max_abs: f64,
    max_index: u64,
}

impl ErrorAccumulator {
    const fn new() -> Self {
        Self {
            count: 0,
            sum_abs: 0.0,
            sum_sq: 0.0,
            max_abs: 0.0,
            max_index: 0,
        }
    }

    fn push(&mut self, index: u64, error: f64) {
        debug_assert!(error.is_finite() && error >= 0.0);
        self.count += 1;
        self.sum_abs += error;
        self.sum_sq += error * error;
        if error > self.max_abs {
            self.max_abs = error;
            self.max_index = index;
        }
    }

    fn into_scalar(self) -> OptionalScalarErrorMetrics {
        (self.count > 0).then_some(ScalarErrorMetrics {
            mae: self.sum_abs / self.count as f64,
            rmse: (self.sum_sq / self.count as f64).sqrt(),
            maximum_absolute_error: self.max_abs,
            maximum_error_index: self.max_index,
        })
    }

    fn into_integer(self) -> IntegerErrorMetrics {
        IntegerErrorMetrics {
            mae: if self.count == 0 {
                0.0
            } else {
                self.sum_abs / self.count as f64
            },
            rmse: if self.count == 0 {
                0.0
            } else {
                (self.sum_sq / self.count as f64).sqrt()
            },
            maximum_absolute_error: self.max_abs as u64,
            maximum_error_index: self.max_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digests() -> OracleSourceDigests {
        OracleSourceDigests {
            numerical_profile_digest: "n".into(),
            trace_data_digest: "t".into(),
            outcome_class_digest: "o".into(),
            celestial_coordinate_digest: "c".into(),
            frequency_shift_digest: Some("f".into()),
            bolometric_digest: Some("b".into()),
        }
    }

    fn frame() -> OracleFrame {
        let mut frame = OracleFrame {
            schema_version: ORACLE_SCHEMA_VERSION,
            oracle_id: ORACLE_ID_V1.into(),
            width: 2,
            height: 2,
            sensor_window: SensorWindow::full_frame(),
            surface_set: TraceSurfaceSet::OpaqueDiskHorizonEscape,
            channel_set: OracleChannelSet::FullBolometricDisk,
            scientific_claim: OracleScientificClaim::v1(),
            source_digests: digests(),
            pixels: vec![
                OraclePixel {
                    local_index: 0,
                    col: 0,
                    row: 0,
                    source_index: 0,
                    source_col: 0,
                    source_row: 0,
                    sensor_x: -0.5,
                    sensor_y: 0.5,
                    outcome_class: OutcomeClass::Escaped,
                    rhs_evaluations: 10,
                    failure_class: None,
                    celestial: Some(OracleCelestialSample {
                        boundary_oblate_radius: 80.0,
                        theta: 1.0,
                        psi: 0.0,
                        unit_coordinate_direction: [1.0, 0.0, 0.0],
                        u: 0.99,
                        v: 0.25,
                        escape_event_value: 0.0,
                    }),
                    disk: None,
                },
                OraclePixel {
                    local_index: 1,
                    col: 1,
                    row: 0,
                    source_index: 1,
                    source_col: 1,
                    source_row: 0,
                    sensor_x: 0.5,
                    sensor_y: 0.5,
                    outcome_class: OutcomeClass::DiskHit,
                    rhs_evaluations: 20,
                    failure_class: None,
                    celestial: None,
                    disk: Some(OracleDiskSample {
                        radius: 4.0,
                        azimuth: 0.25,
                        g_factor: 2.0,
                        log2_g: 1.0,
                        g_fourth: 16.0,
                        emitted_bolometric_intensity: 3.0,
                        observed_bolometric_intensity: 48.0,
                        disk_event_value: 0.0,
                    }),
                },
                OraclePixel {
                    local_index: 2,
                    col: 0,
                    row: 1,
                    source_index: 2,
                    source_col: 0,
                    source_row: 1,
                    sensor_x: -0.5,
                    sensor_y: -0.5,
                    outcome_class: OutcomeClass::HorizonEvent,
                    rhs_evaluations: 30,
                    failure_class: None,
                    celestial: None,
                    disk: None,
                },
                OraclePixel {
                    local_index: 3,
                    col: 1,
                    row: 1,
                    source_index: 3,
                    source_col: 1,
                    source_row: 1,
                    sensor_x: 0.5,
                    sensor_y: -0.5,
                    outcome_class: OutcomeClass::Escaped,
                    rhs_evaluations: 40,
                    failure_class: None,
                    celestial: Some(OracleCelestialSample {
                        boundary_oblate_radius: 80.0,
                        theta: 2.0,
                        psi: 1.0,
                        unit_coordinate_direction: [0.0, 1.0, 0.0],
                        u: 0.25,
                        v: 0.75,
                        escape_event_value: 0.0,
                    }),
                    disk: None,
                },
            ],
            scientific_digest: String::new(),
        };
        frame.scientific_digest = oracle_scientific_digest(&frame);
        frame
    }

    #[test]
    fn sensor_window_validation() {
        assert_eq!(SensorWindow::full_frame().validate(), Ok(()));
        assert!(SensorWindow::new(f64::NAN, 1.0, -1.0, 1.0).is_err());
        assert!(SensorWindow::new(0.0, 0.0, -1.0, 1.0).is_err());
        assert!(SensorWindow::new(-1.0, 1.0, 2.0, 3.0).is_err());
    }

    #[test]
    fn digest_changes_with_scientific_channels() {
        let base = frame();
        let mut changed = base.clone();
        changed.pixels[0].outcome_class = OutcomeClass::HorizonEvent;
        assert_ne!(base.scientific_digest, oracle_scientific_digest(&changed));

        let mut changed = base.clone();
        changed.pixels[1].rhs_evaluations += 1;
        assert_ne!(base.scientific_digest, oracle_scientific_digest(&changed));

        let mut changed = base.clone();
        changed.pixels[0]
            .celestial
            .as_mut()
            .unwrap()
            .unit_coordinate_direction[0] = 0.5;
        assert_ne!(base.scientific_digest, oracle_scientific_digest(&changed));

        let mut changed = base.clone();
        changed.pixels[1].disk.as_mut().unwrap().g_factor = 2.1;
        assert_ne!(base.scientific_digest, oracle_scientific_digest(&changed));

        let mut changed = base.clone();
        changed.pixels[1]
            .disk
            .as_mut()
            .unwrap()
            .emitted_bolometric_intensity = 4.0;
        assert_ne!(base.scientific_digest, oracle_scientific_digest(&changed));

        let mut changed = base.clone();
        changed.pixels[1]
            .disk
            .as_mut()
            .unwrap()
            .observed_bolometric_intensity = 49.0;
        assert_ne!(base.scientific_digest, oracle_scientific_digest(&changed));
    }

    #[test]
    fn crop_preserves_source_coordinates_and_bits() {
        let source = frame();
        let crop = crop_oracle_frame(
            &source,
            PixelCrop {
                left: 1,
                top: 0,
                width: 1,
                height: 2,
            },
        )
        .unwrap();
        assert_eq!(crop.width, 1);
        assert_eq!(crop.height, 2);
        assert_eq!(crop.sensor_window.x_min, 0.0);
        assert_eq!(crop.sensor_window.x_max, 1.0);
        assert_eq!(crop.sensor_window.y_min, -1.0);
        assert_eq!(crop.sensor_window.y_max, 1.0);
        assert_eq!(crop.pixels[0].local_index, 0);
        assert_eq!(crop.pixels[0].source_index, 1);
        assert_eq!(crop.pixels[0].source_col, 1);
        assert_eq!(crop.pixels[0].source_row, 0);
        assert_eq!(crop.pixels[0].disk, source.pixels[1].disk);
        assert!(crop_oracle_frame(
            &source,
            PixelCrop {
                left: 2,
                top: 0,
                width: 1,
                height: 1,
            }
        )
        .is_err());

        let nested = crop_oracle_frame(
            &crop,
            PixelCrop {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            },
        )
        .unwrap();
        assert_eq!(nested.sensor_window.x_min, 0.0);
        assert_eq!(nested.sensor_window.x_max, 1.0);
        assert_eq!(nested.sensor_window.y_min, 0.0);
        assert_eq!(nested.sensor_window.y_max, 1.0);
        assert_eq!(nested.pixels[0].source_index, 1);
        assert_eq!(nested.pixels[0].sensor_x.to_bits(), 0.5f64.to_bits());
    }

    #[test]
    fn self_comparison_is_zero() {
        let f = frame();
        let m = compare_oracle_frames(&f, &f).unwrap();
        assert_eq!(m.outcome_disagreement_count, 0);
        assert_eq!(m.rhs_absolute_error.maximum_absolute_error, 0);
        assert_eq!(
            m.celestial_angular_error_radians
                .unwrap()
                .maximum_absolute_error,
            0.0
        );
        assert_eq!(m.log2_g_error.unwrap().maximum_absolute_error, 0.0);
    }

    #[test]
    fn comparison_metrics_are_seam_aware_and_tie_by_lowest_index() {
        let reference = frame();
        let mut candidate = reference.clone();
        candidate.pixels[0].celestial.as_mut().unwrap().u = 0.01;
        candidate.pixels[3].rhs_evaluations = 50;
        candidate.pixels[1].rhs_evaluations = 30;
        candidate.scientific_digest = oracle_scientific_digest(&candidate);
        let m = compare_oracle_frames(&reference, &candidate).unwrap();
        let u = m.celestial_wrap_u_error.unwrap();
        assert!((u.maximum_absolute_error - 0.02).abs() < 1e-15);
        assert_eq!(u.maximum_error_index, 0);
        assert_eq!(m.rhs_absolute_error.maximum_absolute_error, 10);
        assert_eq!(m.rhs_absolute_error.maximum_error_index, 1);
    }

    #[test]
    fn outcome_and_channel_presence_mismatches_are_reported() {
        let reference = frame();
        let mut candidate = reference.clone();
        // DiskHit → Escaped: both frames remain valid; presence differs independently.
        candidate.pixels[1].outcome_class = OutcomeClass::Escaped;
        candidate.pixels[1].disk = None;
        candidate.pixels[1].celestial = Some(OracleCelestialSample {
            boundary_oblate_radius: 80.0,
            theta: 1.5,
            psi: 0.5,
            unit_coordinate_direction: [0.0, 0.0, 1.0],
            u: 0.5,
            v: 0.5,
            escape_event_value: 0.0,
        });
        candidate.scientific_digest = oracle_scientific_digest(&candidate);
        let m = compare_oracle_frames(&reference, &candidate).unwrap();
        assert_eq!(m.outcome_disagreement_count, 1);
        assert_eq!(m.celestial_presence_mismatch_count, 1);
        assert_eq!(m.disk_presence_mismatch_count, 1);
        assert_eq!(m.disk_pair_count, 0);
        assert!(m.log2_g_error.is_none());
    }

    #[test]
    fn incompatible_grids_are_rejected() {
        let reference = frame();
        let mut candidate = reference.clone();
        candidate.width = 1;
        candidate.scientific_digest = oracle_scientific_digest(&candidate);
        assert!(compare_oracle_frames(&reference, &candidate).is_err());
    }

    #[test]
    fn malformed_frame_validate_and_deserialize_rejected() {
        let mut bad = frame();
        bad.pixels[1].outcome_class = OutcomeClass::Escaped;
        // disk still present → outcome/channel inconsistency
        assert!(matches!(
            bad.validate(),
            Err(OracleError::PixelMismatch { .. })
        ));
        bad.scientific_digest = oracle_scientific_digest(&bad);
        let json = serde_json::to_string(&bad).unwrap();
        assert!(serde_json::from_str::<OracleFrame>(&json).is_err());

        let mut truncated = frame();
        truncated.pixels.pop();
        assert!(matches!(
            truncated.validate(),
            Err(OracleError::LengthMismatch { .. })
        ));
    }

    #[test]
    fn malformed_source_crop_is_typed_rejection_not_panic() {
        let mut source = frame();
        source.pixels.clear();
        source.scientific_digest = oracle_scientific_digest(&source);
        let err = crop_oracle_frame(
            &source,
            PixelCrop {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            },
        );
        assert!(err.is_err());
    }
}
