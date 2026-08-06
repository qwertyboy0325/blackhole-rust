//! Diagnostic bolometric disk emission and `g⁴` transport (Gate 2B1).
//!
//! Scientific channel: normalized bolometric specific intensity in arbitrary
//! project units with `I_obs = g⁴ I_em`. Not spectra, temperature, physical RGB,
//! Novikov–Thorne, or film reconstruction.

use crate::error::BolometricRenderError;
use crate::frequency_shift::{DiskFrequencyShiftFrame, DiskFrequencyShiftPixel, DiskVelocityModel};
use crate::texture::{sample_procedural_celestial, ProceduralCelestialTextureSpec};
use relativity_core::{EquatorialAngularDirection, FrequencyShift};
use relativity_trace::{
    hex_sha, pixel_index, CelestialCoordinateFrame, CelestialCoordinatePixel, OutcomeClass,
    RgbFrame, TraceGrid,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BOLOMETRIC_CONVENTION_ID: &str = "diagnostic-bolometric-disk-g4-v1";
pub const EMISSION_PROFILE_ID_V1: &str = "diagnostic-radial-power-law-v1";
pub const DISK_BOUNDS_SOURCE_V1: &str = "resolved-trace-scene-thin-disk-v1";
pub const DISPLAY_ID_V1: &str = "fixed-log2-grayscale-v1";
pub const EMISSION_UNITS_V1: &str = "arbitrary-normalized-bolometric-specific-intensity";
/// Exact preset `disk.emission_model` accepted by Gate 2B1.
pub const CANONICAL_DISK_EMISSION_MODEL: &str = "diagnostic_radial_profile";
/// Exact preset `disk.emission_claim` accepted by Gate 2B1 (provenance string).
pub const CANONICAL_DISK_EMISSION_CLAIM: &str =
    "project diagnostic, not astrophysical or film-asset reconstruction";

/// Reject non-canonical preset emission model/claim before tracing.
pub fn validate_disk_emission_provenance(
    emission_model: &str,
    emission_claim: &str,
) -> Result<(), BolometricRenderError> {
    if emission_model != CANONICAL_DISK_EMISSION_MODEL {
        return Err(BolometricRenderError::UnsupportedEmissionModel(
            emission_model.into(),
        ));
    }
    if emission_claim != CANONICAL_DISK_EMISSION_CLAIM {
        return Err(BolometricRenderError::UnsupportedEmissionClaim(
            emission_claim.into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmissionNormalizationRadiusSource {
    ResolvedDiskInnerRadius,
}

impl EmissionNormalizationRadiusSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ResolvedDiskInnerRadius => "resolved-disk-inner-radius",
        }
    }

    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::ResolvedDiskInnerRadius => {
                "emission-normalization-radius-source:resolved-disk-inner-radius"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticAngularEmissionModel {
    IsotropicEmitterFrame,
}

impl DiagnosticAngularEmissionModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IsotropicEmitterFrame => "isotropic-emitter-frame",
        }
    }

    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::IsotropicEmitterFrame => {
                "diagnostic-angular-emission-model:isotropic-emitter-frame"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticBolometricEmissionSpec {
    pub schema_version: u32,
    pub profile_id: String,
    pub radial_exponent: u32,
    pub normalization: f64,
    pub normalization_radius_source: EmissionNormalizationRadiusSource,
    pub angular_model: DiagnosticAngularEmissionModel,
    pub units: String,
}

impl DiagnosticBolometricEmissionSpec {
    pub fn validate(&self) -> Result<(), BolometricRenderError> {
        let canon = diagnostic_bolometric_emission_v1();
        if self != &canon {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "non-canonical diagnostic-radial-power-law-v1 field mutation".into(),
            ));
        }
        Ok(())
    }
}

pub fn diagnostic_bolometric_emission_v1() -> DiagnosticBolometricEmissionSpec {
    DiagnosticBolometricEmissionSpec {
        schema_version: 1,
        profile_id: EMISSION_PROFILE_ID_V1.into(),
        radial_exponent: 3,
        normalization: 1.0,
        normalization_radius_source: EmissionNormalizationRadiusSource::ResolvedDiskInnerRadius,
        angular_model: DiagnosticAngularEmissionModel::IsotropicEmitterFrame,
        units: EMISSION_UNITS_V1.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ResolvedDiskBounds {
    inner_radius: f64,
    outer_radius: f64,
}

impl ResolvedDiskBounds {
    pub fn new(inner_radius: f64, outer_radius: f64) -> Result<Self, BolometricRenderError> {
        let bounds = Self {
            inner_radius,
            outer_radius,
        };
        bounds.validate()?;
        Ok(bounds)
    }

    /// Full invariant: finite, `inner > 0`, `outer > inner`. Never clamps.
    pub fn validate(self) -> Result<(), BolometricRenderError> {
        if !self.inner_radius.is_finite() || !self.outer_radius.is_finite() {
            return Err(BolometricRenderError::InvalidDiskBounds(
                "bounds must be finite".into(),
            ));
        }
        if !(self.inner_radius > 0.0) {
            return Err(BolometricRenderError::InvalidDiskBounds(
                "inner_radius must be > 0".into(),
            ));
        }
        if !(self.outer_radius > self.inner_radius) {
            return Err(BolometricRenderError::InvalidDiskBounds(
                "outer_radius must be > inner_radius".into(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn inner_radius(self) -> f64 {
        self.inner_radius
    }

    #[must_use]
    pub fn outer_radius(self) -> f64 {
        self.outer_radius
    }

    pub fn contains(self, radius: f64) -> bool {
        radius.is_finite() && radius >= self.inner_radius && radius <= self.outer_radius
    }
}

impl<'de> Deserialize<'de> for ResolvedDiskBounds {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            inner_radius: f64,
            outer_radius: f64,
        }
        let raw = Raw::deserialize(deserializer)?;
        ResolvedDiskBounds::new(raw.inner_radius, raw.outer_radius)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BolometricSpecificIntensity(f64);

impl BolometricSpecificIntensity {
    pub fn new(value: f64) -> Result<Self, BolometricRenderError> {
        if !value.is_finite() {
            return Err(BolometricRenderError::InvalidIntensity(
                "non-finite intensity".into(),
            ));
        }
        if value < 0.0 {
            return Err(BolometricRenderError::InvalidIntensity(
                "intensity must be >= 0".into(),
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn to_bits(self) -> u64 {
        self.0.to_bits()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BolometricTransportFactor(f64);

impl BolometricTransportFactor {
    pub fn from_frequency_shift(shift: FrequencyShift) -> Result<Self, BolometricRenderError> {
        let g = shift.value();
        if !g.is_finite() || !(g > 0.0) {
            return Err(BolometricRenderError::InvalidTransportFactor(
                "frequency shift must be finite and > 0".into(),
            ));
        }
        // Canonical association: g2 = g*g; g4 = g2*g2.
        let g2 = g * g;
        let g4 = g2 * g2;
        if !g4.is_finite() || !(g4 > 0.0) {
            return Err(BolometricRenderError::InvalidTransportFactor(
                "g⁴ must be finite and > 0".into(),
            ));
        }
        Ok(Self(g4))
    }

    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn to_bits(self) -> u64 {
        self.0.to_bits()
    }
}

pub fn transport_bolometric_specific_intensity(
    emitted: BolometricSpecificIntensity,
    shift: FrequencyShift,
) -> Result<(BolometricTransportFactor, BolometricSpecificIntensity), BolometricRenderError> {
    let factor = BolometricTransportFactor::from_frequency_shift(shift)?;
    let observed = BolometricSpecificIntensity::new(emitted.value() * factor.value())?;
    Ok((factor, observed))
}

pub fn sample_diagnostic_bolometric_emission(
    spec: &DiagnosticBolometricEmissionSpec,
    bounds: ResolvedDiskBounds,
    radius: f64,
) -> Result<BolometricSpecificIntensity, BolometricRenderError> {
    spec.validate()?;
    bounds.validate()?;
    if !radius.is_finite() {
        return Err(BolometricRenderError::InvalidIntensity(
            "radius must be finite".into(),
        ));
    }
    if !bounds.contains(radius) {
        return Err(BolometricRenderError::RadiusOutsideAnnulus { radius });
    }
    let ratio = bounds.inner_radius() / radius;
    let intensity = spec.normalization * ratio.powi(spec.radial_exponent as i32);
    BolometricSpecificIntensity::new(intensity)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BolometricDebugDisplaySpec {
    pub schema_version: u32,
    pub display_id: String,
    pub reference_intensity: f64,
    pub minimum_log2_stops: i32,
    pub maximum_log2_stops: i32,
}

impl BolometricDebugDisplaySpec {
    pub fn validate(&self) -> Result<(), BolometricRenderError> {
        let canon = bolometric_debug_display_v1();
        if self != &canon {
            return Err(BolometricRenderError::InvalidDisplaySpec(
                "non-canonical fixed-log2-grayscale-v1 field mutation".into(),
            ));
        }
        Ok(())
    }
}

pub fn bolometric_debug_display_v1() -> BolometricDebugDisplaySpec {
    BolometricDebugDisplaySpec {
        schema_version: 1,
        display_id: DISPLAY_ID_V1.into(),
        reference_intensity: 1.0,
        minimum_log2_stops: -16,
        maximum_log2_stops: 3,
    }
}

pub fn bolometric_debug_display_spec_digest(spec: &BolometricDebugDisplaySpec) -> String {
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"bolometric-debug-display-digest-v1");
    h.update(spec.schema_version.to_le_bytes());
    update_tagged_str(&mut h, b"display-id", &spec.display_id);
    h.update(spec.reference_intensity.to_bits().to_le_bytes());
    h.update(spec.minimum_log2_stops.to_le_bytes());
    h.update(spec.maximum_log2_stops.to_le_bytes());
    hex_sha(&h.finalize())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskBolometricConvention {
    pub schema_version: u32,
    pub convention_id: String,
    pub emission_profile_id: String,
    pub emission_claim: String,
    pub intensity_quantity: String,
    pub intensity_units: String,
    pub angular_emission_model: DiagnosticAngularEmissionModel,
    pub frequency_shift_source: String,
    pub transport_law: String,
    pub transport_arithmetic: String,
    pub disk_bounds_source: String,
    pub optical_model: String,
    pub spectral_status: String,
    pub physical_rgb_status: String,
}

impl DiskBolometricConvention {
    pub fn v1() -> Self {
        Self {
            schema_version: 1,
            convention_id: BOLOMETRIC_CONVENTION_ID.into(),
            emission_profile_id: EMISSION_PROFILE_ID_V1.into(),
            emission_claim: "project-diagnostic-not-astrophysical-or-film-reconstruction".into(),
            intensity_quantity: "bolometric-specific-intensity".into(),
            intensity_units: EMISSION_UNITS_V1.into(),
            angular_emission_model: DiagnosticAngularEmissionModel::IsotropicEmitterFrame,
            frequency_shift_source: "gate-2b0-frequency-shift-frame".into(),
            transport_law: "observed-bolometric-specific-intensity-equals-g-fourth-times-emitted"
                .into(),
            transport_arithmetic: "g2-equals-g-times-g-g4-equals-g2-times-g2".into(),
            disk_bounds_source: DISK_BOUNDS_SOURCE_V1.into(),
            optical_model: "opaque-first-intersection".into(),
            spectral_status: "not-implemented".into(),
            physical_rgb_status: "not-implemented".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskBolometricSample {
    pub emission_profile_id: String,
    pub radius: f64,
    pub azimuth: f64,
    pub g_factor: f64,
    pub g_fourth: f64,
    pub emitted_bolometric_intensity: f64,
    pub observed_bolometric_intensity: f64,
    pub velocity_model: DiskVelocityModel,
    pub resolved_direction: EquatorialAngularDirection,
    pub disk_event_value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiskBolometricPixel {
    DiskHit(DiskBolometricSample),
    NotDiskHit { outcome_class: OutcomeClass },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskBolometricFrame {
    grid: TraceGrid,
    pixels: Vec<DiskBolometricPixel>,
}

impl DiskBolometricFrame {
    pub fn try_new(
        grid: TraceGrid,
        pixels: Vec<DiskBolometricPixel>,
    ) -> Result<Self, BolometricRenderError> {
        if pixels.len() != grid.pixel_count() {
            return Err(BolometricRenderError::FrameLengthMismatch);
        }
        Ok(Self { grid, pixels })
    }

    pub fn grid(&self) -> TraceGrid {
        self.grid
    }

    pub fn pixels(&self) -> &[DiskBolometricPixel] {
        &self.pixels
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &DiskBolometricPixel {
        &self.pixels[pixel_index(self.grid, col, row)]
    }
}

pub fn build_disk_bolometric_frame(
    frequency_frame: &DiskFrequencyShiftFrame,
    spec: &DiagnosticBolometricEmissionSpec,
    bounds: ResolvedDiskBounds,
) -> Result<DiskBolometricFrame, BolometricRenderError> {
    spec.validate()?;
    bounds.validate()?;
    let grid = frequency_frame.grid();
    let mut pixels = Vec::with_capacity(grid.pixel_count());
    for row in 0..grid.height {
        for col in 0..grid.width {
            let pixel = match frequency_frame.pixel_at(col, row) {
                DiskFrequencyShiftPixel::DiskHit(fs) => {
                    let sample = map_disk_hit(fs, spec, bounds).map_err(|e| {
                        BolometricRenderError::PixelMappingFailed {
                            col,
                            row,
                            cause: e.to_string(),
                        }
                    })?;
                    DiskBolometricPixel::DiskHit(sample)
                }
                DiskFrequencyShiftPixel::NotDiskHit { outcome_class } => {
                    DiskBolometricPixel::NotDiskHit {
                        outcome_class: *outcome_class,
                    }
                }
            };
            debug_assert_eq!(pixel_index(grid, col, row), pixels.len());
            pixels.push(pixel);
        }
    }
    DiskBolometricFrame::try_new(grid, pixels)
}

fn map_disk_hit(
    fs: &crate::frequency_shift::DiskFrequencyShiftSample,
    spec: &DiagnosticBolometricEmissionSpec,
    bounds: ResolvedDiskBounds,
) -> Result<DiskBolometricSample, BolometricRenderError> {
    let emitted = sample_diagnostic_bolometric_emission(spec, bounds, fs.radius)?;
    let shift = FrequencyShift::new(fs.g_factor).map_err(|e| {
        BolometricRenderError::InvalidTransportFactor(format!("Gate 2B0 g_factor: {e}"))
    })?;
    let (factor, observed) = transport_bolometric_specific_intensity(emitted, shift)?;
    Ok(DiskBolometricSample {
        emission_profile_id: spec.profile_id.clone(),
        radius: fs.radius,
        azimuth: fs.azimuth,
        g_factor: fs.g_factor,
        g_fourth: factor.value(),
        emitted_bolometric_intensity: emitted.value(),
        observed_bolometric_intensity: observed.value(),
        velocity_model: fs.velocity_model,
        resolved_direction: fs.resolved_direction,
        disk_event_value: fs.disk_event_value,
    })
}

/// Canonical g⁴ arithmetic used by renderer and evaluator.
#[must_use]
pub fn canonical_g_fourth(g: f64) -> f64 {
    let g2 = g * g;
    g2 * g2
}

pub fn verify_disk_bolometric_frame(
    frequency_frame: &DiskFrequencyShiftFrame,
    bolometric_frame: &DiskBolometricFrame,
    spec: &DiagnosticBolometricEmissionSpec,
    bounds: ResolvedDiskBounds,
) -> Result<(), BolometricRenderError> {
    spec.validate()?;
    bounds.validate()?;
    if frequency_frame.grid() != bolometric_frame.grid() {
        return Err(BolometricRenderError::GridMismatch);
    }
    let grid = frequency_frame.grid();
    for row in 0..grid.height {
        for col in 0..grid.width {
            match (
                frequency_frame.pixel_at(col, row),
                bolometric_frame.pixel_at(col, row),
            ) {
                (DiskFrequencyShiftPixel::DiskHit(fs), DiskBolometricPixel::DiskHit(b)) => {
                    let expected_em =
                        sample_diagnostic_bolometric_emission(spec, bounds, fs.radius)?;
                    let expected_g4 = canonical_g_fourth(fs.g_factor);
                    let expected_obs = expected_em.value() * expected_g4;
                    if b.g_factor.to_bits() != fs.g_factor.to_bits() {
                        return Err(BolometricRenderError::VerificationFailed {
                            col,
                            row,
                            cause: "g_factor does not match Gate 2B0 source".into(),
                        });
                    }
                    if b.g_fourth.to_bits() != expected_g4.to_bits() {
                        return Err(BolometricRenderError::VerificationFailed {
                            col,
                            row,
                            cause: format!(
                                "g_fourth mismatch: got {} expected {}",
                                b.g_fourth, expected_g4
                            ),
                        });
                    }
                    if b.emitted_bolometric_intensity.to_bits() != expected_em.to_bits() {
                        return Err(BolometricRenderError::VerificationFailed {
                            col,
                            row,
                            cause: "emitted intensity mismatch vs resampled profile".into(),
                        });
                    }
                    if b.observed_bolometric_intensity.to_bits() != expected_obs.to_bits() {
                        return Err(BolometricRenderError::VerificationFailed {
                            col,
                            row,
                            cause: "observed != emitted × g⁴".into(),
                        });
                    }
                }
                (
                    DiskFrequencyShiftPixel::NotDiskHit { outcome_class: a },
                    DiskBolometricPixel::NotDiskHit { outcome_class: b },
                ) => {
                    if a != b {
                        return Err(BolometricRenderError::VerificationFailed {
                            col,
                            row,
                            cause: "non-disk outcome class mismatch".into(),
                        });
                    }
                }
                _ => {
                    return Err(BolometricRenderError::VerificationFailed {
                        col,
                        row,
                        cause: "disk/non-disk kind mismatch vs frequency frame".into(),
                    });
                }
            }
        }
    }
    Ok(())
}

pub fn diagnostic_bolometric_emission_spec_digest(
    spec: &DiagnosticBolometricEmissionSpec,
) -> String {
    let mut h = Sha256::new();
    update_tagged_bytes(
        &mut h,
        b"domain",
        b"diagnostic-bolometric-emission-spec-digest-v1",
    );
    h.update(spec.schema_version.to_le_bytes());
    update_tagged_str(&mut h, b"profile-id", &spec.profile_id);
    h.update(spec.radial_exponent.to_le_bytes());
    h.update(spec.normalization.to_bits().to_le_bytes());
    update_tagged_str(
        &mut h,
        b"normalization-radius-source",
        spec.normalization_radius_source.digest_tag(),
    );
    update_tagged_str(&mut h, b"angular-model", spec.angular_model.digest_tag());
    update_tagged_str(&mut h, b"units", &spec.units);
    hex_sha(&h.finalize())
}

pub fn disk_bolometric_digest(
    frame: &DiskBolometricFrame,
    convention: &DiskBolometricConvention,
    emission_spec: &DiagnosticBolometricEmissionSpec,
    bounds: ResolvedDiskBounds,
    source_frequency_shift_digest: &str,
    accepted_emission_model: &str,
    accepted_emission_claim: &str,
) -> Result<String, BolometricRenderError> {
    bounds.validate()?;
    emission_spec.validate()?;
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"disk-bolometric-digest-v1");
    hash_convention(&mut h, convention);
    hash_emission_spec(&mut h, emission_spec);
    h.update(bounds.inner_radius().to_bits().to_le_bytes());
    h.update(bounds.outer_radius().to_bits().to_le_bytes());
    update_tagged_str(
        &mut h,
        b"source-frequency-shift-digest",
        source_frequency_shift_digest,
    );
    update_tagged_str(&mut h, b"accepted-emission-model", accepted_emission_model);
    update_tagged_str(&mut h, b"accepted-emission-claim", accepted_emission_claim);
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for (idx, pixel) in frame.pixels.iter().enumerate() {
        h.update((idx as u64).to_le_bytes());
        match pixel {
            DiskBolometricPixel::DiskHit(s) => {
                update_tagged_str(&mut h, b"kind", "disk-hit");
                update_tagged_str(&mut h, b"emission-profile-id", &s.emission_profile_id);
                h.update(s.radius.to_bits().to_le_bytes());
                h.update(s.azimuth.to_bits().to_le_bytes());
                h.update(s.g_factor.to_bits().to_le_bytes());
                h.update(s.g_fourth.to_bits().to_le_bytes());
                h.update(s.emitted_bolometric_intensity.to_bits().to_le_bytes());
                h.update(s.observed_bolometric_intensity.to_bits().to_le_bytes());
                update_tagged_str(&mut h, b"velocity-model", s.velocity_model.digest_tag());
                update_tagged_str(&mut h, b"direction", s.resolved_direction.digest_tag());
                h.update(s.disk_event_value.to_bits().to_le_bytes());
            }
            DiskBolometricPixel::NotDiskHit { outcome_class } => {
                update_tagged_str(&mut h, b"kind", "not-disk-hit");
                update_tagged_str(&mut h, b"outcome-class", outcome_class.digest_tag());
            }
        }
    }
    Ok(hex_sha(&h.finalize()))
}

fn hash_convention(h: &mut Sha256, c: &DiskBolometricConvention) {
    h.update(c.schema_version.to_le_bytes());
    update_tagged_str(h, b"convention-id", &c.convention_id);
    update_tagged_str(h, b"emission-profile-id", &c.emission_profile_id);
    update_tagged_str(h, b"emission-claim", &c.emission_claim);
    update_tagged_str(h, b"intensity-quantity", &c.intensity_quantity);
    update_tagged_str(h, b"intensity-units", &c.intensity_units);
    update_tagged_str(
        h,
        b"angular-emission-model",
        c.angular_emission_model.digest_tag(),
    );
    update_tagged_str(h, b"frequency-shift-source", &c.frequency_shift_source);
    update_tagged_str(h, b"transport-law", &c.transport_law);
    update_tagged_str(h, b"transport-arithmetic", &c.transport_arithmetic);
    update_tagged_str(h, b"disk-bounds-source", &c.disk_bounds_source);
    update_tagged_str(h, b"optical-model", &c.optical_model);
    update_tagged_str(h, b"spectral-status", &c.spectral_status);
    update_tagged_str(h, b"physical-rgb-status", &c.physical_rgb_status);
}

fn hash_emission_spec(h: &mut Sha256, s: &DiagnosticBolometricEmissionSpec) {
    h.update(s.schema_version.to_le_bytes());
    update_tagged_str(h, b"profile-id", &s.profile_id);
    h.update(s.radial_exponent.to_le_bytes());
    h.update(s.normalization.to_bits().to_le_bytes());
    update_tagged_str(
        h,
        b"normalization-radius-source",
        s.normalization_radius_source.digest_tag(),
    );
    update_tagged_str(h, b"angular-model", s.angular_model.digest_tag());
    update_tagged_str(h, b"units", &s.units);
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
pub struct RankedBolometricPixel {
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub emitted: f64,
    pub observed: f64,
    pub g_fourth: f64,
    pub radius: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BolometricRegressionSample {
    pub role: String,
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub radius_bits: String,
    pub g_factor_bits: String,
    pub g_fourth_bits: String,
    pub emitted_bits: String,
    pub observed_bits: String,
    pub event_value_bits: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskBolometricPixelRecord {
    pub index: u64,
    pub col: u32,
    pub row: u32,
    pub pixel: DiskBolometricPixel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskBolometricMapArtifact {
    pub schema_version: u32,
    pub width: u32,
    pub height: u32,
    pub convention: DiskBolometricConvention,
    pub emission_spec: DiagnosticBolometricEmissionSpec,
    pub emission_spec_digest: String,
    pub accepted_emission_model: String,
    pub accepted_emission_claim: String,
    pub resolved_disk_bounds: ResolvedDiskBounds,
    pub source_frequency_shift_digest: String,
    pub disk_hit_count: u64,
    pub mapped_count: u64,
    pub mapping_failure_count: u64,
    pub attenuated_count: u64,
    pub boosted_count: u64,
    pub unchanged_count: u64,
    pub minimum_emitted: Option<RankedBolometricPixel>,
    pub maximum_emitted: Option<RankedBolometricPixel>,
    pub minimum_observed: Option<RankedBolometricPixel>,
    pub maximum_observed: Option<RankedBolometricPixel>,
    pub minimum_transport_factor: Option<RankedBolometricPixel>,
    pub maximum_transport_factor: Option<RankedBolometricPixel>,
    pub maximum_abs_transport_residual: f64,
    pub bolometric_digest: String,
    pub regression_corpus: Vec<BolometricRegressionSample>,
    pub pixels: Vec<DiskBolometricPixelRecord>,
    pub content_digest_excluding_digest_field: String,
}

fn bits_hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

fn ranked(index: u64, col: u32, row: u32, s: &DiskBolometricSample) -> RankedBolometricPixel {
    RankedBolometricPixel {
        index,
        col,
        row,
        emitted: s.emitted_bolometric_intensity,
        observed: s.observed_bolometric_intensity,
        g_fourth: s.g_fourth,
        radius: s.radius,
    }
}

fn regression_sample(
    role: &str,
    index: u64,
    col: u32,
    row: u32,
    s: &DiskBolometricSample,
) -> BolometricRegressionSample {
    BolometricRegressionSample {
        role: role.into(),
        index,
        col,
        row,
        radius_bits: bits_hex(s.radius),
        g_factor_bits: bits_hex(s.g_factor),
        g_fourth_bits: bits_hex(s.g_fourth),
        emitted_bits: bits_hex(s.emitted_bolometric_intensity),
        observed_bits: bits_hex(s.observed_bolometric_intensity),
        event_value_bits: bits_hex(s.disk_event_value),
    }
}

pub fn build_disk_bolometric_map_artifact(
    frame: &DiskBolometricFrame,
    convention: &DiskBolometricConvention,
    emission_spec: &DiagnosticBolometricEmissionSpec,
    bounds: ResolvedDiskBounds,
    source_frequency_shift_digest: &str,
    accepted_emission_model: &str,
    accepted_emission_claim: &str,
) -> Result<DiskBolometricMapArtifact, BolometricRenderError> {
    bounds.validate()?;
    emission_spec.validate()?;
    let mut disk_hits: Vec<(u64, u32, u32, DiskBolometricSample)> = Vec::new();
    let mut records = Vec::with_capacity(frame.pixels.len());
    let mut attenuated = 0u64;
    let mut boosted = 0u64;
    let mut unchanged = 0u64;
    let mut max_abs_residual = 0.0_f64;

    for row in 0..frame.grid.height {
        for col in 0..frame.grid.width {
            let index = pixel_index(frame.grid, col, row) as u64;
            let pixel = frame.pixel_at(col, row).clone();
            if let DiskBolometricPixel::DiskHit(ref s) = pixel {
                match s
                    .observed_bolometric_intensity
                    .partial_cmp(&s.emitted_bolometric_intensity)
                {
                    Some(std::cmp::Ordering::Less) => attenuated += 1,
                    Some(std::cmp::Ordering::Equal) => unchanged += 1,
                    Some(std::cmp::Ordering::Greater) => boosted += 1,
                    None => {}
                }
                let expected = s.emitted_bolometric_intensity * s.g_fourth;
                max_abs_residual =
                    max_abs_residual.max((s.observed_bolometric_intensity - expected).abs());
                disk_hits.push((index, col, row, s.clone()));
            }
            records.push(DiskBolometricPixelRecord {
                index,
                col,
                row,
                pixel,
            });
        }
    }

    let disk_hit_count = disk_hits.len() as u64;
    let pick = |prefer: &dyn Fn(
        &DiskBolometricSample,
        &DiskBolometricSample,
    ) -> std::cmp::Ordering|
     -> Option<(u64, u32, u32, &DiskBolometricSample)> {
        let mut best: Option<&(u64, u32, u32, DiskBolometricSample)> = None;
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
    let min_em = pick(&|a, b| {
        a.emitted_bolometric_intensity
            .total_cmp(&b.emitted_bolometric_intensity)
    });
    let max_em = pick(&|a, b| {
        b.emitted_bolometric_intensity
            .total_cmp(&a.emitted_bolometric_intensity)
    });
    let min_obs = pick(&|a, b| {
        a.observed_bolometric_intensity
            .total_cmp(&b.observed_bolometric_intensity)
    });
    let max_obs = pick(&|a, b| {
        b.observed_bolometric_intensity
            .total_cmp(&a.observed_bolometric_intensity)
    });
    let min_g4 = pick(&|a, b| a.g_fourth.total_cmp(&b.g_fourth));
    let max_g4 = pick(&|a, b| b.g_fourth.total_cmp(&a.g_fourth));
    let closest_g4 = pick(&|a, b| {
        (a.g_fourth - 1.0)
            .abs()
            .total_cmp(&(b.g_fourth - 1.0).abs())
    });
    let max_residual = pick(&|a, b| {
        let ra =
            (a.observed_bolometric_intensity - a.emitted_bolometric_intensity * a.g_fourth).abs();
        let rb =
            (b.observed_bolometric_intensity - b.emitted_bolometric_intensity * b.g_fourth).abs();
        rb.total_cmp(&ra)
    });

    let mut regression_corpus = Vec::new();
    let mut push = |role: &str, sel: Option<(u64, u32, u32, &DiskBolometricSample)>| {
        if let Some((i, c, r, s)) = sel {
            regression_corpus.push(regression_sample(role, i, c, r, s));
        }
    };
    push("first-disk-hit", first);
    push("last-disk-hit", last);
    push("minimum-emitted", min_em);
    push("maximum-emitted", max_em);
    push("minimum-observed", min_obs);
    push("maximum-observed", max_obs);
    push("minimum-g-fourth", min_g4);
    push("maximum-g-fourth", max_g4);
    push("closest-transport-factor-to-one", closest_g4);
    push("largest-abs-transport-residual", max_residual);

    let emission_spec_digest = diagnostic_bolometric_emission_spec_digest(emission_spec);
    let bolometric_digest = disk_bolometric_digest(
        frame,
        convention,
        emission_spec,
        bounds,
        source_frequency_shift_digest,
        accepted_emission_model,
        accepted_emission_claim,
    )?;
    let mut art = DiskBolometricMapArtifact {
        schema_version: 1,
        width: frame.grid.width,
        height: frame.grid.height,
        convention: convention.clone(),
        emission_spec: emission_spec.clone(),
        emission_spec_digest,
        accepted_emission_model: accepted_emission_model.into(),
        accepted_emission_claim: accepted_emission_claim.into(),
        resolved_disk_bounds: bounds,
        source_frequency_shift_digest: source_frequency_shift_digest.into(),
        disk_hit_count,
        mapped_count: disk_hit_count,
        mapping_failure_count: 0,
        attenuated_count: attenuated,
        boosted_count: boosted,
        unchanged_count: unchanged,
        minimum_emitted: min_em.map(|(i, c, r, s)| ranked(i, c, r, s)),
        maximum_emitted: max_em.map(|(i, c, r, s)| ranked(i, c, r, s)),
        minimum_observed: min_obs.map(|(i, c, r, s)| ranked(i, c, r, s)),
        maximum_observed: max_obs.map(|(i, c, r, s)| ranked(i, c, r, s)),
        minimum_transport_factor: min_g4.map(|(i, c, r, s)| ranked(i, c, r, s)),
        maximum_transport_factor: max_g4.map(|(i, c, r, s)| ranked(i, c, r, s)),
        maximum_abs_transport_residual: max_abs_residual,
        bolometric_digest,
        regression_corpus,
        pixels: records,
        content_digest_excluding_digest_field: String::new(),
    };
    art.content_digest_excluding_digest_field = artifact_content_digest(&art);
    Ok(art)
}

fn artifact_content_digest(art: &DiskBolometricMapArtifact) -> String {
    #[derive(Serialize)]
    struct Proj<'a> {
        schema_version: u32,
        width: u32,
        height: u32,
        convention: &'a DiskBolometricConvention,
        emission_spec: &'a DiagnosticBolometricEmissionSpec,
        emission_spec_digest: &'a str,
        accepted_emission_model: &'a str,
        accepted_emission_claim: &'a str,
        resolved_disk_bounds_inner_bits: u64,
        resolved_disk_bounds_outer_bits: u64,
        source_frequency_shift_digest: &'a str,
        disk_hit_count: u64,
        mapped_count: u64,
        mapping_failure_count: u64,
        attenuated_count: u64,
        boosted_count: u64,
        unchanged_count: u64,
        minimum_emitted: &'a Option<RankedBolometricPixel>,
        maximum_emitted: &'a Option<RankedBolometricPixel>,
        minimum_observed: &'a Option<RankedBolometricPixel>,
        maximum_observed: &'a Option<RankedBolometricPixel>,
        minimum_transport_factor: &'a Option<RankedBolometricPixel>,
        maximum_transport_factor: &'a Option<RankedBolometricPixel>,
        maximum_abs_transport_residual_bits: u64,
        bolometric_digest: &'a str,
        regression_corpus: &'a [BolometricRegressionSample],
        content_digest_excluding_digest_field: &'a str,
    }
    let proj = Proj {
        schema_version: art.schema_version,
        width: art.width,
        height: art.height,
        convention: &art.convention,
        emission_spec: &art.emission_spec,
        emission_spec_digest: &art.emission_spec_digest,
        accepted_emission_model: &art.accepted_emission_model,
        accepted_emission_claim: &art.accepted_emission_claim,
        resolved_disk_bounds_inner_bits: art.resolved_disk_bounds.inner_radius().to_bits(),
        resolved_disk_bounds_outer_bits: art.resolved_disk_bounds.outer_radius().to_bits(),
        source_frequency_shift_digest: &art.source_frequency_shift_digest,
        disk_hit_count: art.disk_hit_count,
        mapped_count: art.mapped_count,
        mapping_failure_count: art.mapping_failure_count,
        attenuated_count: art.attenuated_count,
        boosted_count: art.boosted_count,
        unchanged_count: art.unchanged_count,
        minimum_emitted: &art.minimum_emitted,
        maximum_emitted: &art.maximum_emitted,
        minimum_observed: &art.minimum_observed,
        maximum_observed: &art.maximum_observed,
        minimum_transport_factor: &art.minimum_transport_factor,
        maximum_transport_factor: &art.maximum_transport_factor,
        maximum_abs_transport_residual_bits: art.maximum_abs_transport_residual.to_bits(),
        bolometric_digest: &art.bolometric_digest,
        regression_corpus: &art.regression_corpus,
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize bolometric map digest"))
}

pub fn bolometric_intensity_debug_rgb(
    intensity: f64,
    display: &BolometricDebugDisplaySpec,
) -> Result<[u8; 3], BolometricRenderError> {
    display.validate()?;
    if !intensity.is_finite() || intensity < 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "display intensity must be finite and >= 0".into(),
        ));
    }
    if intensity == 0.0 {
        return Ok([0, 0, 0]);
    }
    let stops = (intensity / display.reference_intensity).log2();
    let denom = f64::from(display.maximum_log2_stops - display.minimum_log2_stops);
    let x = ((stops - f64::from(display.minimum_log2_stops)) / denom).clamp(0.0, 1.0);
    let q = ((255.0 * x).round() as i32).clamp(0, 255) as u8;
    Ok([q, q, q])
}

pub fn bolometric_display_range_counts(
    frame: &DiskBolometricFrame,
    display: &BolometricDebugDisplaySpec,
    observed: bool,
) -> Result<(u64, u64), BolometricRenderError> {
    display.validate()?;
    let min_i = display.reference_intensity * 2f64.powi(display.minimum_log2_stops);
    let max_i = display.reference_intensity * 2f64.powi(display.maximum_log2_stops);
    let mut below = 0u64;
    let mut above = 0u64;
    for pixel in frame.pixels() {
        if let DiskBolometricPixel::DiskHit(s) = pixel {
            let i = if observed {
                s.observed_bolometric_intensity
            } else {
                s.emitted_bolometric_intensity
            };
            if i > 0.0 && i < min_i {
                below += 1;
            } else if i > max_i {
                above += 1;
            }
        }
    }
    Ok((below, above))
}

fn non_disk_debug_rgb(outcome_class: OutcomeClass) -> [u8; 3] {
    match outcome_class {
        OutcomeClass::Escaped => [0, 32, 64],
        OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => [0, 0, 0],
        OutcomeClass::AffineLimit => [128, 0, 128],
        OutcomeClass::Failed | OutcomeClass::DiskHit => [255, 0, 0],
    }
}

pub fn shade_emitted_bolometric_debug(
    frame: &DiskBolometricFrame,
    display: &BolometricDebugDisplaySpec,
) -> Result<RgbFrame, BolometricRenderError> {
    display.validate()?;
    let mut pixels = Vec::with_capacity(frame.pixels.len());
    for pixel in frame.pixels() {
        let rgb = match pixel {
            DiskBolometricPixel::DiskHit(s) => {
                bolometric_intensity_debug_rgb(s.emitted_bolometric_intensity, display)?
            }
            DiskBolometricPixel::NotDiskHit { outcome_class } => non_disk_debug_rgb(*outcome_class),
        };
        pixels.push(rgb);
    }
    RgbFrame::try_new(frame.grid, pixels).map_err(|_| BolometricRenderError::FrameLengthMismatch)
}

pub fn shade_observed_bolometric_debug(
    frame: &DiskBolometricFrame,
    display: &BolometricDebugDisplaySpec,
) -> Result<RgbFrame, BolometricRenderError> {
    display.validate()?;
    let mut pixels = Vec::with_capacity(frame.pixels.len());
    for pixel in frame.pixels() {
        let rgb = match pixel {
            DiskBolometricPixel::DiskHit(s) => {
                bolometric_intensity_debug_rgb(s.observed_bolometric_intensity, display)?
            }
            DiskBolometricPixel::NotDiskHit { outcome_class } => non_disk_debug_rgb(*outcome_class),
        };
        pixels.push(rgb);
    }
    RgbFrame::try_new(frame.grid, pixels).map_err(|_| BolometricRenderError::FrameLengthMismatch)
}

pub fn render_bolometric_celestial_composite(
    coordinates: &CelestialCoordinateFrame,
    bolometric: &DiskBolometricFrame,
    texture: &ProceduralCelestialTextureSpec,
    display: &BolometricDebugDisplaySpec,
) -> Result<RgbFrame, BolometricRenderError> {
    display.validate()?;
    texture
        .validate()
        .map_err(|e| BolometricRenderError::Celestial(e.to_string()))?;
    if coordinates.grid() != bolometric.grid() {
        return Err(BolometricRenderError::GridMismatch);
    }
    let grid = coordinates.grid();
    let mut pixels = Vec::with_capacity(grid.pixel_count());
    for row in 0..grid.height {
        for col in 0..grid.width {
            let rgb = match (
                coordinates.pixel_at(col, row),
                bolometric.pixel_at(col, row),
            ) {
                (CelestialCoordinatePixel::Escaped(_), DiskBolometricPixel::DiskHit(_)) => {
                    return Err(BolometricRenderError::DiskHitAsEscaped { col, row });
                }
                (
                    CelestialCoordinatePixel::Escaped(sample),
                    DiskBolometricPixel::NotDiskHit { .. },
                ) => sample_procedural_celestial(texture, sample)
                    .map_err(|e| BolometricRenderError::Celestial(e.to_string()))?,
                (_, DiskBolometricPixel::DiskHit(s)) => {
                    if !s.observed_bolometric_intensity.is_finite() {
                        return Err(BolometricRenderError::InvalidIntensity(
                            "non-finite observed intensity in composite".into(),
                        ));
                    }
                    bolometric_intensity_debug_rgb(s.observed_bolometric_intensity, display)?
                }
                (
                    CelestialCoordinatePixel::NotEscaped { outcome_class },
                    DiskBolometricPixel::NotDiskHit {
                        outcome_class: bolo_class,
                    },
                ) => {
                    if outcome_class != bolo_class {
                        return Err(BolometricRenderError::VerificationFailed {
                            col,
                            row,
                            cause: "celestial/bolometric outcome class mismatch".into(),
                        });
                    }
                    non_disk_debug_rgb(*outcome_class)
                }
            };
            debug_assert_eq!(pixel_index(grid, col, row), pixels.len());
            pixels.push(rgb);
        }
    }
    RgbFrame::try_new(grid, pixels).map_err(|_| BolometricRenderError::FrameLengthMismatch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frequency_shift::{
        DiskFrequencyShiftPixel, DiskFrequencyShiftSample, ObserverFrequencySource,
    };
    use approx::assert_relative_eq;

    fn bounds_3_20() -> ResolvedDiskBounds {
        ResolvedDiskBounds::new(3.0, 20.0).unwrap()
    }

    fn fs_sample(radius: f64, g: f64) -> DiskFrequencyShiftSample {
        DiskFrequencyShiftSample {
            velocity_model: DiskVelocityModel::ProgradeCircularGeodesic,
            resolved_direction: EquatorialAngularDirection::PositivePhi,
            observer_frequency_source: ObserverFrequencySource::CameraLocalUnitPastNull,
            radius,
            azimuth: 0.1,
            angular_velocity_bl: 0.05,
            emitter_four_velocity_bl: [1.0, 0.0, 0.0, 0.05],
            observer_frequency: 1.0,
            emitter_frequency: 1.0 / g,
            g_factor: g,
            log2_g: g.log2(),
            disk_event_value: 1e-12,
            disk_radius_residual: 0.0,
        }
    }

    #[test]
    fn canonical_emission_spec_validates() {
        diagnostic_bolometric_emission_v1().validate().unwrap();
    }

    #[test]
    fn every_emission_field_mutation_rejected_and_changes_digest() {
        let canon = diagnostic_bolometric_emission_v1();
        let d0 = diagnostic_bolometric_emission_spec_digest(&canon);
        let mutors: Vec<Box<dyn Fn() -> DiagnosticBolometricEmissionSpec>> = vec![
            Box::new(|| {
                let mut s = diagnostic_bolometric_emission_v1();
                s.schema_version = 2;
                s
            }),
            Box::new(|| {
                let mut s = diagnostic_bolometric_emission_v1();
                s.profile_id = "other".into();
                s
            }),
            Box::new(|| {
                let mut s = diagnostic_bolometric_emission_v1();
                s.radial_exponent = 4;
                s
            }),
            Box::new(|| {
                let mut s = diagnostic_bolometric_emission_v1();
                s.normalization = 2.0;
                s
            }),
            Box::new(|| {
                let mut s = diagnostic_bolometric_emission_v1();
                s.units = "other".into();
                s
            }),
        ];
        for m in mutors {
            let s = m();
            assert!(s.validate().is_err());
            assert_ne!(diagnostic_bolometric_emission_spec_digest(&s), d0);
        }
    }

    #[test]
    fn profile_inner_unity_and_outer_value() {
        let spec = diagnostic_bolometric_emission_v1();
        let b = bounds_3_20();
        let i_in = sample_diagnostic_bolometric_emission(&spec, b, 3.0)
            .unwrap()
            .value();
        let i_out = sample_diagnostic_bolometric_emission(&spec, b, 20.0)
            .unwrap()
            .value();
        assert_relative_eq!(i_in, 1.0, epsilon = 0.0);
        assert_relative_eq!(i_out, 0.003375, epsilon = 1e-15);
    }

    #[test]
    fn profile_monotonic_and_rejects_outside() {
        let spec = diagnostic_bolometric_emission_v1();
        let b = bounds_3_20();
        let mut prev = f64::INFINITY;
        for r in [3.0, 4.0, 6.0, 10.0, 20.0] {
            let i = sample_diagnostic_bolometric_emission(&spec, b, r)
                .unwrap()
                .value();
            assert!(i < prev);
            prev = i;
        }
        assert!(matches!(
            sample_diagnostic_bolometric_emission(&spec, b, 2.9),
            Err(BolometricRenderError::RadiusOutsideAnnulus { .. })
        ));
        assert!(matches!(
            sample_diagnostic_bolometric_emission(&spec, b, 20.1),
            Err(BolometricRenderError::RadiusOutsideAnnulus { .. })
        ));
    }

    #[test]
    fn transport_canonical_factors() {
        let em = BolometricSpecificIntensity::new(2.0).unwrap();
        let (f1, o1) =
            transport_bolometric_specific_intensity(em, FrequencyShift::new(1.0).unwrap()).unwrap();
        assert_eq!(f1.value(), 1.0);
        assert_eq!(o1.value(), 2.0);
        let (f2, o2) =
            transport_bolometric_specific_intensity(em, FrequencyShift::new(2.0).unwrap()).unwrap();
        assert_eq!(f2.value(), 16.0);
        assert_eq!(o2.value(), 32.0);
        let (f3, o3) =
            transport_bolometric_specific_intensity(em, FrequencyShift::new(0.5).unwrap()).unwrap();
        assert_eq!(f3.value(), 0.0625);
        assert_eq!(o3.value(), 0.125);
        assert_eq!(canonical_g_fourth(2.0), 16.0);
        assert_eq!(canonical_g_fourth(0.5), 0.0625);
    }

    #[test]
    fn transport_scales_with_emitted() {
        let g = FrequencyShift::new(1.5).unwrap();
        let a = BolometricSpecificIntensity::new(0.4).unwrap();
        let b = BolometricSpecificIntensity::new(1.2).unwrap();
        let (_, oa) = transport_bolometric_specific_intensity(a, g).unwrap();
        let (_, ob) = transport_bolometric_specific_intensity(b, g).unwrap();
        assert_relative_eq!(ob.value(), 3.0 * oa.value(), epsilon = 1e-15);
    }

    #[test]
    fn zero_emitted_stays_zero() {
        let (_, o) = transport_bolometric_specific_intensity(
            BolometricSpecificIntensity::new(0.0).unwrap(),
            FrequencyShift::new(2.0).unwrap(),
        )
        .unwrap();
        assert_eq!(o.value(), 0.0);
    }

    #[test]
    fn gate2b0_g_is_sole_source() {
        let grid = TraceGrid {
            width: 1,
            height: 1,
        };
        let mut s = fs_sample(6.0, 0.5);
        let frame = DiskFrequencyShiftFrame::try_new(
            grid,
            vec![DiskFrequencyShiftPixel::DiskHit(s.clone())],
        )
        .unwrap();
        let spec = diagnostic_bolometric_emission_v1();
        let b = bounds_3_20();
        let bolo = build_disk_bolometric_frame(&frame, &spec, b).unwrap();
        match bolo.pixel_at(0, 0) {
            DiskBolometricPixel::DiskHit(x) => {
                assert_eq!(x.g_factor.to_bits(), 0.5f64.to_bits());
                assert_eq!(x.g_fourth, 0.0625);
            }
            _ => panic!("expected disk"),
        }
        s.g_factor = 2.0;
        s.emitter_frequency = 0.5;
        s.log2_g = 1.0;
        let frame2 =
            DiskFrequencyShiftFrame::try_new(grid, vec![DiskFrequencyShiftPixel::DiskHit(s)])
                .unwrap();
        let bolo2 = build_disk_bolometric_frame(&frame2, &spec, b).unwrap();
        match bolo2.pixel_at(0, 0) {
            DiskBolometricPixel::DiskHit(x) => {
                assert_eq!(x.g_factor.to_bits(), 2.0f64.to_bits());
                assert_eq!(x.g_fourth, 16.0);
            }
            _ => panic!("expected disk"),
        }
        let d1 = disk_bolometric_digest(
            &bolo,
            &DiskBolometricConvention::v1(),
            &spec,
            b,
            "src",
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .unwrap();
        let d2 = disk_bolometric_digest(
            &bolo2,
            &DiskBolometricConvention::v1(),
            &spec,
            b,
            "src",
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .unwrap();
        assert_ne!(d1, d2);
    }

    #[test]
    fn display_does_not_affect_scientific_digest() {
        let grid = TraceGrid {
            width: 1,
            height: 1,
        };
        let frame = DiskFrequencyShiftFrame::try_new(
            grid,
            vec![DiskFrequencyShiftPixel::DiskHit(fs_sample(6.0, 1.0))],
        )
        .unwrap();
        let spec = diagnostic_bolometric_emission_v1();
        let b = bounds_3_20();
        let bolo = build_disk_bolometric_frame(&frame, &spec, b).unwrap();
        let d = disk_bolometric_digest(
            &bolo,
            &DiskBolometricConvention::v1(),
            &spec,
            b,
            "src",
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .unwrap();
        let display = bolometric_debug_display_v1();
        let _ = shade_observed_bolometric_debug(&bolo, &display).unwrap();
        let d2 = disk_bolometric_digest(
            &bolo,
            &DiskBolometricConvention::v1(),
            &spec,
            b,
            "src",
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn display_endpoints_and_clamp() {
        let display = bolometric_debug_display_v1();
        let i_min = 1.0 * 2f64.powi(-16);
        let i_max = 1.0 * 2f64.powi(3);
        assert_eq!(
            bolometric_intensity_debug_rgb(i_min, &display).unwrap(),
            [0, 0, 0]
        );
        assert_eq!(
            bolometric_intensity_debug_rgb(i_max, &display).unwrap(),
            [255, 255, 255]
        );
        let tiny = 1e-30_f64;
        let _ = bolometric_intensity_debug_rgb(tiny, &display).unwrap();
        assert_eq!(tiny, 1e-30);
    }

    #[test]
    fn invalid_bounds_and_intensity_rejected() {
        assert!(ResolvedDiskBounds::new(0.0, 10.0).is_err());
        assert!(ResolvedDiskBounds::new(10.0, 10.0).is_err());
        assert!(ResolvedDiskBounds::new(-1.0, 10.0).is_err());
        assert!(ResolvedDiskBounds::new(f64::NAN, 10.0).is_err());
        assert!(ResolvedDiskBounds::new(3.0, f64::INFINITY).is_err());
        assert!(BolometricSpecificIntensity::new(-1.0).is_err());
        assert!(BolometricSpecificIntensity::new(f64::NAN).is_err());
        assert!(FrequencyShift::new(-1.0).is_err());
    }

    #[test]
    fn illegal_bounds_struct_literal_rejected_by_public_apis() {
        // Same-module bypass of `new()`; public scientific entry points must still reject.
        let bad = ResolvedDiskBounds {
            inner_radius: 0.0,
            outer_radius: 10.0,
        };
        assert!(matches!(
            bad.validate(),
            Err(BolometricRenderError::InvalidDiskBounds(_))
        ));
        let spec = diagnostic_bolometric_emission_v1();
        assert!(matches!(
            sample_diagnostic_bolometric_emission(&spec, bad, 5.0),
            Err(BolometricRenderError::InvalidDiskBounds(_))
        ));
        let grid = TraceGrid {
            width: 1,
            height: 1,
        };
        let frame = DiskFrequencyShiftFrame::try_new(
            grid,
            vec![DiskFrequencyShiftPixel::DiskHit(fs_sample(6.0, 1.0))],
        )
        .unwrap();
        assert!(matches!(
            build_disk_bolometric_frame(&frame, &spec, bad),
            Err(BolometricRenderError::InvalidDiskBounds(_))
        ));
        assert!(matches!(
            disk_bolometric_digest(
                &DiskBolometricFrame::try_new(
                    grid,
                    vec![DiskBolometricPixel::NotDiskHit {
                        outcome_class: OutcomeClass::Escaped,
                    }],
                )
                .unwrap(),
                &DiskBolometricConvention::v1(),
                &spec,
                bad,
                "src",
                CANONICAL_DISK_EMISSION_MODEL,
                CANONICAL_DISK_EMISSION_CLAIM,
            ),
            Err(BolometricRenderError::InvalidDiskBounds(_))
        ));
    }

    #[test]
    fn illegal_bounds_deserialize_rejected() {
        let json = r#"{"inner_radius":0.0,"outer_radius":10.0}"#;
        let err = serde_json::from_str::<ResolvedDiskBounds>(json).unwrap_err();
        assert!(err.to_string().contains("inner_radius must be > 0"));
        let equal = r#"{"inner_radius":5.0,"outer_radius":5.0}"#;
        assert!(serde_json::from_str::<ResolvedDiskBounds>(equal).is_err());
        let inverted = r#"{"inner_radius":20.0,"outer_radius":3.0}"#;
        assert!(serde_json::from_str::<ResolvedDiskBounds>(inverted).is_err());
    }

    #[test]
    fn emission_provenance_exact_match_required() {
        assert!(validate_disk_emission_provenance(
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .is_ok());
        assert!(matches!(
            validate_disk_emission_provenance(
                "unsupported_emission_model_x",
                CANONICAL_DISK_EMISSION_CLAIM
            ),
            Err(BolometricRenderError::UnsupportedEmissionModel(_))
        ));
        assert!(matches!(
            validate_disk_emission_provenance(
                CANONICAL_DISK_EMISSION_MODEL,
                "project-diagnostic-not-astrophysical-or-film-reconstruction"
            ),
            Err(BolometricRenderError::UnsupportedEmissionClaim(_))
        ));
        assert!(matches!(
            validate_disk_emission_provenance(
                CANONICAL_DISK_EMISSION_MODEL,
                "astrophysical reconstruction"
            ),
            Err(BolometricRenderError::UnsupportedEmissionClaim(_))
        ));
    }

    #[test]
    fn accepted_claim_is_hashed_into_scientific_digest() {
        let grid = TraceGrid {
            width: 1,
            height: 1,
        };
        let frame = DiskFrequencyShiftFrame::try_new(
            grid,
            vec![DiskFrequencyShiftPixel::DiskHit(fs_sample(6.0, 1.0))],
        )
        .unwrap();
        let spec = diagnostic_bolometric_emission_v1();
        let b = bounds_3_20();
        let bolo = build_disk_bolometric_frame(&frame, &spec, b).unwrap();
        let d_canon = disk_bolometric_digest(
            &bolo,
            &DiskBolometricConvention::v1(),
            &spec,
            b,
            "src",
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .unwrap();
        let d_alt = disk_bolometric_digest(
            &bolo,
            &DiskBolometricConvention::v1(),
            &spec,
            b,
            "src",
            CANONICAL_DISK_EMISSION_MODEL,
            "altered claim",
        )
        .unwrap();
        assert_ne!(d_canon, d_alt);
    }
}
