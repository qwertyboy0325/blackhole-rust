//! Spectral measure, frequency/wavelength newtypes, and observer-frame grids.
//!
//! Gate 2B2 primitives. No image I/O. Canonical transported quantity is `I_ν`.

use crate::error::CoreError;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use std::fmt;

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
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
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

impl<'de> Deserialize<'de> for Frequency {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        Frequency::new(v).map_err(de::Error::custom)
    }
}

/// Strictly positive finite wavelength.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
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

impl<'de> Deserialize<'de> for Wavelength {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        Wavelength::new(v).map_err(de::Error::custom)
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
///
/// Construction and deserialization both run [`SpectralGrid::validate`].
#[derive(Debug, Clone, PartialEq)]
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
    /// Frozen authoritative bin count (Gate 2B2 closure `5203577417`).
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

        let grid = Self {
            measure: SpectralMeasure::FrequencySpecificIntensity,
            grid_id: grid_id.into(),
            nu_min,
            nu_max,
            n_bins,
            edges,
            centers,
            weights,
        };
        grid.validate()?;
        Ok(grid)
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
        self.validate()?;
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

    /// Full structural + consistency validation (constructor and deser authority).
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.measure != SpectralMeasure::FrequencySpecificIntensity {
            return Err(CoreError::InvalidFrequency {
                context: "canonical spectral grid must be frequency measure",
            });
        }
        if self.grid_id.is_empty() {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid id must be non-empty",
            });
        }
        if self.n_bins == 0 || self.n_bins > Self::MAX_BINS {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid bin count out of bounds",
            });
        }
        if !self.nu_min.is_finite()
            || !self.nu_max.is_finite()
            || !(self.nu_min > 0.0)
            || !(self.nu_max > self.nu_min)
        {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid requires 0 < nu_min < nu_max finite",
            });
        }
        let n = self.n_bins as usize;
        if self.centers.len() != n || self.weights.len() != n || self.edges.len() != n + 1 {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid edge/center/weight length inconsistent",
            });
        }
        if !spectral_f64_consistent(self.edges[0], self.nu_min)
            || !spectral_f64_consistent(self.edges[n], self.nu_max)
        {
            return Err(CoreError::InvalidFrequency {
                context: "spectral grid endpoints must match nu_min/nu_max",
            });
        }
        for i in 0..=n {
            let e = self.edges[i];
            if !e.is_finite() || !(e > 0.0) {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral grid edge non-finite or non-positive",
                });
            }
            if i > 0 && !(e > self.edges[i - 1]) {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral grid edges must be strictly monotonic increasing",
                });
            }
        }
        for i in 0..n {
            let lo = self.edges[i];
            let hi = self.edges[i + 1];
            let expected_w = hi - lo;
            let expected_c = (lo * hi).sqrt();
            let w = self.weights[i];
            let c = self.centers[i];
            if !w.is_finite() || !(w > 0.0) || !spectral_f64_consistent(w, expected_w) {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral grid weight must equal edge delta",
                });
            }
            if !c.is_finite() || !(c > 0.0) || !spectral_f64_consistent(c, expected_c) {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral grid center must equal geometric midpoint",
                });
            }
            if !(c > lo && c < hi) {
                return Err(CoreError::InvalidFrequency {
                    context: "spectral grid center outside its bin",
                });
            }
        }
        Ok(())
    }

    /// Rebuild centers/weights from edges (post-deser canonicalize).
    fn recompute_centers_weights_from_edges(&mut self) -> Result<(), CoreError> {
        let n = self.n_bins as usize;
        if self.edges.len() != n + 1 {
            return Err(CoreError::InvalidFrequency {
                context: "cannot recompute centers: edge length mismatch",
            });
        }
        // Pin endpoints to declared bounds (JSON may perturb trailing bits).
        self.edges[0] = self.nu_min;
        self.edges[n] = self.nu_max;
        let mut centers = Vec::with_capacity(n);
        let mut weights = Vec::with_capacity(n);
        for i in 0..n {
            let lo = self.edges[i];
            let hi = self.edges[i + 1];
            let c = (lo * hi).sqrt();
            let w = hi - lo;
            if !c.is_finite() || !(c > 0.0) || !w.is_finite() || !(w > 0.0) {
                return Err(CoreError::InvalidFrequency {
                    context: "recomputed center/weight invalid",
                });
            }
            centers.push(c);
            weights.push(w);
        }
        self.centers = centers;
        self.weights = weights;
        Ok(())
    }
}

/// Exact bits, or a few ULPs — enough for JSON float round-trip, not for corruption.
fn spectral_f64_consistent(stored: f64, expected: f64) -> bool {
    if stored.to_bits() == expected.to_bits() {
        return true;
    }
    let scale = expected.abs().max(1.0);
    (stored - expected).abs() <= 8.0 * f64::EPSILON * scale
}

impl Serialize for SpectralGrid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("SpectralGrid", 8)?;
        state.serialize_field("measure", &self.measure)?;
        state.serialize_field("grid_id", &self.grid_id)?;
        state.serialize_field("nu_min", &self.nu_min)?;
        state.serialize_field("nu_max", &self.nu_max)?;
        state.serialize_field("n_bins", &self.n_bins)?;
        state.serialize_field("edges", &self.edges)?;
        state.serialize_field("centers", &self.centers)?;
        state.serialize_field("weights", &self.weights)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SpectralGrid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Measure,
            GridId,
            NuMin,
            NuMax,
            NBins,
            Edges,
            Centers,
            Weights,
        }

        struct SpectralGridVisitor;

        impl<'de> Visitor<'de> for SpectralGridVisitor {
            type Value = SpectralGrid;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("validated SpectralGrid")
            }

            fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<SpectralGrid, V::Error> {
                let mut measure = None;
                let mut grid_id = None;
                let mut nu_min = None;
                let mut nu_max = None;
                let mut n_bins = None;
                let mut edges = None;
                let mut centers = None;
                let mut weights = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Measure => measure = Some(map.next_value()?),
                        Field::GridId => grid_id = Some(map.next_value()?),
                        Field::NuMin => nu_min = Some(map.next_value()?),
                        Field::NuMax => nu_max = Some(map.next_value()?),
                        Field::NBins => n_bins = Some(map.next_value()?),
                        Field::Edges => edges = Some(map.next_value()?),
                        Field::Centers => centers = Some(map.next_value()?),
                        Field::Weights => weights = Some(map.next_value()?),
                    }
                }
                let mut grid = SpectralGrid {
                    measure: measure.ok_or_else(|| de::Error::missing_field("measure"))?,
                    grid_id: grid_id.ok_or_else(|| de::Error::missing_field("grid_id"))?,
                    nu_min: nu_min.ok_or_else(|| de::Error::missing_field("nu_min"))?,
                    nu_max: nu_max.ok_or_else(|| de::Error::missing_field("nu_max"))?,
                    n_bins: n_bins.ok_or_else(|| de::Error::missing_field("n_bins"))?,
                    edges: edges.ok_or_else(|| de::Error::missing_field("edges"))?,
                    centers: centers.ok_or_else(|| de::Error::missing_field("centers"))?,
                    weights: weights.ok_or_else(|| de::Error::missing_field("weights"))?,
                };
                // Reject corrupt tables, then canonicalize centers/weights from edges.
                grid.validate().map_err(de::Error::custom)?;
                grid.recompute_centers_weights_from_edges()
                    .map_err(de::Error::custom)?;
                grid.validate().map_err(de::Error::custom)?;
                Ok(grid)
            }

            fn visit_seq<V: SeqAccess<'de>>(self, mut seq: V) -> Result<SpectralGrid, V::Error> {
                let measure = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let grid_id = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let nu_min = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let nu_max = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let n_bins = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let edges = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(5, &self))?;
                let centers = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(6, &self))?;
                let weights = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(7, &self))?;
                let mut grid = SpectralGrid {
                    measure,
                    grid_id,
                    nu_min,
                    nu_max,
                    n_bins,
                    edges,
                    centers,
                    weights,
                };
                grid.validate().map_err(de::Error::custom)?;
                grid.recompute_centers_weights_from_edges()
                    .map_err(de::Error::custom)?;
                grid.validate().map_err(de::Error::custom)?;
                Ok(grid)
            }
        }

        const FIELDS: &[&str] = &[
            "measure", "grid_id", "nu_min", "nu_max", "n_bins", "edges", "centers", "weights",
        ];
        deserializer.deserialize_struct("SpectralGrid", FIELDS, SpectralGridVisitor)
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
    fn frequency_deserialize_rejects_non_positive() {
        assert!(serde_json::from_str::<Frequency>("0.0").is_err());
        assert!(serde_json::from_str::<Frequency>("-1.0").is_err());
        assert!(serde_json::from_str::<Frequency>("1.5").is_ok());
        assert!(serde_json::from_str::<Wavelength>("0.0").is_err());
        assert!(serde_json::from_str::<Wavelength>("2.0").is_ok());
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
        assert!(SpectralGrid::log_spaced("x", 2.0, 1.0, 8).is_err());
    }

    #[test]
    fn spectral_grid_deserialize_rejects_tampered_weight() {
        let g = SpectralGrid::spectral_grid_v1().unwrap();
        let mut v = serde_json::to_value(&g).unwrap();
        v["weights"][0] = serde_json::json!(1.0e9);
        assert!(serde_json::from_value::<SpectralGrid>(v).is_err());
    }

    #[test]
    fn spectral_grid_deserialize_roundtrip() {
        let g = SpectralGrid::spectral_grid_v1().unwrap();
        let bytes = serde_json::to_vec(&g).unwrap();
        let back: SpectralGrid = serde_json::from_slice(&bytes).unwrap();
        // Edges may shift by JSON float noise; centers/weights are recomputed.
        assert_eq!(back.grid_id(), g.grid_id());
        assert_eq!(back.n_bins(), g.n_bins());
        back.validate().unwrap();
        for i in 0..g.n_bins() as usize {
            assert!(spectral_f64_consistent(back.edges()[i], g.edges()[i]));
        }
        assert!(spectral_f64_consistent(
            back.edges()[g.n_bins() as usize],
            g.nu_max()
        ));
    }

    #[test]
    fn spectral_grid_validate_rejects_non_monotonic_edges() {
        let mut g = SpectralGrid::log_spaced("t", 0.25, 4.0, 4).unwrap();
        g.edges[2] = g.edges[1];
        assert!(g.validate().is_err());
    }
}
