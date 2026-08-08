//! Diagnostic spectral specific-intensity transport (Gate 2B2).
//!
//! Canonical measure `I_ν` with vacuum law `I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs/g)`,
//! scaled from Gate 2B1 bolometric intensity. Not temperature, Planck, RGB, or EXR.

use crate::bolometric::{
    DiskBolometricFrame, DiskBolometricPixel, ResolvedDiskBounds, CANONICAL_DISK_EMISSION_CLAIM,
    CANONICAL_DISK_EMISSION_MODEL,
};
use crate::error::SpectralRenderError;
use crate::frequency_shift::{DiskFrequencyShiftFrame, DiskFrequencyShiftPixel, DiskVelocityModel};
use relativity_core::{
    transport_i_nu, EquatorialAngularDirection, FrequencyShift, SpectralGrid, SpectralMeasure,
};
use relativity_trace::{hex_sha, pixel_index, OutcomeClass, TraceGrid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SPECTRAL_CONVENTION_ID: &str = "diagnostic-spectral-disk-g3-v1";
pub const CONTINUUM_SPECTRUM_ID: &str = "diagnostic-lognormal-continuum-v1";
pub const LINE_FIXTURE_ID: &str = "diagnostic-gaussian-line-v1";
pub const SPECTRAL_UNITS_V1: &str =
    "arbitrary-normalized-spectral-specific-intensity-per-unit-frequency";
pub const EMITTER_DOMAIN_POLICY: &str = "zero-outside-domain-with-truncated-energy-accounting";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticSpectrumSpec {
    pub schema_version: u32,
    pub spectrum_id: String,
    pub measure: SpectralMeasure,
    pub mu: f64,
    pub sigma: f64,
    pub nu_min: f64,
    pub nu_max: f64,
    pub units: String,
}

impl DiagnosticSpectrumSpec {
    pub fn validate(&self) -> Result<(), SpectralRenderError> {
        let canon = diagnostic_lognormal_continuum_v1();
        if self != &canon {
            return Err(SpectralRenderError::InvalidSpectrumSpec(
                "non-canonical diagnostic-lognormal-continuum-v1 field mutation".into(),
            ));
        }
        Ok(())
    }
}

pub fn diagnostic_lognormal_continuum_v1() -> DiagnosticSpectrumSpec {
    DiagnosticSpectrumSpec {
        schema_version: 1,
        spectrum_id: CONTINUUM_SPECTRUM_ID.into(),
        measure: SpectralMeasure::FrequencySpecificIntensity,
        mu: 0.0,
        sigma: 0.5,
        nu_min: SpectralGrid::V1_NU_MIN,
        nu_max: SpectralGrid::V1_NU_MAX,
        units: SPECTRAL_UNITS_V1.into(),
    }
}

/// Unnormalized kernel `(1/ν) exp(-(ln ν − μ)²/(2σ²))`.
fn lognormal_kernel(nu: f64, mu: f64, sigma: f64) -> f64 {
    let ln = nu.ln();
    let z = (ln - mu) / sigma;
    (1.0 / nu) * (-0.5 * z * z).exp()
}

fn std_normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf_approx(z / std::f64::consts::SQRT_2))
}

/// Abramowitz–Stegun 7.1.26 style erf approximation (sufficient for digest/tests).
fn erf_approx(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let ax = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * ax);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    sign * (1.0 - poly * (-ax * ax).exp())
}

/// Analytic truncation normalization for the lognormal-in-frequency continuum.
pub fn continuum_normalization(spec: &DiagnosticSpectrumSpec) -> Result<f64, SpectralRenderError> {
    if !(spec.sigma > 0.0) || !spec.sigma.is_finite() {
        return Err(SpectralRenderError::InvalidSpectrumSpec(
            "sigma must be finite and > 0".into(),
        ));
    }
    let a = (spec.nu_min.ln() - spec.mu) / spec.sigma;
    let b = (spec.nu_max.ln() - spec.mu) / spec.sigma;
    let mass = std_normal_cdf(b) - std_normal_cdf(a);
    // ∫ (1/ν) exp(...) dν = σ √(2π) · ΔΦ
    let integ_unnorm = spec.sigma * (2.0 * std::f64::consts::PI).sqrt() * mass;
    if !integ_unnorm.is_finite() || !(integ_unnorm > 0.0) {
        return Err(SpectralRenderError::InvalidSpectrumSpec(
            "continuum normalization integral non-positive".into(),
        ));
    }
    Ok(1.0 / integ_unnorm)
}

/// ∫ φ(ν) dν over `[lo, hi] ∩ [ν_min, ν_max]` (analytic; 0 if empty).
pub fn continuum_mass_on_interval(
    spec: &DiagnosticSpectrumSpec,
    lo: f64,
    hi: f64,
) -> Result<f64, SpectralRenderError> {
    if !lo.is_finite() || !hi.is_finite() {
        return Err(SpectralRenderError::InvalidFrequency(
            "continuum mass interval must be finite".into(),
        ));
    }
    let a = lo.min(hi).max(spec.nu_min);
    let b = lo.max(hi).min(spec.nu_max);
    if !(b > a) {
        return Ok(0.0);
    }
    let n = continuum_normalization(spec)?;
    let za = (a.ln() - spec.mu) / spec.sigma;
    let zb = (b.ln() - spec.mu) / spec.sigma;
    let mass = std_normal_cdf(zb) - std_normal_cdf(za);
    let unnorm = spec.sigma * (2.0 * std::f64::consts::PI).sqrt() * mass;
    let out = n * unnorm;
    if !out.is_finite() || out < 0.0 {
        return Err(SpectralRenderError::InvalidIntensity(
            "continuum mass non-finite".into(),
        ));
    }
    // Clamp tiny numerical overshoot.
    Ok(out.clamp(0.0, 1.0))
}

/// Dimensionless `φ(ν)` on the declared domain (0 outside).
pub fn evaluate_continuum_phi(
    spec: &DiagnosticSpectrumSpec,
    nu: f64,
) -> Result<f64, SpectralRenderError> {
    if !nu.is_finite() || !(nu > 0.0) {
        return Err(SpectralRenderError::InvalidFrequency(
            "spectrum evaluation frequency must be finite and > 0".into(),
        ));
    }
    if nu < spec.nu_min || nu > spec.nu_max {
        return Ok(0.0);
    }
    let c = continuum_normalization(spec)?;
    let v = c * lognormal_kernel(nu, spec.mu, spec.sigma);
    if !v.is_finite() || v < 0.0 {
        return Err(SpectralRenderError::InvalidIntensity(
            "continuum phi non-finite or negative".into(),
        ));
    }
    Ok(v)
}

/// Hermetic narrow Gaussian line in frequency (tests / line-shift report only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticLineFixture {
    pub schema_version: u32,
    pub spectrum_id: String,
    pub nu0: f64,
    pub sigma: f64,
    pub amplitude: f64,
}

pub fn diagnostic_gaussian_line_v1(nu0: f64) -> DiagnosticLineFixture {
    DiagnosticLineFixture {
        schema_version: 1,
        spectrum_id: LINE_FIXTURE_ID.into(),
        nu0,
        sigma: 0.02,
        amplitude: 1.0,
    }
}

pub fn evaluate_line_fixture(
    line: &DiagnosticLineFixture,
    nu: f64,
) -> Result<f64, SpectralRenderError> {
    if !nu.is_finite() || !(nu > 0.0) {
        return Err(SpectralRenderError::InvalidFrequency(
            "line evaluation frequency must be finite and > 0".into(),
        ));
    }
    if !(line.sigma > 0.0) || !line.amplitude.is_finite() || line.amplitude < 0.0 {
        return Err(SpectralRenderError::InvalidSpectrumSpec(
            "invalid line fixture parameters".into(),
        ));
    }
    let z = (nu - line.nu0) / line.sigma;
    let v = line.amplitude * (-0.5 * z * z).exp();
    if !v.is_finite() || v < 0.0 {
        return Err(SpectralRenderError::InvalidIntensity(
            "line intensity non-finite".into(),
        ));
    }
    Ok(v)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskSpectralConvention {
    pub schema_version: u32,
    pub convention_id: String,
    pub continuum_spectrum_id: String,
    pub intensity_units: String,
    pub transport_law: String,
    pub transport_arithmetic: String,
    pub frequency_shift_source: String,
    pub bolometric_source: String,
    pub emitter_domain_policy: String,
    pub spectral_status: String,
    pub physical_rgb_status: String,
    pub accepted_emission_model: String,
    pub accepted_emission_claim: String,
}

impl DiskSpectralConvention {
    pub fn v1() -> Self {
        Self {
            schema_version: 1,
            convention_id: SPECTRAL_CONVENTION_ID.into(),
            continuum_spectrum_id: CONTINUUM_SPECTRUM_ID.into(),
            intensity_units: SPECTRAL_UNITS_V1.into(),
            transport_law: "observed-i-nu-equals-g-cubed-times-emitted-i-nu-at-nu-obs-over-g"
                .into(),
            transport_arithmetic: "g2-equals-g-times-g-g3-equals-g2-times-g".into(),
            frequency_shift_source: "gate-2b0-frequency-shift-frame".into(),
            bolometric_source: "gate-2b1-bolometric-frame".into(),
            emitter_domain_policy: EMITTER_DOMAIN_POLICY.into(),
            spectral_status: "sampled-diagnostic-i-nu-v1".into(),
            physical_rgb_status: "not-implemented".into(),
            accepted_emission_model: CANONICAL_DISK_EMISSION_MODEL.into(),
            accepted_emission_claim: CANONICAL_DISK_EMISSION_CLAIM.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralDiskSample {
    pub continuum_spectrum_id: String,
    pub radius: f64,
    pub azimuth: f64,
    pub g_factor: f64,
    pub emitted_bolometric_intensity: f64,
    pub observed_bolometric_intensity: f64,
    pub integrated_emitted_i_nu: f64,
    pub integrated_observed_i_nu: f64,
    pub truncated_emitted_energy_fraction: f64,
    pub truncated_observed_energy_fraction: f64,
    pub velocity_model: DiskVelocityModel,
    pub resolved_direction: EquatorialAngularDirection,
    pub disk_event_value: f64,
    /// Observer-frame `I_ν,obs` samples (length = n_bins).
    pub i_nu_obs: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpectralPixel {
    DiskHit(SpectralDiskSample),
    NotDiskHit { outcome_class: OutcomeClass },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpectralFrame {
    grid: TraceGrid,
    spectral_grid: SpectralGrid,
    pixels: Vec<SpectralPixel>,
}

impl SpectralFrame {
    pub fn try_new(
        grid: TraceGrid,
        spectral_grid: SpectralGrid,
        pixels: Vec<SpectralPixel>,
    ) -> Result<Self, SpectralRenderError> {
        spectral_grid
            .validate()
            .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))?;
        if pixels.len() != grid.pixel_count() {
            return Err(SpectralRenderError::FrameLengthMismatch);
        }
        Ok(Self {
            grid,
            spectral_grid,
            pixels,
        })
    }

    pub fn grid(&self) -> TraceGrid {
        self.grid
    }

    pub fn spectral_grid(&self) -> &SpectralGrid {
        &self.spectral_grid
    }

    pub fn pixels(&self) -> &[SpectralPixel] {
        &self.pixels
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &SpectralPixel {
        &self.pixels[pixel_index(self.grid, col, row)]
    }
}

fn map_disk_hit(
    fs: &crate::frequency_shift::DiskFrequencyShiftSample,
    bolo: &crate::bolometric::DiskBolometricSample,
    continuum: &DiagnosticSpectrumSpec,
    spectral_grid: &SpectralGrid,
) -> Result<SpectralDiskSample, SpectralRenderError> {
    if fs.g_factor.to_bits() != bolo.g_factor.to_bits() {
        return Err(SpectralRenderError::ProvenanceMismatch(
            "frequency and bolometric g_factor mismatch".into(),
        ));
    }
    let g = FrequencyShift::new(bolo.g_factor).map_err(|e| {
        SpectralRenderError::InvalidFrequency(format!("g from bolometric frame: {e}"))
    })?;
    let i_em = bolo.emitted_bolometric_intensity;
    let n = spectral_grid.n_bins() as usize;
    let mut i_nu_obs = vec![0.0; n];
    let mut integ_em = 0.0;
    let mut integ_obs = 0.0;

    let g_val = g.value();
    for (i, (&nu_obs, &w)) in spectral_grid
        .centers()
        .iter()
        .zip(spectral_grid.weights().iter())
        .enumerate()
    {
        let nu_em = nu_obs / g_val;
        let phi = evaluate_continuum_phi(continuum, nu_em)?;
        let i_em_nu = i_em * phi;
        let i_obs_nu = transport_i_nu(i_em_nu, g_val)
            .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))?;
        i_nu_obs[i] = i_obs_nu;
        // Emitted integral over ν_em: on the observer grid, dν_em = dν_obs / g.
        let w_em = w / g_val;
        integ_em += i_em_nu * w_em;
        integ_obs += i_obs_nu * w;
    }

    // Continuum mass captured by mapping the observer grid into emitter frequency.
    let captured = continuum_mass_on_interval(
        continuum,
        spectral_grid.nu_min() / g_val,
        spectral_grid.nu_max() / g_val,
    )?;
    let frac_trunc = (1.0 - captured).clamp(0.0, 1.0);

    Ok(SpectralDiskSample {
        continuum_spectrum_id: CONTINUUM_SPECTRUM_ID.into(),
        radius: bolo.radius,
        azimuth: bolo.azimuth,
        g_factor: bolo.g_factor,
        emitted_bolometric_intensity: bolo.emitted_bolometric_intensity,
        observed_bolometric_intensity: bolo.observed_bolometric_intensity,
        integrated_emitted_i_nu: integ_em,
        integrated_observed_i_nu: integ_obs,
        truncated_emitted_energy_fraction: frac_trunc,
        truncated_observed_energy_fraction: frac_trunc,
        velocity_model: bolo.velocity_model,
        resolved_direction: bolo.resolved_direction,
        disk_event_value: bolo.disk_event_value,
        i_nu_obs,
    })
}

/// Build `SpectralFrame` from accepted Gate 2B0 + 2B1 frames (no retrace).
pub fn build_disk_spectral_frame(
    frequency_frame: &DiskFrequencyShiftFrame,
    bolometric_frame: &DiskBolometricFrame,
    continuum: &DiagnosticSpectrumSpec,
    spectral_grid: &SpectralGrid,
    bounds: ResolvedDiskBounds,
) -> Result<SpectralFrame, SpectralRenderError> {
    continuum.validate()?;
    bounds
        .validate()
        .map_err(|e| SpectralRenderError::ProvenanceMismatch(e.to_string()))?;
    spectral_grid
        .validate()
        .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))?;
    if frequency_frame.grid() != bolometric_frame.grid() {
        return Err(SpectralRenderError::GridMismatch);
    }
    let grid = frequency_frame.grid();
    let mut pixels = Vec::with_capacity(grid.pixel_count());
    for row in 0..grid.height {
        for col in 0..grid.width {
            let pixel = match (
                frequency_frame.pixel_at(col, row),
                bolometric_frame.pixel_at(col, row),
            ) {
                (DiskFrequencyShiftPixel::DiskHit(fs), DiskBolometricPixel::DiskHit(bolo)) => {
                    let sample = map_disk_hit(fs, bolo, continuum, spectral_grid).map_err(|e| {
                        SpectralRenderError::PixelMappingFailed {
                            col,
                            row,
                            cause: e.to_string(),
                        }
                    })?;
                    SpectralPixel::DiskHit(sample)
                }
                (
                    DiskFrequencyShiftPixel::NotDiskHit { outcome_class: a },
                    DiskBolometricPixel::NotDiskHit { outcome_class: b },
                ) => {
                    if a != b {
                        return Err(SpectralRenderError::ProvenanceMismatch(format!(
                            "outcome class mismatch at ({col},{row})"
                        )));
                    }
                    SpectralPixel::NotDiskHit { outcome_class: *a }
                }
                _ => {
                    return Err(SpectralRenderError::ProvenanceMismatch(format!(
                        "disk/non-disk kind mismatch at ({col},{row})"
                    )));
                }
            };
            pixels.push(pixel);
        }
    }
    SpectralFrame::try_new(grid, spectral_grid.clone(), pixels)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralClosureMetrics {
    pub disk_hit_count: u64,
    /// True maximum absolute emitted closure error across disk hits.
    pub max_abs_emitted_closure_error: f64,
    /// True maximum relative emitted closure error across disk hits.
    pub max_rel_emitted_closure_error: f64,
    /// True maximum absolute observed closure error across disk hits.
    pub max_abs_observed_closure_error: f64,
    /// True maximum relative observed closure error across disk hits.
    pub max_rel_observed_closure_error: f64,
    pub rmse_emitted: f64,
    pub rmse_observed: f64,
    /// Source index of the disk-hit with maximum relative emitted error.
    pub worst_emitted_source_index: u64,
    /// Source index of the disk-hit with maximum relative observed error.
    pub worst_observed_source_index: u64,
}

pub fn compute_bolometric_closure(
    frame: &SpectralFrame,
) -> Result<SpectralClosureMetrics, SpectralRenderError> {
    let mut n = 0u64;
    let mut max_abs_e = 0.0;
    let mut max_rel_e = 0.0;
    let mut max_abs_o = 0.0;
    let mut max_rel_o = 0.0;
    let mut sse_e = 0.0;
    let mut sse_o = 0.0;
    let mut worst_e_idx = 0u64;
    let mut worst_o_idx = 0u64;

    for (idx, pixel) in frame.pixels().iter().enumerate() {
        let SpectralPixel::DiskHit(s) = pixel else {
            continue;
        };
        n += 1;
        let captured = (1.0 - s.truncated_emitted_energy_fraction).clamp(0.0, 1.0);
        let expected_e = s.emitted_bolometric_intensity * captured;
        let expected_o = s.observed_bolometric_intensity * captured;
        let err_e = (s.integrated_emitted_i_nu - expected_e).abs();
        let err_o = (s.integrated_observed_i_nu - expected_o).abs();
        let rel_e = if expected_e > 0.0 {
            err_e / expected_e
        } else if err_e == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        let rel_o = if expected_o > 0.0 {
            err_o / expected_o
        } else if err_o == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        if !err_e.is_finite() || !err_o.is_finite() || !rel_e.is_finite() || !rel_o.is_finite() {
            return Err(SpectralRenderError::VerificationFailed {
                col: (idx as u32) % frame.grid().width,
                row: (idx as u32) / frame.grid().width,
                cause: "non-finite bolometric closure".into(),
            });
        }
        sse_e += err_e * err_e;
        sse_o += err_o * err_o;
        // Absolute and relative maxima are independent authorities.
        if err_e > max_abs_e {
            max_abs_e = err_e;
        }
        if err_o > max_abs_o {
            max_abs_o = err_o;
        }
        if rel_e > max_rel_e {
            max_rel_e = rel_e;
            worst_e_idx = idx as u64;
        }
        if rel_o > max_rel_o {
            max_rel_o = rel_o;
            worst_o_idx = idx as u64;
        }
    }

    let rmse_e = if n == 0 {
        0.0
    } else {
        (sse_e / n as f64).sqrt()
    };
    let rmse_o = if n == 0 {
        0.0
    } else {
        (sse_o / n as f64).sqrt()
    };

    Ok(SpectralClosureMetrics {
        disk_hit_count: n,
        max_abs_emitted_closure_error: max_abs_e,
        max_rel_emitted_closure_error: max_rel_e,
        max_abs_observed_closure_error: max_abs_o,
        max_rel_observed_closure_error: max_rel_o,
        rmse_emitted: rmse_e,
        rmse_observed: rmse_o,
        worst_emitted_source_index: worst_e_idx,
        worst_observed_source_index: worst_o_idx,
    })
}

pub fn diagnostic_spectrum_spec_digest(spec: &DiagnosticSpectrumSpec) -> String {
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"diagnostic-spectrum-spec-digest-v1");
    h.update(spec.schema_version.to_le_bytes());
    update_tagged_str(&mut h, b"spectrum-id", &spec.spectrum_id);
    update_tagged_str(&mut h, b"measure", spec.measure.digest_tag());
    h.update(spec.mu.to_bits().to_le_bytes());
    h.update(spec.sigma.to_bits().to_le_bytes());
    h.update(spec.nu_min.to_bits().to_le_bytes());
    h.update(spec.nu_max.to_bits().to_le_bytes());
    update_tagged_str(&mut h, b"units", &spec.units);
    hex_sha(&h.finalize())
}

pub fn spectral_grid_digest(grid: &SpectralGrid) -> Result<String, SpectralRenderError> {
    grid.validate()
        .map_err(|e| SpectralRenderError::InvalidGrid(e.to_string()))?;
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"spectral-grid-digest-v1");
    update_tagged_str(&mut h, b"grid-id", grid.grid_id());
    update_tagged_str(&mut h, b"measure", grid.measure().digest_tag());
    h.update(grid.nu_min().to_bits().to_le_bytes());
    h.update(grid.nu_max().to_bits().to_le_bytes());
    h.update(grid.n_bins().to_le_bytes());
    for e in grid.edges() {
        h.update(e.to_bits().to_le_bytes());
    }
    for c in grid.centers() {
        h.update(c.to_bits().to_le_bytes());
    }
    for w in grid.weights() {
        h.update(w.to_bits().to_le_bytes());
    }
    Ok(hex_sha(&h.finalize()))
}

pub fn disk_spectral_digest(
    frame: &SpectralFrame,
    convention: &DiskSpectralConvention,
    continuum: &DiagnosticSpectrumSpec,
    source_frequency_shift_digest: &str,
    source_bolometric_digest: &str,
) -> Result<String, SpectralRenderError> {
    continuum.validate()?;
    let mut h = Sha256::new();
    update_tagged_bytes(&mut h, b"domain", b"disk-spectral-digest-v1");
    hash_convention(&mut h, convention);
    update_tagged_str(
        &mut h,
        b"continuum-digest",
        &diagnostic_spectrum_spec_digest(continuum),
    );
    update_tagged_str(
        &mut h,
        b"spectral-grid-digest",
        &spectral_grid_digest(frame.spectral_grid())?,
    );
    update_tagged_str(
        &mut h,
        b"source-frequency-shift-digest",
        source_frequency_shift_digest,
    );
    update_tagged_str(
        &mut h,
        b"source-bolometric-digest",
        source_bolometric_digest,
    );
    h.update(frame.grid().width.to_le_bytes());
    h.update(frame.grid().height.to_le_bytes());
    for (idx, pixel) in frame.pixels().iter().enumerate() {
        h.update((idx as u64).to_le_bytes());
        match pixel {
            SpectralPixel::DiskHit(s) => {
                update_tagged_str(&mut h, b"kind", "disk-hit");
                update_tagged_str(&mut h, b"continuum-id", &s.continuum_spectrum_id);
                h.update(s.radius.to_bits().to_le_bytes());
                h.update(s.azimuth.to_bits().to_le_bytes());
                h.update(s.g_factor.to_bits().to_le_bytes());
                h.update(s.emitted_bolometric_intensity.to_bits().to_le_bytes());
                h.update(s.observed_bolometric_intensity.to_bits().to_le_bytes());
                h.update(s.integrated_emitted_i_nu.to_bits().to_le_bytes());
                h.update(s.integrated_observed_i_nu.to_bits().to_le_bytes());
                h.update(s.truncated_emitted_energy_fraction.to_bits().to_le_bytes());
                h.update(s.truncated_observed_energy_fraction.to_bits().to_le_bytes());
                update_tagged_str(&mut h, b"velocity-model", s.velocity_model.digest_tag());
                update_tagged_str(&mut h, b"direction", s.resolved_direction.digest_tag());
                h.update(s.disk_event_value.to_bits().to_le_bytes());
                h.update((s.i_nu_obs.len() as u64).to_le_bytes());
                for v in &s.i_nu_obs {
                    h.update(v.to_bits().to_le_bytes());
                }
            }
            SpectralPixel::NotDiskHit { outcome_class } => {
                update_tagged_str(&mut h, b"kind", "not-disk-hit");
                update_tagged_str(&mut h, b"outcome-class", outcome_class.digest_tag());
            }
        }
    }
    Ok(hex_sha(&h.finalize()))
}

fn hash_convention(h: &mut Sha256, c: &DiskSpectralConvention) {
    h.update(c.schema_version.to_le_bytes());
    update_tagged_str(h, b"convention-id", &c.convention_id);
    update_tagged_str(h, b"continuum-spectrum-id", &c.continuum_spectrum_id);
    update_tagged_str(h, b"units", &c.intensity_units);
    update_tagged_str(h, b"transport-law", &c.transport_law);
    update_tagged_str(h, b"transport-arithmetic", &c.transport_arithmetic);
    update_tagged_str(h, b"frequency-source", &c.frequency_shift_source);
    update_tagged_str(h, b"bolometric-source", &c.bolometric_source);
    update_tagged_str(h, b"domain-policy", &c.emitter_domain_policy);
    update_tagged_str(h, b"spectral-status", &c.spectral_status);
    update_tagged_str(h, b"physical-rgb-status", &c.physical_rgb_status);
    update_tagged_str(h, b"accepted-emission-model", &c.accepted_emission_model);
    update_tagged_str(h, b"accepted-emission-claim", &c.accepted_emission_claim);
}

fn update_tagged_bytes(h: &mut Sha256, tag: &[u8], value: &[u8]) {
    h.update((tag.len() as u64).to_le_bytes());
    h.update(tag);
    h.update((value.len() as u64).to_le_bytes());
    h.update(value);
}

fn update_tagged_str(h: &mut Sha256, tag: &[u8], value: &str) {
    update_tagged_bytes(h, tag, value.as_bytes());
}

/// Independent pointwise check used by tests (must not call builder helpers).
pub fn independent_i_nu_obs(
    i_em_bol: f64,
    g: f64,
    nu_obs: f64,
    spec: &DiagnosticSpectrumSpec,
) -> Result<f64, SpectralRenderError> {
    let nu_em = nu_obs / g;
    let phi = evaluate_continuum_phi(spec, nu_em)?;
    transport_i_nu(i_em_bol * phi, g)
        .map_err(|e| SpectralRenderError::InvalidIntensity(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuum_phi_integrates_near_one_on_fine_grid() {
        let spec = diagnostic_lognormal_continuum_v1();
        let grid = SpectralGrid::log_spaced("fine", spec.nu_min, spec.nu_max, 256).unwrap();
        let mut samples = Vec::new();
        for &c in grid.centers() {
            samples.push(evaluate_continuum_phi(&spec, c).unwrap());
        }
        let integ = grid.integrate(&samples).unwrap();
        assert!(
            (integ - 1.0).abs() < 5e-3,
            "phi integral {integ} not near 1"
        );
    }

    #[test]
    fn transport_identity_g_one() {
        let spec = diagnostic_lognormal_continuum_v1();
        let nu = 1.0;
        let i = independent_i_nu_obs(2.0, 1.0, nu, &spec).unwrap();
        let phi = evaluate_continuum_phi(&spec, nu).unwrap();
        assert!((i - 2.0 * phi).abs() < 1e-14);
    }

    #[test]
    fn wrong_g4_pointwise_differs_from_g3() {
        let g = 0.5;
        let i_em = 1.0;
        let g3 = transport_i_nu(i_em, g).unwrap();
        let g4 = i_em * g * g * g * g;
        assert!((g3 - g4).abs() > 1e-6);
    }

    #[test]
    fn line_fixture_peaks_at_nu0() {
        let line = diagnostic_gaussian_line_v1(1.2);
        let a = evaluate_line_fixture(&line, 1.2).unwrap();
        let b = evaluate_line_fixture(&line, 1.0).unwrap();
        assert!(a > b);
    }

    #[test]
    fn continuum_zero_outside_domain() {
        let spec = diagnostic_lognormal_continuum_v1();
        assert_eq!(evaluate_continuum_phi(&spec, 0.1).unwrap(), 0.0);
        assert_eq!(evaluate_continuum_phi(&spec, 10.0).unwrap(), 0.0);
    }

    #[test]
    fn line_shift_amplitude_scales_g3() {
        let line = diagnostic_gaussian_line_v1(1.0);
        let g = 2.0;
        let nu_obs = 2.0; // → ν_em = 1.0 peak
        let i_em = evaluate_line_fixture(&line, nu_obs / g).unwrap();
        let i_obs = transport_i_nu(i_em, g).unwrap();
        assert!((i_obs - i_em * g * g * g).abs() < 1e-14);
        // Wrong sampling I_em(g ν) must differ from I_em(ν/g) for g≠1.
        let wrong = evaluate_line_fixture(&line, g * nu_obs).unwrap();
        assert!((wrong - i_em).abs() > 1e-6);
    }

    #[test]
    fn wavelength_jacobian_g5_identity() {
        use relativity_core::{i_lambda_from_i_nu, wavelength_from_frequency, Frequency};
        let g = 0.5;
        let nu_obs = Frequency::new(1.0).unwrap();
        let i_nu_em = 3.0;
        let i_nu_obs = transport_i_nu(i_nu_em, g).unwrap();
        let lam_obs = wavelength_from_frequency(nu_obs).unwrap();
        let lam_em =
            wavelength_from_frequency(Frequency::new(nu_obs.value() / g).unwrap()).unwrap();
        let i_lam_em = i_lambda_from_i_nu(i_nu_em, lam_em).unwrap();
        let i_lam_obs = i_lambda_from_i_nu(i_nu_obs, lam_obs).unwrap();
        let expected = i_lam_em * g.powi(5);
        assert!(
            (i_lam_obs - expected).abs() < 1e-12,
            "I_λ obs {i_lam_obs} vs g⁵ I_λ em {expected}"
        );
    }

    #[test]
    fn bolometric_recovery_g4_from_g3_samples() {
        let spec = diagnostic_lognormal_continuum_v1();
        let g = 1.7;
        let i_em_bol = 0.4;
        let grid = SpectralGrid::log_spaced("t", spec.nu_min, spec.nu_max, 256).unwrap();
        let mut obs = Vec::new();
        let mut integ_em = 0.0;
        for (&nu_obs, &w) in grid.centers().iter().zip(grid.weights().iter()) {
            let phi = evaluate_continuum_phi(&spec, nu_obs / g).unwrap();
            let i_em = i_em_bol * phi;
            integ_em += i_em * (w / g);
            obs.push(transport_i_nu(i_em, g).unwrap());
        }
        let integ_obs = grid.integrate(&obs).unwrap();
        let expected_obs = i_em_bol * g.powi(4);
        // Domain truncation + quadrature: allow moderate relative error at g≠1.
        assert!(
            (integ_obs - expected_obs).abs() / expected_obs < 0.05,
            "obs integ {integ_obs} vs g⁴ I_em {expected_obs}"
        );
        assert!(
            (integ_em - i_em_bol).abs() / i_em_bol < 0.05,
            "em integ {integ_em} vs I_em {i_em_bol}"
        );
    }

    #[test]
    fn closure_max_abs_independent_of_max_rel() {
        use relativity_core::EquatorialAngularDirection;
        use relativity_trace::{OutcomeClass, TraceGrid};

        fn sample(i_em: f64, i_obs: f64, integ_em: f64, integ_obs: f64) -> SpectralDiskSample {
            SpectralDiskSample {
                continuum_spectrum_id: CONTINUUM_SPECTRUM_ID.into(),
                radius: 10.0,
                azimuth: 0.0,
                g_factor: 1.0,
                emitted_bolometric_intensity: i_em,
                observed_bolometric_intensity: i_obs,
                integrated_emitted_i_nu: integ_em,
                integrated_observed_i_nu: integ_obs,
                truncated_emitted_energy_fraction: 0.0,
                truncated_observed_energy_fraction: 0.0,
                velocity_model: DiskVelocityModel::ProgradeCircularGeodesic,
                resolved_direction: EquatorialAngularDirection::PositivePhi,
                disk_event_value: 0.0,
                i_nu_obs: vec![0.0; 2],
            }
        }

        // Pixel 0: large absolute, small relative (1%).
        // Pixel 1: small absolute, large relative (50%).
        let pixels = vec![
            SpectralPixel::DiskHit(sample(100.0, 100.0, 101.0, 101.0)),
            SpectralPixel::DiskHit(sample(1.0, 1.0, 1.5, 1.5)),
            SpectralPixel::NotDiskHit {
                outcome_class: OutcomeClass::Escaped,
            },
        ];
        let grid = TraceGrid {
            width: 3,
            height: 1,
        };
        let sgrid = SpectralGrid::log_spaced("hermetic-2", 0.25, 4.0, 2).unwrap();
        let frame = SpectralFrame::try_new(grid, sgrid, pixels).unwrap();
        let m = compute_bolometric_closure(&frame).unwrap();
        assert!((m.max_abs_emitted_closure_error - 1.0).abs() < 1e-14);
        assert!((m.max_rel_emitted_closure_error - 0.5).abs() < 1e-14);
        assert_eq!(m.worst_emitted_source_index, 1);
        assert!((m.max_abs_observed_closure_error - 1.0).abs() < 1e-14);
        assert!((m.max_rel_observed_closure_error - 0.5).abs() < 1e-14);
    }
}
