//! Finite celestial-boundary coordinate mapping (Gate 2A1).
//!
//! Derives deterministic spherical Kerr–Schild `(θ, ψ)` and seam-defined UV
//! coordinates from each escaped ray's localized finite escape-boundary position.
//!
//! # Scientific claim
//!
//! Coordinates are on the finite diagnostic escape boundary
//! (`r_oblate = r_escape`). They are **not** asymptotic null directions at
//! future/past null infinity. Terminal momentum does not determine UV.
//!
//! The UV-debug image visualizes a coordinate field. It is not a celestial
//! texture, physical radiance image, or final lensed-sky render.

use crate::camera::{pixel_index, TraceGrid};
use crate::diagnostics::{hex_sha, PixelCoord};
use crate::outcome::{EscapeHit, OutcomeClass, RayOutcome};
use crate::shade::{categorical_rgb, RgbFrame};
use crate::trace::TraceBundle;
use relativity_core::{
    spherical_ks_direction_from_cartesian, KerrParams, PositionKs, SphericalKsAzimuthStatus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Strict algebraic envelope for coordinate unit-vector length (matches existing
/// coordinate round-trip scale; not relaxed for this gate).
const UNIT_DIRECTION_ABS_ERR: f64 = 1e-12;

pub const CELESTIAL_CONVENTION_ID: &str = "finite-oblate-ks-boundary-uv-v1";
pub const ACCEPTED_SEAM: &str = "positive_x_half_plane";
pub const RADIUS_POLICY_GATE_1B2_CAP: &str = "gate-1b2-diagnostic-radius-cap";

#[derive(Debug, Error, Clone, PartialEq)]
pub enum CelestialMappingError {
    #[error("non-finite escape-boundary position")]
    NonFinitePosition,
    #[error("unresolved oblate radius / spherical KS recovery: {0}")]
    CoordinateRecovery(String),
    #[error("invalid angular value")]
    InvalidAngle,
    #[error("invalid UV range u={u} v={v}")]
    InvalidUv { u: f64, v: f64 },
    #[error("coordinate direction not unit length: norm={norm}")]
    NonUnitDirection { norm: f64 },
    #[error("celestial frame length mismatch")]
    FrameLengthMismatch,
    #[error("unsupported celestial seam `{seam}` (only `{ACCEPTED_SEAM}` accepted)")]
    UnsupportedSeam { seam: String },
    #[error("escaped-ray celestial mapping failed at pixel ({col},{row}): {detail}")]
    EscapedMappingFailed { col: u32, row: u32, detail: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CelestialDirectionSource {
    FiniteOblateEscapeBoundaryPosition,
}

impl CelestialDirectionSource {
    /// Stable project-owned digest tag (not Debug/Display/serde).
    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::FiniteOblateEscapeBoundaryPosition => {
                "celestial-direction-source:finite-oblate-escape-boundary-position"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CelestialUv {
    pub u: f64,
    pub v: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CelestialBoundarySample {
    pub source: CelestialDirectionSource,
    pub oblate_radius: f64,
    pub theta: f64,
    pub psi: f64,
    pub unit_coordinate_direction: [f64; 3],
    pub uv: CelestialUv,
    pub azimuth_status: SphericalKsAzimuthStatus,
    pub escape_event_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CelestialCoordinatePixel {
    Escaped(CelestialBoundarySample),
    NotEscaped { outcome_class: OutcomeClass },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CelestialCoordinateFrame {
    grid: TraceGrid,
    pixels: Vec<CelestialCoordinatePixel>,
}

impl CelestialCoordinateFrame {
    pub fn try_new(
        grid: TraceGrid,
        pixels: Vec<CelestialCoordinatePixel>,
    ) -> Result<Self, CelestialMappingError> {
        if pixels.len() != grid.pixel_count() {
            return Err(CelestialMappingError::FrameLengthMismatch);
        }
        Ok(Self { grid, pixels })
    }

    pub fn grid(&self) -> TraceGrid {
        self.grid
    }

    pub fn pixels(&self) -> &[CelestialCoordinatePixel] {
        &self.pixels
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &CelestialCoordinatePixel {
        &self.pixels[pixel_index(self.grid, col, row)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CelestialCoordinateConvention {
    pub schema_version: u32,
    pub convention_id: String,
    pub source: CelestialDirectionSource,
    pub boundary_surface: String,
    pub angular_chart: String,
    pub north_axis: String,
    pub azimuth_handedness: String,
    pub seam: String,
    pub u_mapping: String,
    pub v_mapping: String,
    pub pole_policy: String,
    pub asymptotic_correction: String,
}

impl CelestialCoordinateConvention {
    pub fn finite_oblate_ks_boundary_uv_v1() -> Self {
        Self {
            schema_version: 1,
            convention_id: CELESTIAL_CONVENTION_ID.into(),
            source: CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition,
            boundary_surface: "constant-oblate-kerr-schild-radius".into(),
            angular_chart: "ingoing-spherical-kerr-schild".into(),
            north_axis: "positive-cartesian-ks-z".into(),
            azimuth_handedness: "psi-increases-from-plus-x-toward-plus-y".into(),
            seam: "spherical-ks-psi-zero".into(),
            u_mapping: "wrapped-psi-over-two-pi".into(),
            v_mapping: "theta-over-pi".into(),
            pole_policy: "canonical-psi-zero-with-explicit-status".into(),
            asymptotic_correction: "not-implemented".into(),
        }
    }
}

/// Validate the preset seam string for this gate.
pub fn validate_celestial_seam(seam: &str) -> Result<(), CelestialMappingError> {
    if seam == ACCEPTED_SEAM {
        Ok(())
    } else {
        Err(CelestialMappingError::UnsupportedSeam {
            seam: seam.to_string(),
        })
    }
}

fn canonicalize_neg_zero(v: f64) -> f64 {
    if v == 0.0 {
        0.0
    } else {
        v
    }
}

/// Wrap ψ into `[0, 2π)` with `−0 → +0`.
pub fn wrap_psi_0_2pi(psi: f64) -> f64 {
    canonicalize_neg_zero(psi.rem_euclid(std::f64::consts::TAU))
}

/// Map wrapped ψ and θ to UV under the seam convention `u = ψ/(2π)`, `v = θ/π`.
pub fn uv_from_spherical_angles(
    theta: f64,
    psi_wrapped: f64,
) -> Result<CelestialUv, CelestialMappingError> {
    if !theta.is_finite() || !psi_wrapped.is_finite() {
        return Err(CelestialMappingError::InvalidAngle);
    }
    let u = canonicalize_neg_zero(psi_wrapped / std::f64::consts::TAU);
    let v = canonicalize_neg_zero(theta / std::f64::consts::PI);
    if !(0.0..1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return Err(CelestialMappingError::InvalidUv { u, v });
    }
    Ok(CelestialUv { u, v })
}

fn validate_unit_direction(d: [f64; 3]) -> Result<(), CelestialMappingError> {
    if !d.iter().all(|c| c.is_finite()) {
        return Err(CelestialMappingError::NonFinitePosition);
    }
    let norm = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if (norm - 1.0).abs() > UNIT_DIRECTION_ABS_ERR {
        return Err(CelestialMappingError::NonUnitDirection { norm });
    }
    Ok(())
}

/// Map one escaped hit from its finite-boundary position (not momentum).
pub fn celestial_sample_from_escape(
    kerr: &KerrParams,
    escape: &EscapeHit,
) -> Result<CelestialBoundarySample, CelestialMappingError> {
    celestial_sample_from_position(kerr, &escape.state.position, escape.event_value)
}

/// Position-driven mapping (shared by EscapeHit and algebraic tests).
pub fn celestial_sample_from_position(
    kerr: &KerrParams,
    position: &PositionKs,
    escape_event_value: f64,
) -> Result<CelestialBoundarySample, CelestialMappingError> {
    if !position.t.is_finite()
        || !position.x.is_finite()
        || !position.y.is_finite()
        || !position.z.is_finite()
    {
        return Err(CelestialMappingError::NonFinitePosition);
    }
    let dir = spherical_ks_direction_from_cartesian(kerr, position)
        .map_err(|e| CelestialMappingError::CoordinateRecovery(e.to_string()))?;
    let psi = wrap_psi_0_2pi(dir.psi);
    let uv = uv_from_spherical_angles(dir.theta, psi)?;
    validate_unit_direction(dir.unit_coordinate_direction)?;
    if !dir.r.is_finite() || !escape_event_value.is_finite() {
        return Err(CelestialMappingError::InvalidAngle);
    }
    Ok(CelestialBoundarySample {
        source: CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition,
        oblate_radius: dir.r,
        theta: dir.theta,
        psi,
        unit_coordinate_direction: [
            canonicalize_neg_zero(dir.unit_coordinate_direction[0]),
            canonicalize_neg_zero(dir.unit_coordinate_direction[1]),
            canonicalize_neg_zero(dir.unit_coordinate_direction[2]),
        ],
        uv,
        azimuth_status: dir.azimuth_status,
        escape_event_value,
    })
}

/// Build a full celestial coordinate frame from a completed TraceBundle.
pub fn build_celestial_coordinate_frame(
    kerr: &KerrParams,
    bundle: &TraceBundle,
) -> Result<CelestialCoordinateFrame, CelestialMappingError> {
    let n = bundle.grid.pixel_count();
    let mut pixels = Vec::with_capacity(n);
    for row in 0..bundle.grid.height {
        for col in 0..bundle.grid.width {
            let outcome = bundle.outcome_at(col, row);
            let pixel = match outcome {
                RayOutcome::Escaped(hit) => match celestial_sample_from_escape(kerr, hit) {
                    Ok(sample) => CelestialCoordinatePixel::Escaped(sample),
                    Err(e) => {
                        return Err(CelestialMappingError::EscapedMappingFailed {
                            col,
                            row,
                            detail: e.to_string(),
                        });
                    }
                },
                other => CelestialCoordinatePixel::NotEscaped {
                    outcome_class: other.class(),
                },
            };
            pixels.push(pixel);
        }
    }
    CelestialCoordinateFrame::try_new(bundle.grid, pixels)
}

/// Scientific coordinate digest (bit patterns; excludes shading/PPM/timing).
///
/// Hashing rules:
/// - enums use fixed project-owned `digest_tag()` strings (never Debug/Display/serde);
/// - strings are length-prefixed with explicit domain separators;
/// - the full [`CelestialCoordinateConvention`] is included.
pub fn celestial_coordinate_digest(
    frame: &CelestialCoordinateFrame,
    convention: &CelestialCoordinateConvention,
) -> String {
    let mut h = Sha256::new();
    update_domain(&mut h, b"celestial-coordinate-digest-v1");
    update_convention(&mut h, convention);
    update_domain(&mut h, b"grid");
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    update_domain(&mut h, b"pixels-row-major");
    for (idx, pixel) in frame.pixels.iter().enumerate() {
        h.update((idx as u64).to_le_bytes());
        match pixel {
            CelestialCoordinatePixel::NotEscaped { outcome_class } => {
                update_domain(&mut h, b"pixel-kind:not-escaped");
                update_tagged_str(&mut h, b"outcome-class", outcome_class.digest_tag());
            }
            CelestialCoordinatePixel::Escaped(s) => {
                update_domain(&mut h, b"pixel-kind:escaped");
                update_tagged_str(&mut h, b"direction-source", s.source.digest_tag());
                h.update(s.oblate_radius.to_bits().to_le_bytes());
                h.update(s.theta.to_bits().to_le_bytes());
                h.update(s.psi.to_bits().to_le_bytes());
                for c in s.unit_coordinate_direction {
                    h.update(c.to_bits().to_le_bytes());
                }
                h.update(s.uv.u.to_bits().to_le_bytes());
                h.update(s.uv.v.to_bits().to_le_bytes());
                update_tagged_str(&mut h, b"azimuth-status", s.azimuth_status.digest_tag());
                h.update(s.escape_event_value.to_bits().to_le_bytes());
            }
        }
    }
    hex_sha(&h.finalize())
}

fn update_domain(h: &mut Sha256, domain: &[u8]) {
    update_tagged_bytes(h, b"domain", domain);
}

fn update_tagged_str(h: &mut Sha256, tag: &[u8], value: &str) {
    update_tagged_bytes(h, tag, value.as_bytes());
}

fn update_tagged_bytes(h: &mut Sha256, tag: &[u8], value: &[u8]) {
    // tag_len || tag || value_len || value  — length-prefixed to avoid concatenation ambiguity
    h.update((tag.len() as u64).to_le_bytes());
    h.update(tag);
    h.update((value.len() as u64).to_le_bytes());
    h.update(value);
}

fn update_convention(h: &mut Sha256, convention: &CelestialCoordinateConvention) {
    update_domain(h, b"convention");
    h.update(convention.schema_version.to_le_bytes());
    update_tagged_str(h, b"convention-id", &convention.convention_id);
    update_tagged_str(h, b"direction-source", convention.source.digest_tag());
    update_tagged_str(h, b"boundary-surface", &convention.boundary_surface);
    update_tagged_str(h, b"angular-chart", &convention.angular_chart);
    update_tagged_str(h, b"north-axis", &convention.north_axis);
    update_tagged_str(h, b"azimuth-handedness", &convention.azimuth_handedness);
    update_tagged_str(h, b"seam", &convention.seam);
    update_tagged_str(h, b"u-mapping", &convention.u_mapping);
    update_tagged_str(h, b"v-mapping", &convention.v_mapping);
    update_tagged_str(h, b"pole-policy", &convention.pole_policy);
    update_tagged_str(
        h,
        b"asymptotic-correction",
        &convention.asymptotic_correction,
    );
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CelestialCoordinatePixelRecord {
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub value: CelestialCoordinatePixel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CelestialCoordinateMapArtifact {
    pub schema_version: u32,
    pub width: u32,
    pub height: u32,
    pub convention: CelestialCoordinateConvention,
    pub preset_requested_radius: f64,
    pub resolved_diagnostic_escape_radius: f64,
    pub resolution_policy: String,
    pub boundary_oblate_radius: f64,
    pub escaped_count: u64,
    pub mapped_count: u64,
    pub mapping_failure_count: u64,
    pub pole_count: u64,
    pub coordinate_digest: String,
    pub pixels: Vec<CelestialCoordinatePixelRecord>,
    pub content_digest_excluding_digest_field: String,
}

pub fn build_celestial_coordinate_map_artifact(
    frame: &CelestialCoordinateFrame,
    convention: &CelestialCoordinateConvention,
    preset_requested_radius: f64,
    resolved_diagnostic_escape_radius: f64,
) -> CelestialCoordinateMapArtifact {
    let mut escaped_count = 0u64;
    let mut pole_count = 0u64;
    let mut records = Vec::with_capacity(frame.pixels.len());
    for row in 0..frame.grid.height {
        for col in 0..frame.grid.width {
            let index = pixel_index(frame.grid, col, row) as u64;
            let value = frame.pixel_at(col, row).clone();
            if let CelestialCoordinatePixel::Escaped(s) = &value {
                escaped_count += 1;
                if matches!(
                    s.azimuth_status,
                    SphericalKsAzimuthStatus::CanonicalizedNorthPole
                        | SphericalKsAzimuthStatus::CanonicalizedSouthPole
                ) {
                    pole_count += 1;
                }
            }
            records.push(CelestialCoordinatePixelRecord {
                index,
                col,
                row,
                value,
            });
        }
    }
    let mapped_count = escaped_count;
    let coordinate_digest = celestial_coordinate_digest(frame, convention);
    let mut art = CelestialCoordinateMapArtifact {
        schema_version: convention.schema_version,
        width: frame.grid.width,
        height: frame.grid.height,
        convention: convention.clone(),
        preset_requested_radius,
        resolved_diagnostic_escape_radius,
        resolution_policy: RADIUS_POLICY_GATE_1B2_CAP.into(),
        boundary_oblate_radius: resolved_diagnostic_escape_radius,
        escaped_count,
        mapped_count,
        mapping_failure_count: 0,
        pole_count,
        coordinate_digest,
        pixels: records,
        content_digest_excluding_digest_field: String::new(),
    };
    art.content_digest_excluding_digest_field = celestial_map_content_digest(&art);
    art
}

fn celestial_map_content_digest(art: &CelestialCoordinateMapArtifact) -> String {
    #[derive(Serialize)]
    struct Proj<'a> {
        schema_version: u32,
        width: u32,
        height: u32,
        convention: &'a CelestialCoordinateConvention,
        preset_requested_radius_bits: u64,
        resolved_diagnostic_escape_radius_bits: u64,
        resolution_policy: &'a str,
        boundary_oblate_radius_bits: u64,
        escaped_count: u64,
        mapped_count: u64,
        mapping_failure_count: u64,
        pole_count: u64,
        coordinate_digest: &'a str,
        pixels: &'a [CelestialCoordinatePixelRecord],
        content_digest_excluding_digest_field: &'a str,
    }
    let proj = Proj {
        schema_version: art.schema_version,
        width: art.width,
        height: art.height,
        convention: &art.convention,
        preset_requested_radius_bits: art.preset_requested_radius.to_bits(),
        resolved_diagnostic_escape_radius_bits: art.resolved_diagnostic_escape_radius.to_bits(),
        resolution_policy: &art.resolution_policy,
        boundary_oblate_radius_bits: art.boundary_oblate_radius.to_bits(),
        escaped_count: art.escaped_count,
        mapped_count: art.mapped_count,
        mapping_failure_count: art.mapping_failure_count,
        pole_count: art.pole_count,
        coordinate_digest: &art.coordinate_digest,
        pixels: &art.pixels,
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize celestial map digest"))
}

/// Coordinate-field visualization (not a texture / radiance image).
///
/// Escaped: `R=quant(u), G=quant(v), B=255`. Non-escaped: Gate 1B2 categorical.
pub fn shade_celestial_uv_debug(coordinates: &CelestialCoordinateFrame) -> RgbFrame {
    let n = coordinates.grid.pixel_count();
    let mut pixels = Vec::with_capacity(n);
    for p in coordinates.pixels() {
        let rgb = match p {
            CelestialCoordinatePixel::Escaped(s) => {
                let r = quantize_unit(s.uv.u);
                let g = quantize_unit(s.uv.v);
                [r, g, 255]
            }
            CelestialCoordinatePixel::NotEscaped { outcome_class } => {
                categorical_rgb(*outcome_class)
            }
        };
        pixels.push(rgb);
    }
    RgbFrame::try_new(coordinates.grid, pixels).expect("frame length matches grid")
}

fn quantize_unit(x: f64) -> u8 {
    // Map [0,1] → [0,255]; u in [0,1) saturates at 254 for exact 1.0 edge cases on v.
    let t = x.clamp(0.0, 1.0);
    (t * 255.0).round().clamp(0.0, 255.0) as u8
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CelestialRegressionSample {
    pub role: String,
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub theta_bits: String,
    pub psi_bits: String,
    pub u_bits: String,
    pub v_bits: String,
    pub direction_bits: [String; 3],
    pub event_value_bits: String,
}

fn bits_hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn sample_record(
    role: &str,
    index: u64,
    col: u32,
    row: u32,
    s: &CelestialBoundarySample,
) -> CelestialRegressionSample {
    CelestialRegressionSample {
        role: role.into(),
        index,
        col,
        row,
        theta_bits: bits_hex(s.theta),
        psi_bits: bits_hex(s.psi),
        u_bits: bits_hex(s.uv.u),
        v_bits: bits_hex(s.uv.v),
        direction_bits: [
            bits_hex(s.unit_coordinate_direction[0]),
            bits_hex(s.unit_coordinate_direction[1]),
            bits_hex(s.unit_coordinate_direction[2]),
        ],
        event_value_bits: bits_hex(s.escape_event_value),
    }
}

/// Deterministic escaped-ray corpus for authoritative Gate scene review.
pub fn build_celestial_regression_corpus(
    frame: &CelestialCoordinateFrame,
) -> Vec<CelestialRegressionSample> {
    let mut escaped: Vec<(u64, u32, u32, &CelestialBoundarySample)> = Vec::new();
    for row in 0..frame.grid.height {
        for col in 0..frame.grid.width {
            let index = pixel_index(frame.grid, col, row) as u64;
            if let CelestialCoordinatePixel::Escaped(s) = frame.pixel_at(col, row) {
                escaped.push((index, col, row, s));
            }
        }
    }
    if escaped.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    let first = escaped.first().unwrap();
    out.push(sample_record(
        "first_escaped",
        first.0,
        first.1,
        first.2,
        first.3,
    ));
    let last = escaped.last().unwrap();
    out.push(sample_record(
        "last_escaped",
        last.0,
        last.1,
        last.2,
        last.3,
    ));

    let pick_extreme = |want_max: bool, key: fn(&CelestialBoundarySample) -> f64| {
        escaped
            .iter()
            .min_by(|a, b| {
                let ka = key(a.3);
                let kb = key(b.3);
                let ord = if want_max {
                    kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
                };
                ord.then_with(|| a.0.cmp(&b.0))
            })
            .unwrap()
    };

    let r = pick_extreme(false, |s| s.uv.u);
    out.push(sample_record("min_u", r.0, r.1, r.2, r.3));
    let r = pick_extreme(true, |s| s.uv.u);
    out.push(sample_record("max_u", r.0, r.1, r.2, r.3));
    let r = pick_extreme(false, |s| s.uv.v);
    out.push(sample_record("min_v", r.0, r.1, r.2, r.3));
    let r = pick_extreme(true, |s| s.uv.v);
    out.push(sample_record("max_v", r.0, r.1, r.2, r.3));

    let closest = |target: f64| {
        escaped
            .iter()
            .min_by(|a, b| {
                let da = (a.3.uv.u - target).abs();
                let db = (b.3.uv.u - target).abs();
                da.partial_cmp(&db)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            })
            .unwrap()
    };
    let r = closest(0.25);
    out.push(sample_record("closest_u_0_25", r.0, r.1, r.2, r.3));
    let r = closest(0.50);
    out.push(sample_record("closest_u_0_50", r.0, r.1, r.2, r.3));
    let r = closest(0.75);
    out.push(sample_record("closest_u_0_75", r.0, r.1, r.2, r.3));

    let r = pick_extreme(true, |s| s.escape_event_value.abs());
    out.push(sample_record(
        "largest_abs_escape_event_residual",
        r.0,
        r.1,
        r.2,
        r.3,
    ));
    out
}

/// Rank escaped pixels by descending `|event_value|`, then ascending index.
pub fn worst_boundary_residual_pixels(
    frame: &CelestialCoordinateFrame,
    limit: usize,
) -> Vec<PixelCoord> {
    let mut ranked: Vec<(f64, u64, PixelCoord)> = Vec::new();
    for row in 0..frame.grid.height {
        for col in 0..frame.grid.width {
            let index = pixel_index(frame.grid, col, row) as u64;
            if let CelestialCoordinatePixel::Escaped(s) = frame.pixel_at(col, row) {
                ranked.push((s.escape_event_value.abs(), index, PixelCoord { col, row }));
            }
        }
    }
    ranked.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    ranked.into_iter().take(limit).map(|(_, _, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shade::categorical_rgb;
    use relativity_core::{cartesian_from_spherical_ks, Covector, PositionSphericalKs};
    use relativity_integrate::{
        AffineParameter, GeodesicState, IntegrationStats, InvariantDiagnostics, RawSolverStop,
    };

    fn dummy_escape(
        pos: PositionKs,
        mom: relativity_core::Covector,
        event_value: f64,
    ) -> EscapeHit {
        let state = GeodesicState::new(pos, mom).unwrap();
        EscapeHit {
            lambda: AffineParameter(1.0),
            state,
            raw_solver_stop: RawSolverStop {
                lambda: AffineParameter(1.0),
                state,
            },
            integration: IntegrationStats {
                accepted_steps: 1,
                rejected_steps: 0,
                rhs_evaluations: 1,
                callback_count: 0,
            },
            diagnostics: InvariantDiagnostics {
                h_initial: 0.0,
                h_final: 0.0,
                h_max_abs_residual: 0.0,
                p_t_initial: 0.0,
                p_t_final: 0.0,
                p_t_max_abs_drift: 0.0,
                non_finite_checks: 0,
                raw_vs_localized_lambda_separation: None,
                relative_tolerance: [1e-8; 8],
                absolute_tolerance: [1e-9; 8],
            },
            event_value,
        }
    }

    #[test]
    fn schwarzschild_cardinal_uv() {
        let kerr = KerrParams::new(1.0, 0.0).unwrap();
        let r = 80.0;
        let cases = [
            (0.0, [1.0, 0.0, 0.0], 0.0, 0.5),
            (std::f64::consts::FRAC_PI_2, [0.0, 1.0, 0.0], 0.25, 0.5),
            (std::f64::consts::PI, [-1.0, 0.0, 0.0], 0.5, 0.5),
            (
                3.0 * std::f64::consts::FRAC_PI_2,
                [0.0, -1.0, 0.0],
                0.75,
                0.5,
            ),
        ];
        for (psi, dir_exp, u_exp, v_exp) in cases {
            let sph = PositionSphericalKs::new(0.0, r, std::f64::consts::FRAC_PI_2, psi);
            let cart = cartesian_from_spherical_ks(&kerr, &sph).unwrap();
            let sample = celestial_sample_from_position(&kerr, &cart, 0.0).unwrap();
            for i in 0..3 {
                assert!(
                    (sample.unit_coordinate_direction[i] - dir_exp[i]).abs() < 1e-12,
                    "dir ψ={psi}"
                );
            }
            assert!((sample.uv.u - u_exp).abs() < 1e-12);
            assert!((sample.uv.v - v_exp).abs() < 1e-12);
        }
    }

    #[test]
    fn schwarzschild_direction_equals_euclidean_normalize() {
        let kerr = KerrParams::new(1.0, 0.0).unwrap();
        let cart = PositionKs::new(0.0, 3.0, 4.0, 12.0);
        let sample = celestial_sample_from_position(&kerr, &cart, 0.0).unwrap();
        let n = (3.0f64 * 3.0 + 4.0 * 4.0 + 12.0 * 12.0).sqrt();
        let eu = [3.0 / n, 4.0 / n, 12.0 / n];
        for i in 0..3 {
            assert!((sample.unit_coordinate_direction[i] - eu[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn seam_wrapping_sides() {
        let kerr = KerrParams::new(1.0, 0.0).unwrap();
        let r = 80.0;
        let delta = 1e-6;
        let plus = cartesian_from_spherical_ks(
            &kerr,
            &PositionSphericalKs::new(0.0, r, std::f64::consts::FRAC_PI_2, delta),
        )
        .unwrap();
        let minus = cartesian_from_spherical_ks(
            &kerr,
            &PositionSphericalKs::new(0.0, r, std::f64::consts::FRAC_PI_2, -delta),
        )
        .unwrap();
        let sp = celestial_sample_from_position(&kerr, &plus, 0.0).unwrap();
        let sm = celestial_sample_from_position(&kerr, &minus, 0.0).unwrap();
        assert!(sp.uv.u < 0.01, "plus side near 0, got {}", sp.uv.u);
        assert!(sm.uv.u > 0.99, "minus side near 1, got {}", sm.uv.u);
        assert_ne!(sp.uv.u, sm.uv.u);
        let zero = cartesian_from_spherical_ks(
            &kerr,
            &PositionSphericalKs::new(0.0, r, std::f64::consts::FRAC_PI_2, 0.0),
        )
        .unwrap();
        let s0 = celestial_sample_from_position(&kerr, &zero, 0.0).unwrap();
        assert_eq!(s0.uv.u, 0.0);
        assert!(s0.uv.u.to_bits() == 0.0f64.to_bits() || s0.psi == 0.0);
    }

    #[test]
    fn poles_canonical() {
        let kerr = KerrParams::new(1.0, 0.5).unwrap();
        let n = celestial_sample_from_position(&kerr, &PositionKs::new(0.0, 0.0, 0.0, 80.0), 0.0)
            .unwrap();
        assert_eq!(
            n.azimuth_status,
            SphericalKsAzimuthStatus::CanonicalizedNorthPole
        );
        assert_eq!(n.uv.u, 0.0);
        assert_eq!(n.uv.v, 0.0);
        assert_eq!(n.unit_coordinate_direction, [0.0, 0.0, 1.0]);

        let s = celestial_sample_from_position(&kerr, &PositionKs::new(0.0, 0.0, 0.0, -80.0), 0.0)
            .unwrap();
        assert_eq!(
            s.azimuth_status,
            SphericalKsAzimuthStatus::CanonicalizedSouthPole
        );
        assert_eq!(s.uv.u, 0.0);
        assert!((s.uv.v - 1.0).abs() < 1e-15);
        assert_eq!(s.unit_coordinate_direction, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn kerr_round_trip_preserves_angles() {
        let cases = [(0.5, 20.0, 1.0, 0.3), (0.999, 40.0, 0.8, 2.0)];
        for (a, r, theta, psi) in cases {
            let kerr = KerrParams::new(1.0, a).unwrap();
            let sph = PositionSphericalKs::new(0.0, r, theta, psi);
            let cart = cartesian_from_spherical_ks(&kerr, &sph).unwrap();
            let sample = celestial_sample_from_position(&kerr, &cart, 0.0).unwrap();
            assert!((sample.oblate_radius - r).abs() < 1e-10);
            assert!((sample.theta - theta).abs() < 1e-10);
            let wrapped = wrap_psi_0_2pi(psi);
            assert!((sample.psi - wrapped).abs() < 1e-10);
            let dir = [
                theta.sin() * wrapped.cos(),
                theta.sin() * wrapped.sin(),
                theta.cos(),
            ];
            for i in 0..3 {
                assert!((sample.unit_coordinate_direction[i] - dir[i]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn position_not_momentum_determines_uv() {
        let kerr = KerrParams::new(1.0, 0.0).unwrap();
        // Position on +x celestial coordinate (ψ=0, equator) → u=0, v=0.5
        let pos = PositionKs::new(0.0, 80.0, 0.0, 0.0);
        // Momentum points approximately +y (must not drive UV to u=0.25)
        let mom = Covector::from_components([0.0, 0.0, 1.0, 0.0]);
        let hit = dummy_escape(pos, mom, 0.0);
        let sample = celestial_sample_from_escape(&kerr, &hit).unwrap();
        assert!(
            (sample.uv.u - 0.0).abs() < 1e-12,
            "expected position u=0, got {}",
            sample.uv.u
        );
        assert!((sample.uv.v - 0.5).abs() < 1e-12);
        assert!((sample.uv.u - 0.25).abs() > 0.1);
    }

    #[test]
    fn unsupported_seam_rejected() {
        assert!(validate_celestial_seam("positive_x_half_plane").is_ok());
        assert!(matches!(
            validate_celestial_seam("other"),
            Err(CelestialMappingError::UnsupportedSeam { .. })
        ));
    }

    fn tiny_frame() -> CelestialCoordinateFrame {
        let kerr = KerrParams::new(1.0, 0.0).unwrap();
        let sample =
            celestial_sample_from_position(&kerr, &PositionKs::new(0.0, 80.0, 0.0, 0.0), 0.0)
                .unwrap();
        CelestialCoordinateFrame::try_new(
            TraceGrid {
                width: 2,
                height: 1,
            },
            vec![
                CelestialCoordinatePixel::Escaped(sample),
                CelestialCoordinatePixel::NotEscaped {
                    outcome_class: OutcomeClass::DiskHit,
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn digest_enum_tags_are_explicit_and_distinct() {
        let classes = [
            OutcomeClass::DiskHit,
            OutcomeClass::Escaped,
            OutcomeClass::HorizonEvent,
            OutcomeClass::HorizonApproach,
            OutcomeClass::AffineLimit,
            OutcomeClass::Failed,
        ];
        let mut tags: Vec<&str> = classes.iter().map(|c| c.digest_tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), 6);
        for t in &tags {
            assert!(t.starts_with("outcome-class:"));
            assert!(!t.contains("DiskHit")); // not Debug
        }

        let az = [
            SphericalKsAzimuthStatus::Defined,
            SphericalKsAzimuthStatus::CanonicalizedNorthPole,
            SphericalKsAzimuthStatus::CanonicalizedSouthPole,
        ];
        let mut atags: Vec<&str> = az.iter().map(|a| a.digest_tag()).collect();
        atags.sort_unstable();
        atags.dedup();
        assert_eq!(atags.len(), 3);
        for t in &atags {
            assert!(t.starts_with("spherical-ks-azimuth:"));
        }

        let src = CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition.digest_tag();
        assert_eq!(
            src,
            "celestial-direction-source:finite-oblate-escape-boundary-position"
        );
        assert!(!src.contains("FiniteOblate"));
    }

    #[test]
    fn convention_field_changes_alter_coordinate_digest() {
        let frame = tiny_frame();
        let base = CelestialCoordinateConvention::finite_oblate_ks_boundary_uv_v1();
        let d0 = celestial_coordinate_digest(&frame, &base);

        let mut seam = base.clone();
        seam.seam = "alternate-seam".into();
        assert_ne!(d0, celestial_coordinate_digest(&frame, &seam));

        let mut pole = base.clone();
        pole.pole_policy = "alternate-pole-policy".into();
        assert_ne!(d0, celestial_coordinate_digest(&frame, &pole));

        let mut umap = base.clone();
        umap.u_mapping = "alternate-u".into();
        assert_ne!(d0, celestial_coordinate_digest(&frame, &umap));

        let mut vmap = base.clone();
        vmap.v_mapping = "alternate-v".into();
        assert_ne!(d0, celestial_coordinate_digest(&frame, &vmap));

        let mut chart = base.clone();
        chart.angular_chart = "alternate-chart".into();
        assert_ne!(d0, celestial_coordinate_digest(&frame, &chart));
    }

    #[test]
    fn shade_style_does_not_affect_coordinate_digest() {
        let frame = tiny_frame();
        let conv = CelestialCoordinateConvention::finite_oblate_ks_boundary_uv_v1();
        let before = celestial_coordinate_digest(&frame, &conv);
        let _rgb_a = shade_celestial_uv_debug(&frame);
        let _rgb_b = categorical_rgb(OutcomeClass::Escaped);
        let after = celestial_coordinate_digest(&frame, &conv);
        assert_eq!(before, after);
    }
}
