//! Spectral measure, frequency/wavelength newtypes, and observer-frame grids.
//!
//! Gate 2B2 primitives. No image I/O. Canonical transported quantity is `I_ν`.

use crate::error::CoreError;
use serde::{Deserialize, Serialize};

/// Which spectral density is being represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpectralMeasure {
    FrequencySpecificIntensity,
    WavelengthSpecificIntensity,
}

impl SpectralMeasure {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrequencySpecificIntensity => "frequency-specific-intensity",
            Self::WavelengthSpecificIntensity => "wavelength-specific-intensity",
        }
    }

    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::FrequencySpecificIntensity => "spectral-measure:frequency-specific-intensity",
            Self::WavelengthSpecificIntensity => "spectral-measure:wavelength-specific-intensity",
        }
    }
}

/// Strictly positive finite frequency (diagnostic or SI — caller documents units).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Frequency(f64);

impl Frequency {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        if !value.is_finite() {
            return Err(CoreError::InvalidFrequency {
                context: "non-finite spectral frequency",
            });
        }
        if !(value > 0.0) {
            return Err(CoreError::InvalidFrequency {
                context: "spectral frequency must be strictly positive",
            });
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

/// Strictly positive finite wavelength.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wavelength(f64);

impl Wavelength {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        if !value.is_finite() {
            return Err(CoreError::InvalidFrequency {
                context: "non-finite spectral wavelength",
            });
        }
        if !(value > 0.0) {
            return Err(CoreError::InvalidFrequency {
                context: "spectral wavelength must be strictly positive",
            });
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

/// Diagnostic conversion constant for `λ = C / ν` (not a SI claim in digests).
pub const DIAGNOSTIC_WAVELENGTH_FREQUENCY_PRODUCT: f64 = 1.0;

pub fn wavelength_from_frequency(nu: Frequency) -> Result<Wavelength, CoreError> {
    Wavelength::new(DIAGNOSTIC_WAVELENGTH_FREQUENCY_PRODUCT / nu.value())
}

pub fn frequency_from_wavelength(lambda: Wavelength) -> Result<Frequency, CoreError> {
    Frequency::new(DIAGNOSTIC_WAVELENGTH_FREQUENCY_PRODUCT / lambda.value())
}

/// `I_λ = I_ν · (C / λ²)` with `C = DIAGNOSTIC_WAVELENGTH_FREQUENCY_PRODUCT`.
pub fn i_lambda_from_i_nu(i_nu: f64, lambda: Wavelength) -> Result<f64, CoreError> {
    if !i_nu.is_finite() || i_nu < 0.0 {
        return Err(CoreError::InvalidFrequency {
            context: "I_nu for wavelength conversion must be finite and >= 0",
        });
    }
    let l = lambda.value();
    let out = i_nu * DIAGNOSTIC_WAVELENGTH_FREQUENCY_PRODUCT / (l * l);
    if !out.is_finite() || out < 0.0 {
        return Err(CoreError::InvalidFrequency {
            context: "I_lambda conversion produced non-finite or negative value",
        });
    }
    Ok(out)
}

/// `I_ν = I_λ · (λ² / C)`.
pub fn i_nu_from_i_lambda(i_lambda: f64, lambda: Wavelength) -> Result<f64, CoreError> {
    if !i_lambda.is_finite() || i_lambda < 0.0 {
        return Err(CoreError::InvalidFrequency {
            context: "I_lambda for frequency conversion must be finite and >= 0",
        });
    }
    let l = lambda.value();
    let out = i_lambda * (l * l) / DIAGNOSTIC_WAVELENGTH_FREQUENCY_PRODUCT;
    if !out.is_finite() || out < 0.0 {
        return Err(CoreError::InvalidFrequency {
            context: "I_nu conversion produced non-finite or negative value",
        });
    }
    Ok(out)
}

/// Vacuum invariant transport: `I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs/g)`.
pub fn transport_i_nu(i_nu_em: f64, g: f64) -> Result<f64, CoreError> {
    if !i_nu_em.is_finite() || i_nu_em < 0.0 {
        return Err(CoreError::InvalidFrequency {
            context: "emitted I_nu must be finite and >= 0",
        });
    }
    if !g.is_finite() || !(g > 0.0) {
        return Err(CoreError::InvalidFrequency {
            context: "g for spectral transport must be finite and > 0",
        });
    }
    let g2 = g * g;
    let g3 = g2 * g;
    let out = i_nu_em * g3;
    if !out.is_finite() || out < 0.0 {
        return Err(CoreError::InvalidFrequency {
            context: "transported I_nu non-finite or negative",
        });
    }
    Ok(out)
}

/// Log-spaced observer-frame frequency grid (canonical Gate 2B2 layout metadata).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpectralGrid {
    measure: SpectralMeasure,
    grid_id: String,
    nu_min: f64,
    nu_max: f64,
    n_bins: u32,
    edges: Vec<f64>,
    centers: Vec<f64>,
    weights: Vec<f64>,
}

impl SpectralGrid {
    pub const V1_ID: &'static str = "spectral-grid-v1";
    pub const V1_N_BINS: u32 = 64;
    pub const V1_NU_MIN: f64 = 0.25;
    pub const V1_NU_MAX: f64 = 4.0;
    pub const MAX_BINS: u32 = 4096;

    pub fn log_spaced(
        grid_id: impl Into<String>,
        nu_min: f64,
        nu_max: f64,
        n_bins: u32,
    ) -> Result<Self, CoreError> {
        if n_bins == 0 || n_bins > Self::MAX_BINS {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid bin count out of bounds",
            });
        }
        if !nu_min.is_finite() || !nu_max.is_finite() || !(nu_min > 0.0) || !(nu_max > nu_min) {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid requires 0 < nu_min < nu_max finite",
            });
        }
        let n = n_bins as usize;
        let mut edges = Vec::with_capacity(n + 1);
        let ln_min = nu_min.ln();
        let ln_max = nu_max.ln();
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let edge = (ln_min + t * (ln_max - ln_min)).exp();
            if !edge.is_finite() || !(edge > 0.0) {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral grid edge non-finite",
                });
            }
            edges.push(edge);
        }
        // Exact endpoints.
        edges[0] = nu_min;
        edges[n] = nu_max;

        let mut centers = Vec::with_capacity(n);
        let mut weights = Vec::with_capacity(n);
        for i in 0..n {
            let lo = edges[i];
            let hi = edges[i + 1];
            let c = (lo * hi).sqrt();
            let w = hi - lo;
            if !c.is_finite() || !(c > 0.0) || !w.is_finite() || !(w > 0.0) {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral grid center/weight invalid",
                });
            }
            centers.push(c);
            weights.push(w);
        }

        Ok(Self {
            measure: SpectralMeasure::FrequencySpecificIntensity,
            grid_id: grid_id.into(),
            nu_min,
            nu_max,
            n_bins,
            edges,
            centers,
            weights,
        })
    }

    pub fn spectral_grid_v1() -> Result<Self, CoreError> {
        Self::log_spaced(
            Self::V1_ID,
            Self::V1_NU_MIN,
            Self::V1_NU_MAX,
            Self::V1_N_BINS,
        )
    }

    #[must_use]
    pub fn measure(&self) -> SpectralMeasure {
        self.measure
    }

    #[must_use]
    pub fn grid_id(&self) -> &str {
        &self.grid_id
    }

    #[must_use]
    pub fn nu_min(&self) -> f64 {
        self.nu_min
    }

    #[must_use]
    pub fn nu_max(&self) -> f64 {
        self.nu_max
    }

    #[must_use]
    pub fn n_bins(&self) -> u32 {
        self.n_bins
    }

    #[must_use]
    pub fn edges(&self) -> &[f64] {
        &self.edges
    }

    #[must_use]
    pub fn centers(&self) -> &[f64] {
        &self.centers
    }

    #[must_use]
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    pub fn integrate(&self, samples: &[f64]) -> Result<f64, CoreError> {
        if samples.len() != self.n_bins as usize {
            return Err(CoreError::InvalidFrequency {
                context: "spectral integrate length mismatch",
            });
        }
        let mut acc = 0.0;
        for (s, w) in samples.iter().zip(self.weights.iter()) {
            if !s.is_finite() || *s < 0.0 || !w.is_finite() {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral integrate non-finite or negative sample",
                });
            }
            acc += s * w;
        }
        if !acc.is_finite() || acc < 0.0 {
            return Err(CoreError::InvalidFrequency {
                context: "spectral integral non-finite",
            });
        }
        Ok(acc)
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        if self.measure != SpectralMeasure::FrequencySpecificIntensity {
            return Err(CoreError::InvalidFrequency {
                context: "canonical spectral grid must be frequency measure",
            });
        }
        if self.n_bins == 0 || self.centers.len() != self.n_bins as usize {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid bin metadata inconsistent",
            });
        }
        if self.edges.len() != self.n_bins as usize + 1
            || self.weights.len() != self.n_bins as usize
        {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid edge/weight length inconsistent",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_rejects_non_positive() {
        assert!(Frequency::new(0.0).is_err());
        assert!(Frequency::new(-1.0).is_err());
        assert!(Frequency::new(f64::NAN).is_err());
        assert!(Frequency::new(1.0).is_ok());
    }

    #[test]
    fn transport_i_nu_scales_g3() {
        let i = transport_i_nu(2.0, 0.5).unwrap();
        assert!((i - 2.0 * 0.125).abs() < 1e-15);
        let i2 = transport_i_nu(2.0, 2.0).unwrap();
        assert!((i2 - 2.0 * 8.0).abs() < 1e-15);
        assert!(transport_i_nu(-1.0, 1.0).is_err());
        assert!(transport_i_nu(1.0, 0.0).is_err());
    }

    #[test]
    fn wavelength_jacobian_roundtrip() {
        let nu = Frequency::new(2.0).unwrap();
        let lam = wavelength_from_frequency(nu).unwrap();
        let i_nu = 3.0;
        let i_l = i_lambda_from_i_nu(i_nu, lam).unwrap();
        let back = i_nu_from_i_lambda(i_l, lam).unwrap();
        assert!((back - i_nu).abs() < 1e-14);
    }

    #[test]
    fn wavelength_transport_g5() {
        // I_λ,obs(λ_obs) = g⁵ I_λ,em(g λ_obs)
        let g = 0.5;
        let lambda_obs = Wavelength::new(1.0).unwrap();
        let lambda_em = Wavelength::new(g * lambda_obs.value()).unwrap();
        let i_nu_em = 4.0;
        let i_l_em = i_lambda_from_i_nu(i_nu_em, lambda_em).unwrap();
        let nu_obs = frequency_from_wavelength(lambda_obs).unwrap();
        let nu_em = Frequency::new(nu_obs.value() / g).unwrap();
        assert!(
            (nu_em.value() - frequency_from_wavelength(lambda_em).unwrap().value()).abs() < 1e-12
        );
        let i_nu_obs = transport_i_nu(i_nu_em, g).unwrap();
        let i_l_obs = i_lambda_from_i_nu(i_nu_obs, lambda_obs).unwrap();
        let expected = g.powi(5) * i_l_em;
        assert!((i_l_obs - expected).abs() < 1e-12);
    }

    #[test]
    fn spectral_grid_v1_integrates_flat() {
        let g = SpectralGrid::spectral_grid_v1().unwrap();
        g.validate().unwrap();
        assert_eq!(g.n_bins(), 64);
        let ones = vec![1.0; g.n_bins() as usize];
        let integ = g.integrate(&ones).unwrap();
        assert!((integ - (g.nu_max() - g.nu_min())).abs() < 1e-12);
    }

    #[test]
    fn spectral_grid_rejects_bad_bounds() {
        assert!(SpectralGrid::log_spaced("x", 1.0, 1.0, 8).is_err());
        assert!(SpectralGrid::log_spaced("x", 0.1, 1.0, 0).is_err());
    }
}
