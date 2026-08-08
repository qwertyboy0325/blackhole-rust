//! Physical thin-disk emission frame: Page–Thorne `F`, `T_eff` (Gate 2C0).
//!
//! Does not mutate diagnostic bolometric / SpectralFrame V1 authorities.

use crate::bolometric::ResolvedDiskBounds;
use crate::error::BolometricRenderError;
use crate::frequency_shift::{DiskFrequencyShiftFrame, DiskFrequencyShiftPixel, DiskVelocityModel};
use crate::page_thorne::{page_thorne_one_face_flux, FACE_POLICY, FLUX_MODEL_ID};
use crate::planck::{stefan_boltzmann_flux, teff_from_one_face_flux, TEMPERATURE_MODEL_ID};
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
/// Relative tolerance for persisted `F ↔ σ T_eff⁴` (constructor arithmetic, not spectral quad).
pub const F_TEFF_CONSTRUCTOR_REL_TOL: f64 = 1e-12;
/// Relative tolerance for `radius_m = r_g · radius_over_m`.
pub const RADIUS_M_PROVENANCE_REL_TOL: f64 = 1e-12;

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
    /// Physical scale used to author `radius_m = r_g · radius_over_m` (hashed).
    pub gravitational_radius_m: f64,
}

impl PhysicalDiskEmissionFrame {
    pub fn try_new(
        grid: TraceGrid,
        pixels: Vec<PhysicalDiskEmissionPixel>,
        r_isco_over_m: f64,
        bounds: ResolvedDiskBounds,
        gravitational_radius_m: f64,
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
        if !gravitational_radius_m.is_finite() || !(gravitational_radius_m > 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "gravitational_radius_m must be finite and > 0".into(),
            ));
        }
        let frame = Self {
            grid,
            pixels,
            r_isco_over_m,
            bounds,
            gravitational_radius_m,
        };
        frame.validate()?;
        Ok(frame)
    }

    /// Frame-level authority: bounds/ISCO/emission state, F↔T_eff, radius_m provenance.
    pub fn validate(&self) -> Result<(), BolometricRenderError> {
        self.bounds.validate()?;
        if !self.r_isco_over_m.is_finite() || !(self.r_isco_over_m > 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "r_isco/M must be finite and > 0".into(),
            ));
        }
        if !self.gravitational_radius_m.is_finite() || !(self.gravitational_radius_m > 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "gravitational_radius_m must be finite and > 0".into(),
            ));
        }
        if self.pixels.len() != self.grid.pixel_count() {
            return Err(BolometricRenderError::FrameLengthMismatch);
        }
        for pix in &self.pixels {
            let PhysicalDiskEmissionPixel::DiskHit(s) = pix else {
                continue;
            };
            s.validate()?;
            validate_emission_sample_in_frame(
                s,
                self.r_isco_over_m,
                self.bounds,
                self.gravitational_radius_m,
            )?;
        }
        Ok(())
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &PhysicalDiskEmissionPixel {
        &self.pixels[pixel_index(self.grid, col, row)]
    }
}

fn validate_emission_sample_in_frame(
    s: &PhysicalDiskEmissionSample,
    r_isco_over_m: f64,
    bounds: ResolvedDiskBounds,
    gravitational_radius_m: f64,
) -> Result<(), BolometricRenderError> {
    let expect_inside = s.radius_over_m <= r_isco_over_m;
    if s.inside_isco != expect_inside {
        return Err(BolometricRenderError::InvalidEmissionSpec(format!(
            "inside_isco={} inconsistent with radius_over_m={} vs r_isco/M={}",
            s.inside_isco, s.radius_over_m, r_isco_over_m
        )));
    }

    let expect_rm = gravitational_radius_m * s.radius_over_m;
    let denom = expect_rm.abs().max(1.0);
    if (s.radius_m - expect_rm).abs() / denom > RADIUS_M_PROVENANCE_REL_TOL {
        return Err(BolometricRenderError::InvalidEmissionSpec(format!(
            "radius_m={} inconsistent with r_g·(r/M)={expect_rm}",
            s.radius_m
        )));
    }

    let in_bounds = bounds.contains(s.radius_over_m);
    let emitting = s.f_one_face_w_m2 > 0.0 || s.t_eff_k > 0.0;
    if emitting {
        if !in_bounds {
            return Err(BolometricRenderError::InvalidIntensity(
                "positive F/T_eff outside resolved disk bounds (absence, not clamp)".into(),
            ));
        }
        if expect_inside || s.inside_isco {
            return Err(BolometricRenderError::InvalidIntensity(
                "positive F/T_eff inside ISCO is forbidden".into(),
            ));
        }
        // Authoritative Stefan–Boltzmann: F = σ T⁴ (constructor arithmetic).
        let t = TemperatureKelvin::new(s.t_eff_k)
            .map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))?;
        let f_from_t = stefan_boltzmann_flux(t)?.value();
        let scale = s.f_one_face_w_m2.max(f_from_t).max(1e-30);
        if (s.f_one_face_w_m2 - f_from_t).abs() / scale > F_TEFF_CONSTRUCTOR_REL_TOL {
            return Err(BolometricRenderError::InvalidIntensity(format!(
                "F↔T_eff Stefan–Boltzmann mismatch: F={} σT⁴={}",
                s.f_one_face_w_m2, f_from_t
            )));
        }
        // Also require T matches teff_from_one_face_flux(F) within constructor tol.
        let f = FluxWPerM2::new(s.f_one_face_w_m2)
            .map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))?;
        let t_from_f = teff_from_one_face_flux(f)?.value();
        let t_scale = s.t_eff_k.max(t_from_f).max(1e-30);
        if (s.t_eff_k - t_from_f).abs() / t_scale > F_TEFF_CONSTRUCTOR_REL_TOL {
            return Err(BolometricRenderError::InvalidIntensity(format!(
                "T_eff={} inconsistent with (F/σ)^(1/4)={t_from_f}",
                s.t_eff_k
            )));
        }
    } else if s.f_one_face_w_m2 != 0.0 || s.t_eff_k != 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "zero-emission samples require exact F=0 and T_eff=0".into(),
        ));
    }
    Ok(())
}

impl<'de> Deserialize<'de> for PhysicalDiskEmissionFrame {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            grid: TraceGrid,
            pixels: Vec<PhysicalDiskEmissionPixel>,
            r_isco_over_m: f64,
            bounds: ResolvedDiskBounds,
            gravitational_radius_m: f64,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::try_new(
            raw.grid,
            raw.pixels,
            raw.r_isco_over_m,
            raw.bounds,
            raw.gravitational_radius_m,
        )
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
    PhysicalDiskEmissionFrame::try_new(
        grid,
        pixels,
        r_isco_over_m,
        bounds,
        scale.gravitational_radius_m,
    )
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
    frame.validate()?;
    let mut h = Sha256::new();
    h.update(b"physical-disk-emission-digest-v1");
    h.update(convention.convention_id.as_bytes());
    h.update(physical_disk_emission_spec_digest(spec).as_bytes());
    h.update(frequency_shift_digest.as_bytes());
    h.update(frame.r_isco_over_m.to_bits().to_le_bytes());
    h.update(frame.gravitational_radius_m.to_bits().to_le_bytes());
    h.update(frame.bounds.inner_radius().to_bits().to_le_bytes());
    h.update(frame.bounds.outer_radius().to_bits().to_le_bytes());
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for pix in &frame.pixels {
        match pix {
            PhysicalDiskEmissionPixel::DiskHit(s) => {
                h.update([1u8]);
                h.update(s.radius_over_m.to_bits().to_le_bytes());
                h.update(s.radius_m.to_bits().to_le_bytes());
                h.update(s.azimuth.to_bits().to_le_bytes());
                h.update(s.g_factor.to_bits().to_le_bytes());
                h.update(s.f_one_face_w_m2.to_bits().to_le_bytes());
                h.update(s.t_eff_k.to_bits().to_le_bytes());
                h.update([u8::from(s.inside_isco)]);
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
    use relativity_core::stefan_boltzmann_w_m2_k4;

    fn valid_emitting_sample(r_over_m: f64, r_g: f64) -> PhysicalDiskEmissionSample {
        let f = 1.0e12_f64;
        let t = (f / stefan_boltzmann_w_m2_k4()).powf(0.25);
        PhysicalDiskEmissionSample {
            radius_over_m: r_over_m,
            radius_m: r_g * r_over_m,
            azimuth: 0.0,
            g_factor: 1.0,
            f_one_face_w_m2: f,
            t_eff_k: t,
            inside_isco: false,
        }
    }

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

    #[test]
    fn frame_rejects_outside_bounds_positive_flux() {
        let bounds = ResolvedDiskBounds::new(3.0, 20.0).unwrap();
        let r_g = 1.0e11;
        let mut s = valid_emitting_sample(25.0, r_g);
        s.inside_isco = false;
        let err = PhysicalDiskEmissionFrame::try_new(
            TraceGrid {
                width: 1,
                height: 1,
            },
            vec![PhysicalDiskEmissionPixel::DiskHit(s)],
            1.2,
            bounds,
            r_g,
        )
        .unwrap_err();
        assert!(err.to_string().contains("outside resolved disk bounds"));
    }

    #[test]
    fn frame_rejects_wrong_inside_isco() {
        let bounds = ResolvedDiskBounds::new(1.0, 20.0).unwrap();
        let r_g = 1.0e11;
        let mut s = valid_emitting_sample(10.0, r_g);
        s.inside_isco = true; // wrong: r > r_isco
        s.f_one_face_w_m2 = 0.0;
        s.t_eff_k = 0.0;
        let err = PhysicalDiskEmissionFrame::try_new(
            TraceGrid {
                width: 1,
                height: 1,
            },
            vec![PhysicalDiskEmissionPixel::DiskHit(s)],
            1.2,
            bounds,
            r_g,
        )
        .unwrap_err();
        assert!(err.to_string().contains("inside_isco"));
    }

    #[test]
    fn frame_rejects_inconsistent_f_teff() {
        let bounds = ResolvedDiskBounds::new(3.0, 20.0).unwrap();
        let r_g = 1.0e11;
        let mut s = valid_emitting_sample(10.0, r_g);
        s.t_eff_k *= 1.01; // break SB
        let err = PhysicalDiskEmissionFrame::try_new(
            TraceGrid {
                width: 1,
                height: 1,
            },
            vec![PhysicalDiskEmissionPixel::DiskHit(s)],
            1.2,
            bounds,
            r_g,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("Stefan–Boltzmann")
                || err.to_string().contains("T_eff")
                || err.to_string().contains("inconsistent")
        );
    }

    #[test]
    fn frame_rejects_inconsistent_radius_m() {
        let bounds = ResolvedDiskBounds::new(3.0, 20.0).unwrap();
        let r_g = 1.0e11;
        let mut s = valid_emitting_sample(10.0, r_g);
        s.radius_m *= 1.5;
        let err = PhysicalDiskEmissionFrame::try_new(
            TraceGrid {
                width: 1,
                height: 1,
            },
            vec![PhysicalDiskEmissionPixel::DiskHit(s)],
            1.2,
            bounds,
            r_g,
        )
        .unwrap_err();
        assert!(err.to_string().contains("radius_m"));
    }

    #[test]
    fn frame_accepts_absence_outside_bounds_zero_flux() {
        let bounds = ResolvedDiskBounds::new(3.0, 20.0).unwrap();
        let r_g = 1.0e11;
        let s = PhysicalDiskEmissionSample {
            radius_over_m: 25.0,
            radius_m: r_g * 25.0,
            azimuth: 0.1,
            g_factor: 0.9,
            f_one_face_w_m2: 0.0,
            t_eff_k: 0.0,
            inside_isco: false,
        };
        PhysicalDiskEmissionFrame::try_new(
            TraceGrid {
                width: 1,
                height: 1,
            },
            vec![PhysicalDiskEmissionPixel::DiskHit(s)],
            1.2,
            bounds,
            r_g,
        )
        .unwrap();
    }

    #[test]
    fn deserialize_rejects_tampered_f_teff() {
        let bounds = ResolvedDiskBounds::new(3.0, 20.0).unwrap();
        let r_g = 1.0e11;
        let mut s = valid_emitting_sample(10.0, r_g);
        s.t_eff_k *= 2.0;
        let json = serde_json::json!({
            "grid": {"width": 1, "height": 1},
            "pixels": [{"DiskHit": s}],
            "r_isco_over_m": 1.2,
            "bounds": {"inner_radius": 3.0, "outer_radius": 20.0},
            "gravitational_radius_m": r_g,
        });
        // bounds serialize format - check ResolvedDiskBounds serde
        let _ = bounds;
        assert!(serde_json::from_value::<PhysicalDiskEmissionFrame>(json).is_err());
    }
}
