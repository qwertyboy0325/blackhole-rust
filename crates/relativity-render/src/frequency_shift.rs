//! Disk-hit frequency-shift kinematics frame (Gate 2B0).
//!
//! Scientific channel only: `g = ν_obs / ν_em` with `ν_obs = 1` from camera-local
//! unit past-null normalization and circular equatorial emitter velocity.
//! Not emission, intensity, spectra, or physical RGB.

use crate::error::FrequencyShiftError;
use relativity_core::{
    circular_equatorial_geodesic_bl, contract_covector_vector, covector_ks_to_bl,
    frequency_shift_ratio, ks_to_bl_position, measured_frequency_from_backward_covector,
    prograde_equatorial_direction, EquatorialAngularDirection, KerrParams, MeasuredFrequency,
};
use relativity_trace::{hex_sha, pixel_index, OutcomeClass, RayOutcome, TraceBundle, TraceGrid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FREQUENCY_SHIFT_CONVENTION_ID: &str = "backward-covector-circular-disk-g-factor-v1";
pub const EQUATORIAL_POLICY_V1: &str = "localized-radius-equatorial-surface-canonicalization-v1";
pub const OBSERVER_UNIT_FREQUENCY_TOLERANCE: f64 = 1e-10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiskVelocityModel {
    ProgradeCircularGeodesic,
}

impl DiskVelocityModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProgradeCircularGeodesic => "prograde-circular-geodesic",
        }
    }

    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::ProgradeCircularGeodesic => "disk-velocity-model:prograde-circular-geodesic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObserverFrequencySource {
    CameraLocalUnitPastNull,
}

impl ObserverFrequencySource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CameraLocalUnitPastNull => "camera-local-unit-past-null",
        }
    }

    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::CameraLocalUnitPastNull => {
                "observer-frequency-source:camera-local-unit-past-null"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskFrequencyShiftConvention {
    pub schema_version: u32,
    pub convention_id: String,
    pub photon_orientation: String,
    pub measured_frequency_definition: String,
    pub observer_frequency_source: ObserverFrequencySource,
    pub disk_velocity_model: DiskVelocityModel,
    pub equatorial_policy: String,
    pub ratio_definition: String,
}

impl DiskFrequencyShiftConvention {
    pub fn v1() -> Self {
        Self {
            schema_version: 1,
            convention_id: FREQUENCY_SHIFT_CONVENTION_ID.into(),
            photon_orientation: "stored-past-directed-covector".into(),
            measured_frequency_definition: "p-backward-covector-contract-future-timelike-velocity"
                .into(),
            observer_frequency_source: ObserverFrequencySource::CameraLocalUnitPastNull,
            disk_velocity_model: DiskVelocityModel::ProgradeCircularGeodesic,
            equatorial_policy: EQUATORIAL_POLICY_V1.into(),
            ratio_definition: "observer-frequency-over-emitter-frequency".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskFrequencyShiftSample {
    pub velocity_model: DiskVelocityModel,
    pub resolved_direction: EquatorialAngularDirection,
    pub observer_frequency_source: ObserverFrequencySource,
    pub radius: f64,
    pub azimuth: f64,
    pub angular_velocity_bl: f64,
    pub emitter_four_velocity_bl: [f64; 4],
    pub observer_frequency: f64,
    pub emitter_frequency: f64,
    pub g_factor: f64,
    pub log2_g: f64,
    pub disk_event_value: f64,
    pub disk_radius_residual: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiskFrequencyShiftPixel {
    DiskHit(DiskFrequencyShiftSample),
    NotDiskHit { outcome_class: OutcomeClass },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskFrequencyShiftFrame {
    grid: TraceGrid,
    pixels: Vec<DiskFrequencyShiftPixel>,
}

impl DiskFrequencyShiftFrame {
    pub fn try_new(
        grid: TraceGrid,
        pixels: Vec<DiskFrequencyShiftPixel>,
    ) -> Result<Self, FrequencyShiftError> {
        if pixels.len() != grid.pixel_count() {
            return Err(FrequencyShiftError::FrameLengthMismatch);
        }
        Ok(Self { grid, pixels })
    }

    pub fn grid(&self) -> TraceGrid {
        self.grid
    }

    pub fn pixels(&self) -> &[DiskFrequencyShiftPixel] {
        &self.pixels
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &DiskFrequencyShiftPixel {
        &self.pixels[pixel_index(self.grid, col, row)]
    }
}

fn map_disk_hit(
    params: &KerrParams,
    hit: &relativity_trace::DiskHit,
    velocity_model: DiskVelocityModel,
) -> Result<DiskFrequencyShiftSample, FrequencyShiftError> {
    if velocity_model != DiskVelocityModel::ProgradeCircularGeodesic {
        return Err(FrequencyShiftError::UnsupportedVelocityModel);
    }
    let bl_recovered = ks_to_bl_position(params, &hit.state.position).map_err(|e| {
        FrequencyShiftError::CoreMapping {
            context: format!("KS→BL position: {e}"),
        }
    })?;
    // Equatorial surface canonicalization: use localized oblate radius at θ=π/2.
    let radius = hit.oblate_radius;
    let azimuth = bl_recovered.phi;
    let disk_radius_residual = bl_recovered.r - radius;

    let p_bl = covector_ks_to_bl(params, &bl_recovered, &hit.state.momentum).map_err(|e| {
        FrequencyShiftError::CoreMapping {
            context: format!("KS→BL covector: {e}"),
        }
    })?;

    let resolved_direction = prograde_equatorial_direction(params);
    let orbit =
        circular_equatorial_geodesic_bl(params, radius, resolved_direction).map_err(|e| {
            FrequencyShiftError::CoreMapping {
                context: format!("circular orbit: {e}"),
            }
        })?;

    let nu_em =
        measured_frequency_from_backward_covector(&p_bl, &orbit.four_velocity_bl).map_err(|e| {
            FrequencyShiftError::CoreMapping {
                context: format!("emitter frequency: {e}"),
            }
        })?;
    // Camera-local unit past-null: ν_obs = 1 (not contracted at a different event).
    let nu_obs = MeasuredFrequency::new(1.0).map_err(|e| FrequencyShiftError::CoreMapping {
        context: format!("observer frequency: {e}"),
    })?;
    let g = frequency_shift_ratio(nu_obs, nu_em).map_err(|e| FrequencyShiftError::CoreMapping {
        context: format!("g-factor: {e}"),
    })?;

    // Explicit BL contraction check: ν_em = p_t u^t + p_φ u^φ.
    let nu_em_explicit = p_bl.t * orbit.four_velocity_bl.t + p_bl.z * orbit.four_velocity_bl.z;
    if (nu_em_explicit - nu_em.value()).abs() > 1e-12 {
        return Err(FrequencyShiftError::CoreMapping {
            context: "emitter frequency BL t/φ contraction mismatch".into(),
        });
    }

    Ok(DiskFrequencyShiftSample {
        velocity_model,
        resolved_direction,
        observer_frequency_source: ObserverFrequencySource::CameraLocalUnitPastNull,
        radius,
        azimuth,
        angular_velocity_bl: orbit.angular_velocity_bl,
        emitter_four_velocity_bl: orbit.four_velocity_bl.components(),
        observer_frequency: nu_obs.value(),
        emitter_frequency: nu_em.value(),
        g_factor: g.value(),
        log2_g: g.log2(),
        disk_event_value: hit.event_value,
        disk_radius_residual,
    })
}

pub fn build_disk_frequency_shift_frame(
    params: &KerrParams,
    bundle: &TraceBundle,
    velocity_model: DiskVelocityModel,
) -> Result<DiskFrequencyShiftFrame, FrequencyShiftError> {
    let grid = bundle.grid;
    let mut pixels = Vec::with_capacity(grid.pixel_count());
    for row in 0..grid.height {
        for col in 0..grid.width {
            let outcome = bundle.outcome_at(col, row);
            let pixel = match outcome {
                RayOutcome::DiskHit(hit) => {
                    let sample = map_disk_hit(params, hit, velocity_model).map_err(|e| {
                        FrequencyShiftError::PixelMappingFailed {
                            col,
                            row,
                            cause: e.to_string(),
                        }
                    })?;
                    DiskFrequencyShiftPixel::DiskHit(sample)
                }
                other => DiskFrequencyShiftPixel::NotDiskHit {
                    outcome_class: other.class(),
                },
            };
            debug_assert_eq!(pixel_index(grid, col, row), pixels.len());
            pixels.push(pixel);
        }
    }
    DiskFrequencyShiftFrame::try_new(grid, pixels)
}

pub fn disk_frequency_shift_digest(
    frame: &DiskFrequencyShiftFrame,
    convention: &DiskFrequencyShiftConvention,
) -> String {
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"disk-frequency-shift-digest-v1");
    hash_convention(&mut h, convention);
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for (idx, pixel) in frame.pixels.iter().enumerate() {
        h.update((idx as u64).to_le_bytes());
        match pixel {
            DiskFrequencyShiftPixel::DiskHit(s) => {
                update_tagged_str(&mut h, b"kind", "disk-hit");
                update_tagged_str(&mut h, b"velocity-model", s.velocity_model.digest_tag());
                update_tagged_str(&mut h, b"direction", s.resolved_direction.digest_tag());
                update_tagged_str(
                    &mut h,
                    b"observer-source",
                    s.observer_frequency_source.digest_tag(),
                );
                h.update(s.radius.to_bits().to_le_bytes());
                h.update(s.azimuth.to_bits().to_le_bytes());
                h.update(s.angular_velocity_bl.to_bits().to_le_bytes());
                for c in s.emitter_four_velocity_bl {
                    h.update(c.to_bits().to_le_bytes());
                }
                h.update(s.observer_frequency.to_bits().to_le_bytes());
                h.update(s.emitter_frequency.to_bits().to_le_bytes());
                h.update(s.g_factor.to_bits().to_le_bytes());
                h.update(s.log2_g.to_bits().to_le_bytes());
                h.update(s.disk_event_value.to_bits().to_le_bytes());
                h.update(s.disk_radius_residual.to_bits().to_le_bytes());
            }
            DiskFrequencyShiftPixel::NotDiskHit { outcome_class } => {
                update_tagged_str(&mut h, b"kind", "not-disk-hit");
                update_tagged_str(&mut h, b"outcome-class", outcome_class.digest_tag());
            }
        }
    }
    hex_sha(&h.finalize())
}

fn hash_convention(h: &mut Sha256, c: &DiskFrequencyShiftConvention) {
    h.update(c.schema_version.to_le_bytes());
    update_tagged_str(h, b"convention-id", &c.convention_id);
    update_tagged_str(h, b"photon-orientation", &c.photon_orientation);
    update_tagged_str(
        h,
        b"measured-frequency-definition",
        &c.measured_frequency_definition,
    );
    update_tagged_str(
        h,
        b"observer-frequency-source",
        c.observer_frequency_source.digest_tag(),
    );
    update_tagged_str(
        h,
        b"disk-velocity-model",
        c.disk_velocity_model.digest_tag(),
    );
    update_tagged_str(h, b"equatorial-policy", &c.equatorial_policy);
    update_tagged_str(h, b"ratio-definition", &c.ratio_definition);
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedFrequencyShiftPixel {
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub g_factor: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrequencyShiftRegressionSample {
    pub role: String,
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub radius_bits: String,
    pub azimuth_bits: String,
    pub omega_bits: String,
    pub emitter_frequency_bits: String,
    pub g_factor_bits: String,
    pub log2_g_bits: String,
    pub event_value_bits: String,
    pub radius_residual_bits: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskFrequencyShiftPixelRecord {
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub pixel: DiskFrequencyShiftPixel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskFrequencyShiftMapArtifact {
    pub schema_version: u32,
    pub width: u32,
    pub height: u32,
    pub convention: DiskFrequencyShiftConvention,
    pub disk_hit_count: u64,
    pub mapped_count: u64,
    pub mapping_failure_count: u64,
    pub redshifted_count: u64,
    pub blueshifted_count: u64,
    pub exact_unity_count: u64,
    pub minimum_g: Option<RankedFrequencyShiftPixel>,
    pub maximum_g: Option<RankedFrequencyShiftPixel>,
    pub closest_to_unity: Option<RankedFrequencyShiftPixel>,
    pub maximum_abs_disk_radius_residual: f64,
    pub maximum_observer_unit_frequency_residual: f64,
    pub frequency_shift_digest: String,
    pub regression_corpus: Vec<FrequencyShiftRegressionSample>,
    pub pixels: Vec<DiskFrequencyShiftPixelRecord>,
    pub content_digest_excluding_digest_field: String,
}

fn bits_hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn ranked(
    index: u64,
    col: u32,
    row: u32,
    sample: &DiskFrequencyShiftSample,
) -> RankedFrequencyShiftPixel {
    RankedFrequencyShiftPixel {
        index,
        col,
        row,
        g_factor: sample.g_factor,
        radius: sample.radius,
    }
}

fn regression_sample(
    role: &str,
    index: u64,
    col: u32,
    row: u32,
    sample: &DiskFrequencyShiftSample,
) -> FrequencyShiftRegressionSample {
    FrequencyShiftRegressionSample {
        role: role.into(),
        index,
        col,
        row,
        radius_bits: bits_hex(sample.radius),
        azimuth_bits: bits_hex(sample.azimuth),
        omega_bits: bits_hex(sample.angular_velocity_bl),
        emitter_frequency_bits: bits_hex(sample.emitter_frequency),
        g_factor_bits: bits_hex(sample.g_factor),
        log2_g_bits: bits_hex(sample.log2_g),
        event_value_bits: bits_hex(sample.disk_event_value),
        radius_residual_bits: bits_hex(sample.disk_radius_residual),
    }
}

pub fn build_disk_frequency_shift_map_artifact(
    frame: &DiskFrequencyShiftFrame,
    convention: &DiskFrequencyShiftConvention,
    verification: ObserverFrequencyVerification,
) -> DiskFrequencyShiftMapArtifact {
    let mut disk_hits: Vec<(u64, u32, u32, DiskFrequencyShiftSample)> = Vec::new();
    let mut records = Vec::with_capacity(frame.pixels.len());
    let mut redshifted = 0u64;
    let mut blueshifted = 0u64;
    let mut exact_unity = 0u64;
    let mut max_abs_residual = 0.0_f64;

    for row in 0..frame.grid.height {
        for col in 0..frame.grid.width {
            let index = pixel_index(frame.grid, col, row) as u64;
            let pixel = frame.pixel_at(col, row).clone();
            if let DiskFrequencyShiftPixel::DiskHit(ref s) = pixel {
                match s.g_factor.partial_cmp(&1.0) {
                    Some(std::cmp::Ordering::Less) => redshifted += 1,
                    Some(std::cmp::Ordering::Equal) => exact_unity += 1,
                    Some(std::cmp::Ordering::Greater) => blueshifted += 1,
                    None => {}
                }
                max_abs_residual = max_abs_residual.max(s.disk_radius_residual.abs());
                disk_hits.push((index, col, row, s.clone()));
            }
            records.push(DiskFrequencyShiftPixelRecord {
                index,
                col,
                row,
                pixel,
            });
        }
    }

    let disk_hit_count = disk_hits.len() as u64;
    let pick = |prefer: &dyn Fn(
        &DiskFrequencyShiftSample,
        &DiskFrequencyShiftSample,
    ) -> std::cmp::Ordering|
     -> Option<(u64, u32, u32, &DiskFrequencyShiftSample)> {
        let mut best: Option<&(u64, u32, u32, DiskFrequencyShiftSample)> = None;
        for entry in &disk_hits {
            best = Some(match best {
                None => entry,
                Some(cur) => match prefer(&entry.3, &cur.3) {
                    std::cmp::Ordering::Less => entry,
                    std::cmp::Ordering::Greater => cur,
                    std::cmp::Ordering::Equal => {
                        if entry.0 < cur.0 {
                            entry
                        } else {
                            cur
                        }
                    }
                },
            });
        }
        best.map(|e| (e.0, e.1, e.2, &e.3))
    };

    let first = disk_hits.first().map(|e| (e.0, e.1, e.2, &e.3));
    let last = disk_hits.last().map(|e| (e.0, e.1, e.2, &e.3));
    let min_g = pick(&|a, b| a.g_factor.total_cmp(&b.g_factor));
    let max_g = pick(&|a, b| b.g_factor.total_cmp(&a.g_factor));
    let closest = pick(&|a, b| {
        (a.g_factor - 1.0)
            .abs()
            .total_cmp(&(b.g_factor - 1.0).abs())
    });
    let min_r = pick(&|a, b| a.radius.total_cmp(&b.radius));
    let max_r = pick(&|a, b| b.radius.total_cmp(&a.radius));
    let max_residual = pick(&|a, b| {
        b.disk_radius_residual
            .abs()
            .total_cmp(&a.disk_radius_residual.abs())
    });

    let mut regression_corpus = Vec::new();
    let mut push = |role: &str, sel: Option<(u64, u32, u32, &DiskFrequencyShiftSample)>| {
        if let Some((i, c, r, s)) = sel {
            regression_corpus.push(regression_sample(role, i, c, r, s));
        }
    };
    push("first-disk-hit", first);
    push("last-disk-hit", last);
    push("minimum-g", min_g);
    push("maximum-g", max_g);
    push("closest-to-unity", closest);
    push("minimum-radius", min_r);
    push("maximum-radius", max_r);
    push("largest-abs-disk-radius-residual", max_residual);
    let observer_carrier = disk_hits
        .iter()
        .find(|e| e.1 == verification.worst_col && e.2 == verification.worst_row)
        .map(|e| (e.0, e.1, e.2, &e.3))
        .or(first);
    push("largest-observer-unit-frequency-residual", observer_carrier);

    let frequency_shift_digest = disk_frequency_shift_digest(frame, convention);
    let mut art = DiskFrequencyShiftMapArtifact {
        schema_version: 1,
        width: frame.grid.width,
        height: frame.grid.height,
        convention: convention.clone(),
        disk_hit_count,
        mapped_count: disk_hit_count,
        mapping_failure_count: 0,
        redshifted_count: redshifted,
        blueshifted_count: blueshifted,
        exact_unity_count: exact_unity,
        minimum_g: min_g.map(|(i, c, r, s)| ranked(i, c, r, s)),
        maximum_g: max_g.map(|(i, c, r, s)| ranked(i, c, r, s)),
        closest_to_unity: closest.map(|(i, c, r, s)| ranked(i, c, r, s)),
        maximum_abs_disk_radius_residual: max_abs_residual,
        maximum_observer_unit_frequency_residual: verification.maximum_residual,
        frequency_shift_digest,
        regression_corpus,
        pixels: records,
        content_digest_excluding_digest_field: String::new(),
    };
    art.content_digest_excluding_digest_field = artifact_content_digest(&art);
    art
}

fn artifact_content_digest(art: &DiskFrequencyShiftMapArtifact) -> String {
    #[derive(Serialize)]
    struct Proj<'a> {
        schema_version: u32,
        width: u32,
        height: u32,
        convention: &'a DiskFrequencyShiftConvention,
        disk_hit_count: u64,
        mapped_count: u64,
        mapping_failure_count: u64,
        redshifted_count: u64,
        blueshifted_count: u64,
        exact_unity_count: u64,
        minimum_g: &'a Option<RankedFrequencyShiftPixel>,
        maximum_g: &'a Option<RankedFrequencyShiftPixel>,
        closest_to_unity: &'a Option<RankedFrequencyShiftPixel>,
        maximum_abs_disk_radius_residual_bits: u64,
        maximum_observer_unit_frequency_residual_bits: u64,
        frequency_shift_digest: &'a str,
        regression_corpus: &'a [FrequencyShiftRegressionSample],
        // Exclude full pixel dump from content digest? Spec includes scientific digest which
        // already covers pixels; still include counts/extrema. Pixel records are large —
        // include frequency_shift_digest as the pixel authority.
        content_digest_excluding_digest_field: &'a str,
    }
    let proj = Proj {
        schema_version: art.schema_version,
        width: art.width,
        height: art.height,
        convention: &art.convention,
        disk_hit_count: art.disk_hit_count,
        mapped_count: art.mapped_count,
        mapping_failure_count: art.mapping_failure_count,
        redshifted_count: art.redshifted_count,
        blueshifted_count: art.blueshifted_count,
        exact_unity_count: art.exact_unity_count,
        minimum_g: &art.minimum_g,
        maximum_g: &art.maximum_g,
        closest_to_unity: &art.closest_to_unity,
        maximum_abs_disk_radius_residual_bits: art.maximum_abs_disk_radius_residual.to_bits(),
        maximum_observer_unit_frequency_residual_bits: art
            .maximum_observer_unit_frequency_residual
            .to_bits(),
        frequency_shift_digest: &art.frequency_shift_digest,
        regression_corpus: &art.regression_corpus,
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize frequency map digest"))
}

/// Diagnostic visualization of `log2(g)` — not physical intensity.
pub fn shade_g_factor_debug(frame: &DiskFrequencyShiftFrame) -> relativity_trace::RgbFrame {
    let mut pixels = Vec::with_capacity(frame.pixels.len());
    for pixel in frame.pixels() {
        let rgb = match pixel {
            DiskFrequencyShiftPixel::DiskHit(s) => g_factor_debug_rgb(s.g_factor),
            DiskFrequencyShiftPixel::NotDiskHit { outcome_class } => match outcome_class {
                OutcomeClass::Escaped => [0, 32, 64],
                OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => [0, 0, 0],
                OutcomeClass::AffineLimit => [128, 0, 128],
                OutcomeClass::Failed => [255, 0, 0],
                OutcomeClass::DiskHit => [255, 0, 0], // impossible
            },
        };
        pixels.push(rgb);
    }
    relativity_trace::RgbFrame::try_new(frame.grid, pixels).expect("length matched")
}

pub fn g_factor_debug_rgb(g: f64) -> [u8; 3] {
    let x = (g.log2() / 2.0).clamp(-1.0, 1.0);
    if x <= 0.0 {
        let q = ((255.0 * (x + 1.0)).round() as i32).clamp(0, 255) as u8;
        [q, q, 255]
    } else {
        let q = ((255.0 * (1.0 - x)).round() as i32).clamp(0, 255) as u8;
        [255, q, q]
    }
}

pub fn g_visualization_range_counts(frame: &DiskFrequencyShiftFrame) -> (u64, u64) {
    let mut below = 0u64;
    let mut above = 0u64;
    for pixel in frame.pixels() {
        if let DiskFrequencyShiftPixel::DiskHit(s) = pixel {
            if s.g_factor < 0.25 {
                below += 1;
            } else if s.g_factor > 4.0 {
                above += 1;
            }
        }
    }
    (below, above)
}

/// Result of camera-local unit past-null verification (no integrator).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObserverFrequencyVerification {
    pub maximum_residual: f64,
    pub worst_col: u32,
    pub worst_row: u32,
}

/// Verify camera-local unit past-null: reconstruct initial rays, no integrator.
pub fn verify_observer_unit_frequency(
    params: &KerrParams,
    scene: &relativity_trace::TraceScene,
) -> Result<ObserverFrequencyVerification, FrequencyShiftError> {
    use relativity_core::{initialize_rectilinear_ray, zamo_observer};
    use relativity_trace::sensor_at_pixel_center;

    let obs =
        zamo_observer(params, &scene.observer).map_err(|e| FrequencyShiftError::CoreMapping {
            context: format!("zamo observer: {e}"),
        })?;
    let mut max_residual = -1.0_f64;
    let mut worst_col = 0u32;
    let mut worst_row = 0u32;
    for row in 0..scene.grid.height {
        for col in 0..scene.grid.width {
            let sensor = sensor_at_pixel_center(scene.grid, col, row);
            let ray =
                initialize_rectilinear_ray(params, &obs, &scene.camera, sensor).map_err(|e| {
                    FrequencyShiftError::CoreMapping {
                        context: format!("ray init ({col},{row}): {e}"),
                    }
                })?;
            let nu = contract_covector_vector(&ray.covariant_momentum, &obs.four_velocity);
            let residual = (nu - 1.0).abs();
            if !residual.is_finite() {
                return Err(FrequencyShiftError::ObserverFrequencyVerification {
                    col,
                    row,
                    residual,
                });
            }
            let better = residual > max_residual
                || (residual == max_residual
                    && pixel_index(scene.grid, col, row)
                        < pixel_index(scene.grid, worst_col, worst_row));
            if better {
                max_residual = residual;
                worst_col = col;
                worst_row = row;
            }
            if residual > OBSERVER_UNIT_FREQUENCY_TOLERANCE {
                return Err(FrequencyShiftError::ObserverFrequencyVerification {
                    col,
                    row,
                    residual,
                });
            }
        }
    }
    Ok(ObserverFrequencyVerification {
        maximum_residual: max_residual.max(0.0),
        worst_col,
        worst_row,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_core::{Covector, Vector};

    #[test]
    fn visualization_canonical_colors() {
        assert_eq!(g_factor_debug_rgb(0.25), [0, 0, 255]);
        assert_eq!(g_factor_debug_rgb(1.0), [255, 255, 255]);
        assert_eq!(g_factor_debug_rgb(4.0), [255, 0, 0]);
    }

    #[test]
    fn visualization_clamp_does_not_require_changing_g() {
        let g = 0.01_f64;
        let _ = g_factor_debug_rgb(g);
        assert_eq!(g, 0.01);
    }

    #[test]
    fn covariant_not_raised_for_emitter_frequency() {
        // Sanity: ν = p·u uses covector components directly.
        let p = Covector::new(-0.5, 0.0, 0.0, 2.0);
        let u = Vector::new(1.2, 0.0, 0.0, 0.3);
        let nu = contract_covector_vector(&p, &u);
        assert!((nu - (-0.5 * 1.2 + 2.0 * 0.3)).abs() < 1e-15);
    }
}
