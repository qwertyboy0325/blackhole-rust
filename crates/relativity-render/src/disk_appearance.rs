//! Gate 2D1 derived disk appearance (PHYSICALLY_MOTIVATED_APPEARANCE).
//!
//! Does **not** mutate Gate 2C0 `PhysicalDiskEmissionFrame`. Modulation is
//! `ANNULAR_APPEARANCE_MEAN_PRESERVING` — not luminosity/energy conservation.

use crate::color_space::{SceneLinearRgb, XyzToRgbMatrix};
use crate::colorimetry::{
    integrate_xyz_from_emission, outcome_class_code, Cie1931Table, ColorimetricXyz,
    IntegrationMeasure,
};
use crate::error::AppearanceError;
use crate::physical_disk::{
    PhysicalDiskEmissionFrame, PhysicalDiskEmissionPixel, PhysicalDiskEmissionSample,
};
use crate::planck::teff_from_one_face_flux;
use relativity_trace::hex_sha;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DISK_APPEARANCE_MODEL_ID: &str = "spiral-harmonic-flux-modulation-v1";
pub const RADIAL_ENVELOPE_ID: &str = "raised-cosine-radial-envelope-v1";
pub const MEAN_PRESERVATION_CLAIM: &str = "ANNULAR_APPEARANCE_MEAN_PRESERVING";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpiralHarmonicMode {
    pub m: u32,
    pub weight: f64,
    pub k_log: f64,
    pub phase: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskAppearanceSpec {
    pub model_id: String,
    pub radial_envelope_id: String,
    pub mean_preservation_claim: String,
    pub a_max: f64,
    pub r_ref_over_m: f64,
    pub modes: Vec<SpiralHarmonicMode>,
    /// When true, force `m(r,φ)=1` (identity differential / Gate 2D0 path).
    pub identity_modulation: bool,
}

impl DiskAppearanceSpec {
    pub fn validate(&self) -> Result<(), AppearanceError> {
        if self.model_id != DISK_APPEARANCE_MODEL_ID {
            return Err(AppearanceError::InvalidSpec(format!(
                "unsupported disk appearance model {}",
                self.model_id
            )));
        }
        if self.radial_envelope_id != RADIAL_ENVELOPE_ID {
            return Err(AppearanceError::InvalidSpec(format!(
                "unsupported radial envelope {}",
                self.radial_envelope_id
            )));
        }
        if self.mean_preservation_claim != MEAN_PRESERVATION_CLAIM {
            return Err(AppearanceError::InvalidSpec(format!(
                "mean claim must be {MEAN_PRESERVATION_CLAIM}"
            )));
        }
        if !self.a_max.is_finite() || !(0.0..1.0).contains(&self.a_max) {
            return Err(AppearanceError::InvalidSpec(
                "a_max must be finite and in [0, 1)".into(),
            ));
        }
        if !self.r_ref_over_m.is_finite() || !(self.r_ref_over_m > 0.0) {
            return Err(AppearanceError::InvalidSpec(
                "r_ref_over_m must be finite and > 0".into(),
            ));
        }
        if self.modes.is_empty() && !self.identity_modulation {
            return Err(AppearanceError::InvalidSpec(
                "modes must be non-empty unless identity_modulation".into(),
            ));
        }
        let mut wsum = 0.0;
        for mode in &self.modes {
            if mode.m < 1 {
                return Err(AppearanceError::InvalidSpec("mode m must be >= 1".into()));
            }
            if !mode.weight.is_finite() || !mode.k_log.is_finite() || !mode.phase.is_finite() {
                return Err(AppearanceError::InvalidSpec(
                    "mode parameters must be finite".into(),
                ));
            }
            wsum += mode.weight.abs();
        }
        if wsum > 1.0 + 1e-12 {
            return Err(AppearanceError::InvalidSpec(format!(
                "Σ|w_j| must be <= 1 (got {wsum})"
            )));
        }
        Ok(())
    }
}

/// Raised-cosine radial envelope (A2): `A(r_inner)=A(r_outer)=0`, peak mid-annulus.
pub fn radial_amplitude_a(
    r_over_m: f64,
    r_inner: f64,
    r_outer: f64,
    a_max: f64,
) -> Result<f64, AppearanceError> {
    if !(r_over_m.is_finite() && r_inner.is_finite() && r_outer.is_finite() && a_max.is_finite()) {
        return Err(AppearanceError::NonFinite("radial_amplitude inputs".into()));
    }
    if !(r_outer > r_inner) {
        return Err(AppearanceError::InvalidSpec(
            "r_outer must be > r_inner".into(),
        ));
    }
    if !(0.0..1.0).contains(&a_max) {
        return Err(AppearanceError::InvalidSpec(
            "a_max must be in [0, 1)".into(),
        ));
    }
    let u = ((r_over_m - r_inner) / (r_outer - r_inner)).clamp(0.0, 1.0);
    let envelope = (std::f64::consts::PI * u).sin().powi(2);
    let a = a_max * envelope;
    if !a.is_finite() || !(0.0..1.0).contains(&a) {
        return Err(AppearanceError::NonFinite(format!("A(r)={a}")));
    }
    Ok(a)
}

/// Mean-preserving spiral-harmonic modulation factor.
pub fn modulation_factor(
    spec: &DiskAppearanceSpec,
    r_over_m: f64,
    azimuth: f64,
    r_inner: f64,
    r_outer: f64,
) -> Result<f64, AppearanceError> {
    spec.validate()?;
    if spec.identity_modulation || spec.a_max == 0.0 {
        return Ok(1.0);
    }
    if !azimuth.is_finite() || !r_over_m.is_finite() {
        return Err(AppearanceError::NonFinite("r/azimuth".into()));
    }
    let a = radial_amplitude_a(r_over_m, r_inner, r_outer, spec.a_max)?;
    let ln = (r_over_m / spec.r_ref_over_m).ln();
    if !ln.is_finite() {
        return Err(AppearanceError::NonFinite("ln(r/r_ref)".into()));
    }
    let mut s = 0.0;
    for mode in &spec.modes {
        let arg = f64::from(mode.m) * azimuth + mode.k_log * ln + mode.phase;
        s += mode.weight * arg.cos();
    }
    let m = 1.0 + a * s;
    if !m.is_finite() || !(m > 0.0) {
        return Err(AppearanceError::NonFinite(format!("modulation m={m}")));
    }
    Ok(m)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppearanceDiskEmissionSample {
    pub radius_over_m: f64,
    pub azimuth: f64,
    pub g_factor: f64,
    pub f_base_w_m2: f64,
    pub t_base_k: f64,
    pub modulation: f64,
    pub f_app_w_m2: f64,
    pub t_app_k: f64,
    pub inside_isco: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AppearanceDiskEmissionPixel {
    DiskHit(AppearanceDiskEmissionSample),
    NotDiskHit {
        outcome_class: relativity_trace::OutcomeClass,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppearanceDiskEmissionFrame {
    pub grid: relativity_trace::TraceGrid,
    pub pixels: Vec<AppearanceDiskEmissionPixel>,
    pub source_physical_emission_digest: String,
    pub disk_appearance_spec_digest: String,
    pub r_inner: f64,
    pub r_outer: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppearanceDiskColorSample {
    pub xyz: ColorimetricXyz,
    pub rgb: SceneLinearRgb,
    pub g_factor: f64,
    pub f_app_w_m2: f64,
    pub t_app_k: f64,
    pub modulation: f64,
    pub radius_over_m: f64,
    pub azimuth: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AppearanceDiskColorPixel {
    DiskHit(AppearanceDiskColorSample),
    Absent {
        outcome_class: relativity_trace::OutcomeClass,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppearanceDiskColorFrame {
    pub grid: relativity_trace::TraceGrid,
    pub pixels: Vec<AppearanceDiskColorPixel>,
    pub source_physical_emission_digest: String,
    pub disk_appearance_spec_digest: String,
}

pub fn disk_appearance_spec_digest(spec: &DiskAppearanceSpec) -> Result<String, AppearanceError> {
    spec.validate()?;
    let mut h = Sha256::new();
    h.update(b"disk-appearance-spec-digest-v1");
    h.update(b"APPEARANCE_REPRODUCIBILITY_DIGEST");
    h.update(spec.model_id.as_bytes());
    h.update(spec.radial_envelope_id.as_bytes());
    h.update(spec.mean_preservation_claim.as_bytes());
    h.update(spec.a_max.to_bits().to_le_bytes());
    h.update(spec.r_ref_over_m.to_bits().to_le_bytes());
    h.update([u8::from(spec.identity_modulation)]);
    h.update((spec.modes.len() as u64).to_le_bytes());
    for mode in &spec.modes {
        h.update(mode.m.to_le_bytes());
        h.update(mode.weight.to_bits().to_le_bytes());
        h.update(mode.k_log.to_bits().to_le_bytes());
        h.update(mode.phase.to_bits().to_le_bytes());
    }
    Ok(hex_sha(&h.finalize()))
}

fn derive_sample(
    base: &PhysicalDiskEmissionSample,
    spec: &DiskAppearanceSpec,
    r_inner: f64,
    r_outer: f64,
) -> Result<AppearanceDiskEmissionSample, AppearanceError> {
    base.validate()
        .map_err(|e| AppearanceError::Emission(e.to_string()))?;
    let modulation = if base.inside_isco || base.f_one_face_w_m2 == 0.0 {
        1.0
    } else {
        modulation_factor(spec, base.radius_over_m, base.azimuth, r_inner, r_outer)?
    };
    let f_app = if base.f_one_face_w_m2 == 0.0 {
        0.0
    } else {
        let v = base.f_one_face_w_m2 * modulation;
        if !v.is_finite() || v < 0.0 {
            return Err(AppearanceError::NonFinite(format!("F_app={v}")));
        }
        v
    };
    let t_app = if f_app == 0.0 {
        0.0
    } else {
        teff_from_one_face_flux(
            relativity_core::FluxWPerM2::new(f_app)
                .map_err(|e| AppearanceError::Emission(e.to_string()))?,
        )
        .map_err(|e| AppearanceError::Emission(e.to_string()))?
        .value()
    };
    if base.inside_isco && f_app > 0.0 {
        return Err(AppearanceError::InvalidSpec(
            "appearance must not create emission inside ISCO".into(),
        ));
    }
    Ok(AppearanceDiskEmissionSample {
        radius_over_m: base.radius_over_m,
        azimuth: base.azimuth,
        g_factor: base.g_factor,
        f_base_w_m2: base.f_one_face_w_m2,
        t_base_k: base.t_eff_k,
        modulation,
        f_app_w_m2: f_app,
        t_app_k: t_app,
        inside_isco: base.inside_isco,
    })
}

pub fn build_appearance_disk_emission_frame(
    emission: &PhysicalDiskEmissionFrame,
    spec: &DiskAppearanceSpec,
    source_physical_emission_digest: &str,
) -> Result<AppearanceDiskEmissionFrame, AppearanceError> {
    spec.validate()?;
    let spec_digest = disk_appearance_spec_digest(spec)?;
    let r_inner = emission.bounds.inner_radius();
    let r_outer = emission.bounds.outer_radius();
    let mut pixels = Vec::with_capacity(emission.pixels.len());
    for pix in &emission.pixels {
        match pix {
            PhysicalDiskEmissionPixel::DiskHit(base) => {
                pixels.push(AppearanceDiskEmissionPixel::DiskHit(derive_sample(
                    base, spec, r_inner, r_outer,
                )?));
            }
            PhysicalDiskEmissionPixel::NotDiskHit { outcome_class } => {
                pixels.push(AppearanceDiskEmissionPixel::NotDiskHit {
                    outcome_class: *outcome_class,
                });
            }
        }
    }
    Ok(AppearanceDiskEmissionFrame {
        grid: emission.grid,
        pixels,
        source_physical_emission_digest: source_physical_emission_digest.into(),
        disk_appearance_spec_digest: spec_digest,
        r_inner,
        r_outer,
    })
}

pub fn build_appearance_disk_color_frame(
    appearance_emission: &AppearanceDiskEmissionFrame,
    cie: &Cie1931Table,
    rgb_matrix: &XyzToRgbMatrix,
    measure: IntegrationMeasure,
) -> Result<AppearanceDiskColorFrame, AppearanceError> {
    let samples = cie
        .production_subset()
        .map_err(|e| AppearanceError::Colorimetry(e.to_string()))?;
    let mut pixels = Vec::with_capacity(appearance_emission.pixels.len());
    for (i, pix) in appearance_emission.pixels.iter().enumerate() {
        let col = (i as u32) % appearance_emission.grid.width;
        let row = (i as u32) / appearance_emission.grid.width;
        match pix {
            AppearanceDiskEmissionPixel::DiskHit(em) if em.f_app_w_m2 > 0.0 => {
                let xyz = integrate_xyz_from_emission(em.t_app_k, em.g_factor, &samples, measure)
                    .map_err(|e| AppearanceError::PixelMappingFailed {
                    col,
                    row,
                    cause: e.to_string(),
                })?;
                let rgb =
                    rgb_matrix
                        .apply(xyz)
                        .map_err(|e| AppearanceError::PixelMappingFailed {
                            col,
                            row,
                            cause: e.to_string(),
                        })?;
                pixels.push(AppearanceDiskColorPixel::DiskHit(
                    AppearanceDiskColorSample {
                        xyz,
                        rgb,
                        g_factor: em.g_factor,
                        f_app_w_m2: em.f_app_w_m2,
                        t_app_k: em.t_app_k,
                        modulation: em.modulation,
                        radius_over_m: em.radius_over_m,
                        azimuth: em.azimuth,
                    },
                ));
            }
            AppearanceDiskEmissionPixel::DiskHit(_) => {
                pixels.push(AppearanceDiskColorPixel::Absent {
                    outcome_class: relativity_trace::OutcomeClass::DiskHit,
                });
            }
            AppearanceDiskEmissionPixel::NotDiskHit { outcome_class } => {
                pixels.push(AppearanceDiskColorPixel::Absent {
                    outcome_class: *outcome_class,
                });
            }
        }
    }
    Ok(AppearanceDiskColorFrame {
        grid: appearance_emission.grid,
        pixels,
        source_physical_emission_digest: appearance_emission
            .source_physical_emission_digest
            .clone(),
        disk_appearance_spec_digest: appearance_emission.disk_appearance_spec_digest.clone(),
    })
}

pub fn appearance_disk_color_digest(frame: &AppearanceDiskColorFrame) -> String {
    let mut h = Sha256::new();
    h.update(b"appearance-disk-color-digest-v1");
    h.update(b"APPEARANCE_REPRODUCIBILITY_DIGEST");
    h.update(frame.source_physical_emission_digest.as_bytes());
    h.update(frame.disk_appearance_spec_digest.as_bytes());
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for pix in &frame.pixels {
        match pix {
            AppearanceDiskColorPixel::DiskHit(s) => {
                h.update([1u8]);
                h.update(s.rgb.r.to_bits().to_le_bytes());
                h.update(s.rgb.g.to_bits().to_le_bytes());
                h.update(s.rgb.b.to_bits().to_le_bytes());
                h.update(s.modulation.to_bits().to_le_bytes());
            }
            AppearanceDiskColorPixel::Absent { outcome_class } => {
                h.update([0u8]);
                h.update([outcome_class_code(*outcome_class)]);
            }
        }
    }
    hex_sha(&h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_spec(a_max: f64, identity: bool) -> DiskAppearanceSpec {
        DiskAppearanceSpec {
            model_id: DISK_APPEARANCE_MODEL_ID.into(),
            radial_envelope_id: RADIAL_ENVELOPE_ID.into(),
            mean_preservation_claim: MEAN_PRESERVATION_CLAIM.into(),
            a_max,
            r_ref_over_m: 6.0,
            modes: vec![
                SpiralHarmonicMode {
                    m: 2,
                    weight: 0.55,
                    k_log: 0.8,
                    phase: 0.0,
                },
                SpiralHarmonicMode {
                    m: 3,
                    weight: 0.30,
                    k_log: 1.1,
                    phase: 0.7,
                },
                SpiralHarmonicMode {
                    m: 5,
                    weight: 0.15,
                    k_log: 1.6,
                    phase: 1.9,
                },
            ],
            identity_modulation: identity,
        }
    }

    #[test]
    fn envelope_zero_at_boundaries() {
        let a_in = radial_amplitude_a(3.0, 3.0, 20.0, 0.3).unwrap();
        let a_out = radial_amplitude_a(20.0, 3.0, 20.0, 0.3).unwrap();
        let a_mid = radial_amplitude_a(11.5, 3.0, 20.0, 0.3).unwrap();
        assert!((a_in - 0.0).abs() < 1e-15);
        assert!((a_out - 0.0).abs() < 1e-15);
        assert!((a_mid - 0.3).abs() < 1e-12);
    }

    #[test]
    fn annular_mean_is_one() {
        let spec = sample_spec(0.45, false);
        let n = 2048usize;
        for r in [4.0, 8.0, 12.0, 18.0] {
            let mut sum = 0.0;
            for i in 0..n {
                let phi = std::f64::consts::TAU * (i as f64) / (n as f64);
                sum += modulation_factor(&spec, r, phi, 3.0, 20.0).unwrap();
            }
            let mean = sum / (n as f64);
            assert!((mean - 1.0).abs() < 1e-12, "r={r} mean={mean}");
        }
    }

    #[test]
    fn positivity_and_identity() {
        let spec = sample_spec(0.6, false);
        for r in [4.0_f64, 10.0, 18.0] {
            for i in 0..512 {
                let phi = std::f64::consts::TAU * (i as f64) / 512.0;
                let m = modulation_factor(&spec, r, phi, 3.0, 20.0).unwrap();
                assert!(m > 0.0 && m.is_finite());
            }
        }
        let id = sample_spec(0.6, true);
        assert_eq!(modulation_factor(&id, 10.0, 1.2, 3.0, 20.0).unwrap(), 1.0);
    }

    #[test]
    fn seam_continuity_integer_m() {
        let spec = sample_spec(0.3, false);
        let a = modulation_factor(&spec, 10.0, std::f64::consts::PI, 3.0, 20.0).unwrap();
        let b = modulation_factor(&spec, 10.0, -std::f64::consts::PI, 3.0, 20.0).unwrap();
        assert!((a - b).abs() < 1e-12);
    }
}
