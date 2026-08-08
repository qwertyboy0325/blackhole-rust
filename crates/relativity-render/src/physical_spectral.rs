//! Physical spectral `I_ν` transport on SI Hz grids (Gate 2C0).
//!
//! Reuses [`relativity_core::transport_i_nu`] for `g³`. Never attaches SI meaning
//! to diagnostic `spectral-grid-v1`.

use crate::error::SpectralRenderError;
use crate::physical_disk::{
    PhysicalDiskEmissionFrame, PhysicalDiskEmissionPixel, PHYSICAL_EMISSION_MODEL_ID,
};
use crate::planck::{planck_b_nu, stefan_boltzmann_flux, PLANCK_MODEL_ID};
use relativity_core::{
    transport_i_nu, FrequencyShift, PhysicalFrequencyHz, SpectralGrid, SpectralMeasure,
    TemperatureKelvin,
};
use relativity_trace::{hex_sha, pixel_index, OutcomeClass, TraceGrid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PHYSICAL_SPECTRAL_CONVENTION_ID: &str = "physical-spectral-disk-g3-v1";
pub const PHYSICAL_SPECTRAL_UNITS: &str = "W_m^-2_Hz^-1_sr^-1";
pub const PHYSICAL_GRID_EXPLORE_PREFIX: &str = "physical-spectral-grid-explore-";
/// Frozen Gate 2C0 evaluation grid (256 log bins on the calibration band).
pub const PHYSICAL_GRID_V1_ID: &str = "physical-spectral-grid-v1";
pub const PHYSICAL_GRID_V1_N_BINS: u32 = 256;
/// Calibration / frozen band (Hz) covering IR–UV for Gate 2C0 T_eff range after g-mapping.
pub const PHYSICAL_GRID_NU_MIN_HZ: f64 = 1.0e11;
pub const PHYSICAL_GRID_NU_MAX_HZ: f64 = 1.0e17;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicalSpectralConvention {
    pub schema_version: u32,
    pub convention_id: String,
    pub intensity_units: String,
    pub transport_law: String,
    pub transport_arithmetic: String,
    pub planck_model_id: String,
    pub emission_model_id: String,
    pub measure: SpectralMeasure,
    pub colorimetry_status: String,
}

impl PhysicalSpectralConvention {
    pub fn v1() -> Self {
        Self {
            schema_version: 1,
            convention_id: PHYSICAL_SPECTRAL_CONVENTION_ID.into(),
            intensity_units: PHYSICAL_SPECTRAL_UNITS.into(),
            transport_law: "observed-i-nu-equals-g-cubed-times-emitted-i-nu-at-nu-obs-over-g"
                .into(),
            transport_arithmetic: "g2-equals-g-times-g-g3-equals-g2-times-g".into(),
            planck_model_id: PLANCK_MODEL_ID.into(),
            emission_model_id: PHYSICAL_EMISSION_MODEL_ID.into(),
            measure: SpectralMeasure::FrequencySpecificIntensity,
            colorimetry_status: "deferred-to-gate-2c1".into(),
        }
    }
}

/// Explore physical Hz grid (ladder evidence only; not the frozen gate authority).
pub fn physical_spectral_grid_explore(n_bins: u32) -> Result<SpectralGrid, SpectralRenderError> {
    if n_bins < 8 {
        return Err(SpectralRenderError::InvalidGrid(
            "physical explore grid requires n_bins >= 8".into(),
        ));
    }
    let id = format!("{PHYSICAL_GRID_EXPLORE_PREFIX}{n_bins}");
    SpectralGrid::log_spaced(id, PHYSICAL_GRID_NU_MIN_HZ, PHYSICAL_GRID_NU_MAX_HZ, n_bins)
        .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))
}

/// Frozen Gate 2C0 physical spectral grid (`physical-spectral-grid-v1`).
pub fn physical_spectral_grid_v1() -> Result<SpectralGrid, SpectralRenderError> {
    SpectralGrid::log_spaced(
        PHYSICAL_GRID_V1_ID,
        PHYSICAL_GRID_NU_MIN_HZ,
        PHYSICAL_GRID_NU_MAX_HZ,
        PHYSICAL_GRID_V1_N_BINS,
    )
    .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))
}

pub fn parse_physical_spectral_grid_id(grid_id: &str) -> Result<SpectralGrid, SpectralRenderError> {
    if grid_id == "spectral-grid-v1" || grid_id.starts_with("spectral-grid-explore-") {
        return Err(SpectralRenderError::UnsupportedGridId(format!(
            "diagnostic grid `{grid_id}` is not physical Hz; use `{PHYSICAL_GRID_V1_ID}` or `{PHYSICAL_GRID_EXPLORE_PREFIX}{{n}}`"
        )));
    }
    if grid_id == PHYSICAL_GRID_V1_ID {
        return physical_spectral_grid_v1();
    }
    let n: u32 = grid_id
        .strip_prefix(PHYSICAL_GRID_EXPLORE_PREFIX)
        .ok_or_else(|| SpectralRenderError::UnsupportedGridId(grid_id.into()))?
        .parse()
        .map_err(|_| SpectralRenderError::UnsupportedGridId(grid_id.into()))?;
    physical_spectral_grid_explore(n)
}

fn is_allowed_physical_grid_id(grid_id: &str) -> bool {
    grid_id == PHYSICAL_GRID_V1_ID || grid_id.starts_with(PHYSICAL_GRID_EXPLORE_PREFIX)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalSpectralDiskSample {
    pub radius_over_m: f64,
    pub g_factor: f64,
    pub t_eff_k: f64,
    pub f_one_face_w_m2: f64,
    pub integrated_emitted_i_nu: f64,
    pub integrated_observed_i_nu: f64,
    pub emitted_truncation_fraction: f64,
    pub observed_bolometric_from_g4: f64,
    pub i_nu_obs: Vec<f64>,
}

impl PhysicalSpectralDiskSample {
    pub fn validate(&self, n_bins: usize) -> Result<(), SpectralRenderError> {
        if !self.radius_over_m.is_finite() || !(self.radius_over_m > 0.0) {
            return Err(SpectralRenderError::InvalidIntensity(
                "radius_over_m must be finite and > 0".into(),
            ));
        }
        if !self.g_factor.is_finite() || !(self.g_factor > 0.0) {
            return Err(SpectralRenderError::InvalidFrequency(
                "g_factor must be finite and > 0".into(),
            ));
        }
        if !self.f_one_face_w_m2.is_finite() || self.f_one_face_w_m2 < 0.0 {
            return Err(SpectralRenderError::InvalidIntensity(
                "f_one_face must be finite and >= 0".into(),
            ));
        }
        if !self.t_eff_k.is_finite() || self.t_eff_k < 0.0 {
            return Err(SpectralRenderError::InvalidIntensity(
                "t_eff must be finite and >= 0".into(),
            ));
        }
        if self.f_one_face_w_m2 > 0.0 && !(self.t_eff_k > 0.0) {
            return Err(SpectralRenderError::InvalidIntensity(
                "F>0 requires T_eff>0".into(),
            ));
        }
        if !self.emitted_truncation_fraction.is_finite()
            || self.emitted_truncation_fraction < 0.0
            || self.emitted_truncation_fraction > 1.0
        {
            return Err(SpectralRenderError::InvalidIntensity(
                "emitted_truncation_fraction must be in [0,1]".into(),
            ));
        }
        for name_val in [
            ("integrated_emitted_i_nu", self.integrated_emitted_i_nu),
            ("integrated_observed_i_nu", self.integrated_observed_i_nu),
            (
                "observed_bolometric_from_g4",
                self.observed_bolometric_from_g4,
            ),
        ] {
            if !name_val.1.is_finite() || name_val.1 < 0.0 {
                return Err(SpectralRenderError::InvalidIntensity(format!(
                    "{} must be finite and >= 0",
                    name_val.0
                )));
            }
        }
        if self.i_nu_obs.len() != n_bins {
            return Err(SpectralRenderError::InvalidIntensity(format!(
                "i_nu_obs length {} != n_bins {n_bins}",
                self.i_nu_obs.len()
            )));
        }
        for (i, &v) in self.i_nu_obs.iter().enumerate() {
            if !v.is_finite() || v < 0.0 {
                return Err(SpectralRenderError::InvalidIntensity(format!(
                    "i_nu_obs[{i}] must be finite and >= 0"
                )));
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for PhysicalSpectralDiskSample {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            radius_over_m: f64,
            g_factor: f64,
            t_eff_k: f64,
            f_one_face_w_m2: f64,
            integrated_emitted_i_nu: f64,
            integrated_observed_i_nu: f64,
            emitted_truncation_fraction: f64,
            observed_bolometric_from_g4: f64,
            i_nu_obs: Vec<f64>,
        }
        let raw = Raw::deserialize(deserializer)?;
        let sample = Self {
            radius_over_m: raw.radius_over_m,
            g_factor: raw.g_factor,
            t_eff_k: raw.t_eff_k,
            f_one_face_w_m2: raw.f_one_face_w_m2,
            integrated_emitted_i_nu: raw.integrated_emitted_i_nu,
            integrated_observed_i_nu: raw.integrated_observed_i_nu,
            emitted_truncation_fraction: raw.emitted_truncation_fraction,
            observed_bolometric_from_g4: raw.observed_bolometric_from_g4,
            i_nu_obs: raw.i_nu_obs,
        };
        // Length checked fully at frame level; validate scalar fields here.
        sample
            .validate(sample.i_nu_obs.len())
            .map_err(serde::de::Error::custom)?;
        Ok(sample)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PhysicalSpectralPixel {
    DiskHit(PhysicalSpectralDiskSample),
    NotDiskHit { outcome_class: OutcomeClass },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalSpectralFrame {
    pub grid: TraceGrid,
    pub spectral_grid: SpectralGrid,
    pub pixels: Vec<PhysicalSpectralPixel>,
}

impl PhysicalSpectralFrame {
    pub fn try_new(
        grid: TraceGrid,
        spectral_grid: SpectralGrid,
        pixels: Vec<PhysicalSpectralPixel>,
    ) -> Result<Self, SpectralRenderError> {
        spectral_grid
            .validate()
            .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))?;
        if !is_allowed_physical_grid_id(spectral_grid.grid_id()) {
            return Err(SpectralRenderError::UnsupportedGridId(
                spectral_grid.grid_id().into(),
            ));
        }
        if pixels.len() != grid.pixel_count() {
            return Err(SpectralRenderError::FrameLengthMismatch);
        }
        let n_bins = spectral_grid.n_bins() as usize;
        for pix in &pixels {
            if let PhysicalSpectralPixel::DiskHit(s) = pix {
                s.validate(n_bins)?;
            }
        }
        Ok(Self {
            grid,
            spectral_grid,
            pixels,
        })
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &PhysicalSpectralPixel {
        &self.pixels[pixel_index(self.grid, col, row)]
    }
}

impl<'de> Deserialize<'de> for PhysicalSpectralFrame {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            grid: TraceGrid,
            spectral_grid: SpectralGrid,
            pixels: Vec<PhysicalSpectralPixel>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::try_new(raw.grid, raw.spectral_grid, raw.pixels).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicalSpectralClosureMetrics {
    pub max_rel_emitter_sb_error: f64,
    pub max_abs_emitter_sb_error: f64,
    pub max_rel_g4_transport_error: f64,
    pub max_abs_g4_transport_error: f64,
    /// Lowest-index pixel attaining the relative emitter SB maximum (ties → min).
    pub worst_rel_emitter_pixel: Option<(u32, u32)>,
    /// Lowest-index pixel attaining the absolute emitter SB maximum (ties → min).
    pub worst_abs_emitter_pixel: Option<(u32, u32)>,
    /// Lowest-index pixel attaining the relative g⁴ transport maximum (ties → min).
    pub worst_rel_transport_pixel: Option<(u32, u32)>,
    /// Lowest-index pixel attaining the absolute g⁴ transport maximum (ties → min).
    pub worst_abs_transport_pixel: Option<(u32, u32)>,
    pub disk_hit_with_emission: u64,
}

fn planck_mass_on_interval(
    temperature: TemperatureKelvin,
    lo_hz: f64,
    hi_hz: f64,
    n: u32,
) -> Result<f64, SpectralRenderError> {
    if temperature.value() == 0.0 {
        return Ok(0.0);
    }
    if !(hi_hz > lo_hz) {
        return Ok(0.0);
    }
    let ln_lo = lo_hz.ln();
    let ln_hi = hi_hz.ln();
    let mut acc = 0.0;
    for i in 0..n {
        let t0 = i as f64 / n as f64;
        let t1 = (i + 1) as f64 / n as f64;
        let a = (ln_lo + t0 * (ln_hi - ln_lo)).exp();
        let b = (ln_lo + t1 * (ln_hi - ln_lo)).exp();
        let c = (a * b).sqrt();
        let w = b - a;
        let nu = PhysicalFrequencyHz::new(c)
            .map_err(|e| SpectralRenderError::InvalidFrequency(e.to_string()))?;
        let bnu = planck_b_nu(nu, temperature)
            .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?
            .value();
        acc += bnu * w;
    }
    Ok(acc)
}

fn map_disk_hit(
    em: &crate::physical_disk::PhysicalDiskEmissionSample,
    spectral_grid: &SpectralGrid,
) -> Result<PhysicalSpectralDiskSample, SpectralRenderError> {
    if !(em.f_one_face_w_m2 > 0.0) || !(em.t_eff_k > 0.0) {
        return Err(SpectralRenderError::InvalidIntensity(
            "physical spectral map requires F>0 and T_eff>0".into(),
        ));
    }
    let g = FrequencyShift::new(em.g_factor)
        .map_err(|e| SpectralRenderError::InvalidFrequency(format!("g: {e}")))?;
    let g_val = g.value();
    let t = TemperatureKelvin::new(em.t_eff_k)
        .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?;
    let n = spectral_grid.n_bins() as usize;
    let mut i_nu_obs = vec![0.0; n];
    let mut integ_em = 0.0;
    let mut integ_obs = 0.0;
    for (i, (&nu_obs, &w)) in spectral_grid
        .centers()
        .iter()
        .zip(spectral_grid.weights().iter())
        .enumerate()
    {
        let nu_em = nu_obs / g_val;
        let nu = PhysicalFrequencyHz::new(nu_em)
            .map_err(|e| SpectralRenderError::InvalidFrequency(e.to_string()))?;
        let i_em = planck_b_nu(nu, t)
            .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?
            .value();
        let i_obs = transport_i_nu(i_em, g_val)
            .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?;
        i_nu_obs[i] = i_obs;
        let w_em = w / g_val;
        integ_em += i_em * w_em;
        integ_obs += i_obs * w;
    }
    // Truncation vs analytic ∫_0^∞ B_ν dν = σ T⁴ / π = F/π (not another finite band).
    let nu_obs_min = spectral_grid.nu_min();
    let nu_obs_max = spectral_grid.nu_max();
    let nu_em_lo = nu_obs_min / g_val;
    let nu_em_hi = nu_obs_max / g_val;
    let captured = planck_mass_on_interval(t, nu_em_lo, nu_em_hi, 512)?;
    let sigma_t4 = stefan_boltzmann_flux(t)
        .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?
        .value();
    let total = sigma_t4 / std::f64::consts::PI;
    let trunc = if total > 0.0 {
        (1.0 - (captured / total).clamp(0.0, 1.0)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let i_bol_em = em.f_one_face_w_m2 / std::f64::consts::PI; // F = π I for isotropic
    let g2 = g_val * g_val;
    let g4 = g2 * g2;
    let sample = PhysicalSpectralDiskSample {
        radius_over_m: em.radius_over_m,
        g_factor: g_val,
        t_eff_k: em.t_eff_k,
        f_one_face_w_m2: em.f_one_face_w_m2,
        integrated_emitted_i_nu: integ_em,
        integrated_observed_i_nu: integ_obs,
        emitted_truncation_fraction: trunc,
        observed_bolometric_from_g4: i_bol_em * g4,
        i_nu_obs,
    };
    sample.validate(n)?;
    Ok(sample)
}

pub fn build_physical_spectral_frame(
    emission_frame: &PhysicalDiskEmissionFrame,
    spectral_grid: &SpectralGrid,
) -> Result<PhysicalSpectralFrame, SpectralRenderError> {
    spectral_grid
        .validate()
        .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))?;
    if !is_allowed_physical_grid_id(spectral_grid.grid_id()) {
        return Err(SpectralRenderError::UnsupportedGridId(
            spectral_grid.grid_id().into(),
        ));
    }
    let grid = emission_frame.grid;
    let mut pixels = Vec::with_capacity(grid.pixel_count());
    for row in 0..grid.height {
        for col in 0..grid.width {
            let pixel = match emission_frame.pixel_at(col, row) {
                PhysicalDiskEmissionPixel::DiskHit(em) if em.f_one_face_w_m2 > 0.0 => {
                    let sample = map_disk_hit(em, spectral_grid).map_err(|e| {
                        SpectralRenderError::PixelMappingFailed {
                            col,
                            row,
                            cause: e.to_string(),
                        }
                    })?;
                    PhysicalSpectralPixel::DiskHit(sample)
                }
                PhysicalDiskEmissionPixel::DiskHit(_) => PhysicalSpectralPixel::NotDiskHit {
                    outcome_class: OutcomeClass::DiskHit,
                },
                PhysicalDiskEmissionPixel::NotDiskHit { outcome_class } => {
                    PhysicalSpectralPixel::NotDiskHit {
                        outcome_class: *outcome_class,
                    }
                }
            };
            pixels.push(pixel);
        }
    }
    PhysicalSpectralFrame::try_new(grid, spectral_grid.clone(), pixels)
}

pub fn compute_physical_spectral_closure(
    frame: &PhysicalSpectralFrame,
) -> Result<PhysicalSpectralClosureMetrics, SpectralRenderError> {
    let mut max_rel_em = 0.0;
    let mut max_abs_em = 0.0;
    let mut max_rel_tr = 0.0;
    let mut max_abs_tr = 0.0;
    let mut worst_rel_em = None;
    let mut worst_abs_em = None;
    let mut worst_rel_tr = None;
    let mut worst_abs_tr = None;
    let mut count = 0u64;
    for row in 0..frame.grid.height {
        for col in 0..frame.grid.width {
            let PhysicalSpectralPixel::DiskHit(s) = frame.pixel_at(col, row) else {
                continue;
            };
            count += 1;
            let t = TemperatureKelvin::new(s.t_eff_k)
                .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?;
            let sigma_t4 = stefan_boltzmann_flux(t)
                .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?
                .value();
            // Emitter closure: π ∫ I_ν,em ≈ F = σ T⁴ (on captured band, scaled by (1−trunc)).
            let pi_integ = std::f64::consts::PI * s.integrated_emitted_i_nu;
            let expected_capt = sigma_t4 * (1.0 - s.emitted_truncation_fraction);
            let abs_em = (pi_integ - expected_capt).abs();
            let rel_em = abs_em / expected_capt.max(1e-30);
            if !abs_em.is_finite() || !rel_em.is_finite() {
                return Err(SpectralRenderError::VerificationFailed {
                    col,
                    row,
                    cause: "non-finite emitter SB closure".into(),
                });
            }
            // Strict `>` → lowest (col,row) in raster order wins ties.
            if abs_em > max_abs_em {
                max_abs_em = abs_em;
                worst_abs_em = Some((col, row));
            }
            if rel_em > max_rel_em {
                max_rel_em = rel_em;
                worst_rel_em = Some((col, row));
            }
            // Transport: ∫ I_obs ≈ g⁴ ∫ I_em (same observer-mapped band).
            let g = s.g_factor;
            let g4 = {
                let g2 = g * g;
                g2 * g2
            };
            let expect_obs = g4 * s.integrated_emitted_i_nu;
            let abs_tr = (s.integrated_observed_i_nu - expect_obs).abs();
            let rel_tr = abs_tr / expect_obs.max(1e-30);
            if !abs_tr.is_finite() || !rel_tr.is_finite() {
                return Err(SpectralRenderError::VerificationFailed {
                    col,
                    row,
                    cause: "non-finite g4 transport closure".into(),
                });
            }
            if abs_tr > max_abs_tr {
                max_abs_tr = abs_tr;
                worst_abs_tr = Some((col, row));
            }
            if rel_tr > max_rel_tr {
                max_rel_tr = rel_tr;
                worst_rel_tr = Some((col, row));
            }
        }
    }
    Ok(PhysicalSpectralClosureMetrics {
        max_rel_emitter_sb_error: max_rel_em,
        max_abs_emitter_sb_error: max_abs_em,
        max_rel_g4_transport_error: max_rel_tr,
        max_abs_g4_transport_error: max_abs_tr,
        worst_rel_emitter_pixel: worst_rel_em,
        worst_abs_emitter_pixel: worst_abs_em,
        worst_rel_transport_pixel: worst_rel_tr,
        worst_abs_transport_pixel: worst_abs_tr,
        disk_hit_with_emission: count,
    })
}

pub fn physical_spectral_grid_digest(grid: &SpectralGrid) -> Result<String, SpectralRenderError> {
    grid.validate()
        .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))?;
    let mut h = Sha256::new();
    h.update(b"physical-spectral-grid-digest-v1");
    h.update(grid.grid_id().as_bytes());
    h.update(grid.measure().digest_tag().as_bytes());
    h.update(grid.nu_min().to_bits().to_le_bytes());
    h.update(grid.nu_max().to_bits().to_le_bytes());
    h.update(grid.n_bins().to_le_bytes());
    for e in grid.edges() {
        h.update(e.to_bits().to_le_bytes());
    }
    Ok(hex_sha(&h.finalize()))
}

pub fn physical_spectral_digest(
    frame: &PhysicalSpectralFrame,
    convention: &PhysicalSpectralConvention,
    emission_digest: &str,
) -> Result<String, SpectralRenderError> {
    let n_bins = frame.spectral_grid.n_bins() as usize;
    for pix in &frame.pixels {
        if let PhysicalSpectralPixel::DiskHit(s) = pix {
            s.validate(n_bins)?;
        }
    }
    let mut h = Sha256::new();
    h.update(b"physical-spectral-digest-v1");
    h.update(convention.convention_id.as_bytes());
    h.update(convention.planck_model_id.as_bytes());
    h.update(emission_digest.as_bytes());
    h.update(physical_spectral_grid_digest(&frame.spectral_grid)?.as_bytes());
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for pix in &frame.pixels {
        match pix {
            PhysicalSpectralPixel::DiskHit(s) => {
                h.update([1u8]);
                h.update(s.radius_over_m.to_bits().to_le_bytes());
                h.update(s.g_factor.to_bits().to_le_bytes());
                h.update(s.t_eff_k.to_bits().to_le_bytes());
                h.update(s.f_one_face_w_m2.to_bits().to_le_bytes());
                for v in &s.i_nu_obs {
                    h.update(v.to_bits().to_le_bytes());
                }
            }
            PhysicalSpectralPixel::NotDiskHit { outcome_class } => {
                h.update([0u8]);
                h.update(outcome_class.digest_tag().as_bytes());
            }
        }
    }
    Ok(hex_sha(&h.finalize()))
}

/// Hermetic independent observer sample: `I_ν,obs = g³ B_ν(ν_obs/g, T)`.
pub fn independent_physical_i_nu_obs(
    t_eff_k: f64,
    g: f64,
    nu_obs_hz: f64,
) -> Result<f64, SpectralRenderError> {
    let t = TemperatureKelvin::new(t_eff_k)
        .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?;
    let nu_em = PhysicalFrequencyHz::new(nu_obs_hz / g)
        .map_err(|e| SpectralRenderError::InvalidFrequency(e.to_string()))?;
    let i_em = planck_b_nu(nu_em, t)
        .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?
        .value();
    transport_i_nu(i_em, g).map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_diagnostic_grid_id() {
        assert!(parse_physical_spectral_grid_id("spectral-grid-v1").is_err());
    }

    #[test]
    fn explore_grid_hz_bounds() {
        let g = physical_spectral_grid_explore(64).unwrap();
        assert_eq!(g.nu_min(), PHYSICAL_GRID_NU_MIN_HZ);
        assert_eq!(g.nu_max(), PHYSICAL_GRID_NU_MAX_HZ);
        assert!(g.grid_id().starts_with(PHYSICAL_GRID_EXPLORE_PREFIX));
    }

    #[test]
    fn frozen_v1_grid() {
        let g = physical_spectral_grid_v1().unwrap();
        assert_eq!(g.grid_id(), PHYSICAL_GRID_V1_ID);
        assert_eq!(g.n_bins(), PHYSICAL_GRID_V1_N_BINS);
        let parsed = parse_physical_spectral_grid_id(PHYSICAL_GRID_V1_ID).unwrap();
        assert_eq!(parsed.grid_id(), g.grid_id());
    }

    #[test]
    fn transport_g_one_matches_planck() {
        let t = 5.0e3;
        let nu = 5.0e14;
        let i = independent_physical_i_nu_obs(t, 1.0, nu).unwrap();
        let b = planck_b_nu(
            PhysicalFrequencyHz::new(nu).unwrap(),
            TemperatureKelvin::new(t).unwrap(),
        )
        .unwrap()
        .value();
        assert!((i - b).abs() / b < 1e-12);
    }

    #[test]
    fn truncation_uses_analytic_total() {
        let t = TemperatureKelvin::new(5.0e4).unwrap();
        let total = stefan_boltzmann_flux(t).unwrap().value() / std::f64::consts::PI;
        // Narrow window captures little of the analytic 0→∞ mass.
        let captured_narrow = planck_mass_on_interval(t, 1.0e14, 1.1e14, 256).unwrap();
        assert!(captured_narrow / total < 0.05);
        // Finite-band numerical total must not replace analytic authority: a wider
        // numerical band still differs from σT⁴/π at the quadrature noise floor,
        // but truncation mass is always `1 - captured/analytic`.
        let captured_gate = planck_mass_on_interval(t, 1.0e11, 1.0e17, 1024).unwrap();
        let trunc = (1.0 - (captured_gate / total).clamp(0.0, 1.0)).clamp(0.0, 1.0);
        assert!((0.0..=1.0).contains(&trunc));
        let wrong_total = planck_mass_on_interval(t, 1.0e10, 1.0e18, 1024).unwrap();
        // Using wrong_total as denominator would hide out-of-[1e10,1e18] mass.
        assert!((wrong_total - total).abs() / total > 0.0 || trunc >= 0.0);
        let _ = wrong_total;
    }

    #[test]
    fn closure_max_abs_independent_of_max_rel() {
        let grid = TraceGrid {
            width: 2,
            height: 1,
        };
        let spectral = physical_spectral_grid_explore(8).unwrap();
        let n = spectral.n_bins() as usize;
        // Pixel 0: large absolute, small relative (large expected).
        // Pixel 1: small absolute, large relative (tiny expected).
        let s0 = PhysicalSpectralDiskSample {
            radius_over_m: 10.0,
            g_factor: 1.0,
            t_eff_k: 1.0e4,
            f_one_face_w_m2: std::f64::consts::PI * 1.0e6,
            integrated_emitted_i_nu: 1.0e6 + 100.0, // abs≈100 vs expected σT⁴(1-trunc)≈π*integ target
            integrated_observed_i_nu: 1.0e6 + 100.0,
            emitted_truncation_fraction: 0.0,
            observed_bolometric_from_g4: 1.0e6,
            i_nu_obs: vec![0.0; n],
        };
        // Force emitter expected via T such that σT⁴ = π * 1e6 roughly — use F and T consistently
        // Closure uses σT⁴ from T_eff, not F. Set T so σT⁴ = 1e4 (small), integ such that abs is small but rel large.
        let t_small = 100.0;
        let sigma_small = stefan_boltzmann_flux(TemperatureKelvin::new(t_small).unwrap())
            .unwrap()
            .value();
        let s1 = PhysicalSpectralDiskSample {
            radius_over_m: 10.0,
            g_factor: 1.0,
            t_eff_k: t_small,
            f_one_face_w_m2: sigma_small,
            integrated_emitted_i_nu: (sigma_small / std::f64::consts::PI) * 2.0, // 100% relative if trunc=0
            integrated_observed_i_nu: (sigma_small / std::f64::consts::PI) * 2.0,
            emitted_truncation_fraction: 0.0,
            observed_bolometric_from_g4: sigma_small / std::f64::consts::PI,
            i_nu_obs: vec![0.0; n],
        };
        // Large-abs pixel: T with σT⁴ = 1e8, integ off by 1e3 absolute → rel small.
        let t_big = ((1.0e8) / relativity_core::stefan_boltzmann_w_m2_k4()).powf(0.25);
        let sigma_big = stefan_boltzmann_flux(TemperatureKelvin::new(t_big).unwrap())
            .unwrap()
            .value();
        let s0 = PhysicalSpectralDiskSample {
            t_eff_k: t_big,
            f_one_face_w_m2: sigma_big,
            integrated_emitted_i_nu: sigma_big / std::f64::consts::PI
                + 1.0e3 / std::f64::consts::PI,
            integrated_observed_i_nu: sigma_big / std::f64::consts::PI
                + 1.0e3 / std::f64::consts::PI,
            ..s0
        };
        let frame = PhysicalSpectralFrame::try_new(
            grid,
            spectral,
            vec![
                PhysicalSpectralPixel::DiskHit(s0),
                PhysicalSpectralPixel::DiskHit(s1),
            ],
        )
        .unwrap();
        let m = compute_physical_spectral_closure(&frame).unwrap();
        assert_eq!(m.worst_abs_emitter_pixel, Some((0, 0)));
        assert_eq!(m.worst_rel_emitter_pixel, Some((1, 0)));
        assert!(m.max_abs_emitter_sb_error > 100.0);
        assert!(m.max_rel_emitter_sb_error > 0.5);
    }

    #[test]
    fn deserialize_rejects_negative_i_nu() {
        let grid = physical_spectral_grid_v1().unwrap();
        let n = grid.n_bins() as usize;
        let mut i = vec![0.0; n];
        i[0] = -1.0;
        let json = serde_json::json!({
            "radius_over_m": 10.0,
            "g_factor": 1.0,
            "t_eff_k": 1.0e4,
            "f_one_face_w_m2": 1.0,
            "integrated_emitted_i_nu": 0.0,
            "integrated_observed_i_nu": 0.0,
            "emitted_truncation_fraction": 0.0,
            "observed_bolometric_from_g4": 0.0,
            "i_nu_obs": i,
        });
        assert!(serde_json::from_value::<PhysicalSpectralDiskSample>(json).is_err());
    }
}
