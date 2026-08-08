//! Physical thin-disk emission frame: Page–Thorne `F`, `T_eff` (Gate 2C0).
//!
//! Does not mutate diagnostic bolometric / SpectralFrame V1 authorities.

use crate::bolometric::ResolvedDiskBounds;
use crate::error::BolometricRenderError;
use crate::frequency_shift::{DiskFrequencyShiftFrame, DiskFrequencyShiftPixel, DiskVelocityModel};
use crate::page_thorne::{page_thorne_one_face_flux, FACE_POLICY, FLUX_MODEL_ID};
use crate::planck::{teff_from_one_face_flux, TEMPERATURE_MODEL_ID};
use relativity_core::{
    prograde_isco_radius, FluxWPerM2, KerrParams, MdotKgPerS, PhysicalScale, TemperatureKelvin,
    CONSTANTS_REVISION,
};
use relativity_trace::{hex_sha, pixel_index, OutcomeClass, TraceGrid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PHYSICAL_DISK_EMISSION_CONVENTION_ID: &str = "physical-disk-emission-v1";
pub const PHYSICAL_EMISSION_MODEL_ID: &str = "page-thorne-blackbody-v1";
pub const PHYSICAL_EMISSION_CLAIM: &str =
    "project physical demonstration, not film/DNGR reconstruction";
pub const PHYSICAL_FLUX_UNITS: &str = "W_m^-2_one_face";
pub const PHYSICAL_TEFF_UNITS: &str = "K";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicalDiskEmissionSpec {
    pub schema_version: u32,
    pub emission_model_id: String,
    pub flux_model_id: String,
    pub temperature_model_id: String,
    pub face_policy: String,
    pub constants_revision: String,
    pub solar_masses: f64,
    pub mdot_kg_s: f64,
    pub emission_claim: String,
}

impl PhysicalDiskEmissionSpec {
    pub fn page_thorne_blackbody_v1(
        solar_masses: f64,
        mdot_kg_s: f64,
    ) -> Result<Self, BolometricRenderError> {
        if !solar_masses.is_finite() || !(solar_masses > 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "solar_masses must be finite and > 0".into(),
            ));
        }
        if !mdot_kg_s.is_finite() || mdot_kg_s < 0.0 {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "mdot_kg_s must be finite and >= 0".into(),
            ));
        }
        Ok(Self {
            schema_version: 1,
            emission_model_id: PHYSICAL_EMISSION_MODEL_ID.into(),
            flux_model_id: FLUX_MODEL_ID.into(),
            temperature_model_id: TEMPERATURE_MODEL_ID.into(),
            face_policy: FACE_POLICY.into(),
            constants_revision: CONSTANTS_REVISION.into(),
            solar_masses,
            mdot_kg_s,
            emission_claim: PHYSICAL_EMISSION_CLAIM.into(),
        })
    }

    pub fn validate(&self) -> Result<(), BolometricRenderError> {
        let canon = Self::page_thorne_blackbody_v1(self.solar_masses, self.mdot_kg_s)?;
        if self.emission_model_id != canon.emission_model_id
            || self.flux_model_id != canon.flux_model_id
            || self.temperature_model_id != canon.temperature_model_id
            || self.face_policy != canon.face_policy
            || self.constants_revision != canon.constants_revision
            || self.emission_claim != canon.emission_claim
            || self.schema_version != 1
        {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "non-canonical physical disk emission spec fields".into(),
            ));
        }
        Ok(())
    }

    pub fn scale(&self) -> Result<PhysicalScale, BolometricRenderError> {
        PhysicalScale::from_solar_masses(self.solar_masses)
            .map_err(|e| BolometricRenderError::InvalidEmissionSpec(e.to_string()))
    }

    pub fn mdot(&self) -> Result<MdotKgPerS, BolometricRenderError> {
        MdotKgPerS::new(self.mdot_kg_s)
            .map_err(|e| BolometricRenderError::InvalidEmissionSpec(e.to_string()))
    }
}

pub fn validate_physical_emission_provenance(
    emission_model: &str,
    emission_claim: &str,
) -> Result<(), BolometricRenderError> {
    if emission_model != PHYSICAL_EMISSION_MODEL_ID {
        return Err(BolometricRenderError::UnsupportedEmissionModel(
            emission_model.into(),
        ));
    }
    if emission_claim != PHYSICAL_EMISSION_CLAIM {
        return Err(BolometricRenderError::UnsupportedEmissionClaim(
            emission_claim.into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicalDiskEmissionConvention {
    pub schema_version: u32,
    pub convention_id: String,
    pub flux_units: String,
    pub teff_units: String,
    pub velocity_model: String,
    pub spin_policy: String,
    pub absence_policy: String,
}

impl PhysicalDiskEmissionConvention {
    pub fn v1() -> Self {
        Self {
            schema_version: 1,
            convention_id: PHYSICAL_DISK_EMISSION_CONVENTION_ID.into(),
            flux_units: PHYSICAL_FLUX_UNITS.into(),
            teff_units: PHYSICAL_TEFF_UNITS.into(),
            velocity_model: "prograde-circular-geodesic".into(),
            spin_policy: "prograde-only-typed-reject-retrograde".into(),
            absence_policy: "outside-annulus-or-inside-isco-is-absence-not-clamp".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalDiskEmissionSample {
    pub radius_over_m: f64,
    pub radius_m: f64,
    pub azimuth: f64,
    pub g_factor: f64,
    pub f_one_face_w_m2: f64,
    pub t_eff_k: f64,
    pub inside_isco: bool,
}

impl PhysicalDiskEmissionSample {
    pub fn validate(&self) -> Result<(), BolometricRenderError> {
        if !self.radius_over_m.is_finite() || !(self.radius_over_m > 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "radius_over_m must be finite and > 0".into(),
            ));
        }
        if !self.radius_m.is_finite() || self.radius_m < 0.0 {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "radius_m must be finite and >= 0".into(),
            ));
        }
        if !self.azimuth.is_finite() {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "azimuth must be finite".into(),
            ));
        }
        if !self.g_factor.is_finite() || !(self.g_factor > 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "g_factor must be finite and > 0".into(),
            ));
        }
        if !self.f_one_face_w_m2.is_finite() || self.f_one_face_w_m2 < 0.0 {
            return Err(BolometricRenderError::InvalidIntensity(
                "f_one_face must be finite and >= 0".into(),
            ));
        }
        if !self.t_eff_k.is_finite() || self.t_eff_k < 0.0 {
            return Err(BolometricRenderError::InvalidIntensity(
                "t_eff must be finite and >= 0".into(),
            ));
        }
        if self.f_one_face_w_m2 > 0.0 && !(self.t_eff_k > 0.0) {
            return Err(BolometricRenderError::InvalidIntensity(
                "F>0 requires T_eff>0".into(),
            ));
        }
        if self.inside_isco && self.f_one_face_w_m2 > 0.0 {
            return Err(BolometricRenderError::InvalidIntensity(
                "inside_isco samples cannot carry positive flux".into(),
            ));
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for PhysicalDiskEmissionSample {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            radius_over_m: f64,
            radius_m: f64,
            azimuth: f64,
            g_factor: f64,
            f_one_face_w_m2: f64,
            t_eff_k: f64,
            inside_isco: bool,
        }
        let raw = Raw::deserialize(deserializer)?;
        let sample = Self {
            radius_over_m: raw.radius_over_m,
            radius_m: raw.radius_m,
            azimuth: raw.azimuth,
            g_factor: raw.g_factor,
            f_one_face_w_m2: raw.f_one_face_w_m2,
            t_eff_k: raw.t_eff_k,
            inside_isco: raw.inside_isco,
        };
        sample.validate().map_err(serde::de::Error::custom)?;
        Ok(sample)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PhysicalDiskEmissionPixel {
    DiskHit(PhysicalDiskEmissionSample),
    NotDiskHit { outcome_class: OutcomeClass },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalDiskEmissionFrame {
    pub grid: TraceGrid,
    pub pixels: Vec<PhysicalDiskEmissionPixel>,
    pub r_isco_over_m: f64,
    pub bounds: ResolvedDiskBounds,
}

impl PhysicalDiskEmissionFrame {
    pub fn try_new(
        grid: TraceGrid,
        pixels: Vec<PhysicalDiskEmissionPixel>,
        r_isco_over_m: f64,
        bounds: ResolvedDiskBounds,
    ) -> Result<Self, BolometricRenderError> {
        if pixels.len() != grid.pixel_count() {
            return Err(BolometricRenderError::FrameLengthMismatch);
        }
        bounds.validate()?;
        if !r_isco_over_m.is_finite() || !(r_isco_over_m > 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "r_isco/M must be finite and > 0".into(),
            ));
        }
        for pix in &pixels {
            if let PhysicalDiskEmissionPixel::DiskHit(s) = pix {
                s.validate()?;
            }
        }
        Ok(Self {
            grid,
            pixels,
            r_isco_over_m,
            bounds,
        })
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &PhysicalDiskEmissionPixel {
        &self.pixels[pixel_index(self.grid, col, row)]
    }
}

impl<'de> Deserialize<'de> for PhysicalDiskEmissionFrame {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            grid: TraceGrid,
            pixels: Vec<PhysicalDiskEmissionPixel>,
            r_isco_over_m: f64,
            bounds: ResolvedDiskBounds,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::try_new(raw.grid, raw.pixels, raw.r_isco_over_m, raw.bounds)
            .map_err(serde::de::Error::custom)
    }
}

/// Sample one-face flux + T_eff at a geometrized radius (absence if ≤ ISCO).
pub fn sample_physical_disk_emission(
    params: &KerrParams,
    scale: &PhysicalScale,
    mdot: MdotKgPerS,
    bounds: ResolvedDiskBounds,
    radius_over_m: f64,
) -> Result<Option<(FluxWPerM2, TemperatureKelvin)>, BolometricRenderError> {
    if !bounds.contains(radius_over_m) {
        return Ok(None);
    }
    let r_isco = prograde_isco_radius(params)
        .map_err(|e| BolometricRenderError::InvalidEmissionSpec(e.to_string()))?;
    let r_isco_over_m = r_isco / params.mass();
    if !(radius_over_m > r_isco_over_m) {
        return Ok(None);
    }
    let f = page_thorne_one_face_flux(scale, mdot, params, radius_over_m)?;
    let t = teff_from_one_face_flux(f)?;
    Ok(Some((f, t)))
}

pub fn build_physical_disk_emission_frame(
    params: &KerrParams,
    frequency_frame: &DiskFrequencyShiftFrame,
    spec: &PhysicalDiskEmissionSpec,
    bounds: ResolvedDiskBounds,
) -> Result<PhysicalDiskEmissionFrame, BolometricRenderError> {
    spec.validate()?;
    bounds.validate()?;
    if !matches!(
        frequency_frame
            .pixels()
            .iter()
            .find_map(|p| match p {
                DiskFrequencyShiftPixel::DiskHit(s) => Some(s.velocity_model),
                _ => None,
            })
            .unwrap_or(DiskVelocityModel::ProgradeCircularGeodesic),
        DiskVelocityModel::ProgradeCircularGeodesic
    ) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "physical emission requires prograde circular geodesic velocity model".into(),
        ));
    }
    let scale = spec.scale()?;
    let mdot = spec.mdot()?;
    let r_isco = prograde_isco_radius(params)
        .map_err(|e| BolometricRenderError::InvalidEmissionSpec(e.to_string()))?;
    let r_isco_over_m = r_isco / params.mass();
    let grid = frequency_frame.grid();
    let mut pixels = Vec::with_capacity(grid.pixel_count());
    for row in 0..grid.height {
        for col in 0..grid.width {
            let pixel = match frequency_frame.pixel_at(col, row) {
                DiskFrequencyShiftPixel::DiskHit(fs) => {
                    if fs.velocity_model != DiskVelocityModel::ProgradeCircularGeodesic {
                        return Err(BolometricRenderError::PixelMappingFailed {
                            col,
                            row,
                            cause: "non-prograde velocity model".into(),
                        });
                    }
                    let (f_val, t_val, inside) = match sample_physical_disk_emission(
                        params, &scale, mdot, bounds, fs.radius,
                    )? {
                        Some((f, t)) => (f.value(), t.value(), false),
                        None => (0.0, 0.0, fs.radius <= r_isco_over_m),
                    };
                    PhysicalDiskEmissionPixel::DiskHit(PhysicalDiskEmissionSample {
                        radius_over_m: fs.radius,
                        radius_m: scale.gravitational_radius_m * fs.radius,
                        azimuth: fs.azimuth,
                        g_factor: fs.g_factor,
                        f_one_face_w_m2: f_val,
                        t_eff_k: t_val,
                        inside_isco: inside,
                    })
                }
                DiskFrequencyShiftPixel::NotDiskHit { outcome_class } => {
                    PhysicalDiskEmissionPixel::NotDiskHit {
                        outcome_class: *outcome_class,
                    }
                }
            };
            pixels.push(pixel);
        }
    }
    PhysicalDiskEmissionFrame::try_new(grid, pixels, r_isco_over_m, bounds)
}

pub fn physical_disk_emission_spec_digest(spec: &PhysicalDiskEmissionSpec) -> String {
    let mut h = Sha256::new();
    h.update(b"physical-disk-emission-spec-digest-v1");
    h.update(spec.schema_version.to_le_bytes());
    h.update(spec.emission_model_id.as_bytes());
    h.update(spec.flux_model_id.as_bytes());
    h.update(spec.temperature_model_id.as_bytes());
    h.update(spec.face_policy.as_bytes());
    h.update(spec.constants_revision.as_bytes());
    h.update(spec.solar_masses.to_bits().to_le_bytes());
    h.update(spec.mdot_kg_s.to_bits().to_le_bytes());
    h.update(spec.emission_claim.as_bytes());
    hex_sha(&h.finalize())
}

pub fn physical_disk_emission_digest(
    frame: &PhysicalDiskEmissionFrame,
    convention: &PhysicalDiskEmissionConvention,
    spec: &PhysicalDiskEmissionSpec,
    frequency_shift_digest: &str,
) -> Result<String, BolometricRenderError> {
    spec.validate()?;
    for pix in &frame.pixels {
        if let PhysicalDiskEmissionPixel::DiskHit(s) = pix {
            s.validate()?;
        }
    }
    let mut h = Sha256::new();
    h.update(b"physical-disk-emission-digest-v1");
    h.update(convention.convention_id.as_bytes());
    h.update(physical_disk_emission_spec_digest(spec).as_bytes());
    h.update(frequency_shift_digest.as_bytes());
    h.update(frame.r_isco_over_m.to_bits().to_le_bytes());
    h.update(frame.bounds.inner_radius().to_bits().to_le_bytes());
    h.update(frame.bounds.outer_radius().to_bits().to_le_bytes());
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for pix in &frame.pixels {
        match pix {
            PhysicalDiskEmissionPixel::DiskHit(s) => {
                h.update([1u8]);
                h.update(s.radius_over_m.to_bits().to_le_bytes());
                h.update(s.azimuth.to_bits().to_le_bytes());
                h.update(s.g_factor.to_bits().to_le_bytes());
                h.update(s.f_one_face_w_m2.to_bits().to_le_bytes());
                h.update(s.t_eff_k.to_bits().to_le_bytes());
            }
            PhysicalDiskEmissionPixel::NotDiskHit { outcome_class } => {
                h.update([0u8]);
                h.update(outcome_class.digest_tag().as_bytes());
            }
        }
    }
    Ok(hex_sha(&h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_digest_stable() {
        let a = PhysicalDiskEmissionSpec::page_thorne_blackbody_v1(1.0e8, 1.0e18).unwrap();
        let b = PhysicalDiskEmissionSpec::page_thorne_blackbody_v1(1.0e8, 1.0e18).unwrap();
        assert_eq!(
            physical_disk_emission_spec_digest(&a),
            physical_disk_emission_spec_digest(&b)
        );
    }

    #[test]
    fn sample_absent_outside_bounds() {
        let k = KerrParams::new(1.0, 0.999).unwrap();
        let scale = PhysicalScale::from_solar_masses(1.0e8).unwrap();
        let mdot = MdotKgPerS::new(1.0e18).unwrap();
        let bounds = ResolvedDiskBounds::new(3.0, 20.0).unwrap();
        assert!(
            sample_physical_disk_emission(&k, &scale, mdot, bounds, 25.0)
                .unwrap()
                .is_none()
        );
    }
}
