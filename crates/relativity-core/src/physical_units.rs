//! SI newtypes for Gate 2C0 physical radiometry.
//!
//! Distinct from diagnostic dimensionless spectral frequencies. Never treat
//! `spectral-grid-v1` ν as Hz.

use crate::error::CoreError;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

fn require_finite_positive(value: f64, context: &'static str) -> Result<f64, CoreError> {
    if !value.is_finite() {
        return Err(CoreError::InvalidPhysicalQuantity { context });
    }
    if !(value > 0.0) {
        return Err(CoreError::InvalidPhysicalQuantity { context });
    }
    Ok(value)
}

fn require_finite_nonnegative(value: f64, context: &'static str) -> Result<f64, CoreError> {
    if !value.is_finite() {
        return Err(CoreError::InvalidPhysicalQuantity { context });
    }
    if value < 0.0 {
        return Err(CoreError::InvalidPhysicalQuantity { context });
    }
    Ok(value)
}

/// Physical frequency in hertz.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PhysicalFrequencyHz(f64);

impl PhysicalFrequencyHz {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        Ok(Self(require_finite_positive(
            value,
            "physical frequency Hz must be finite and > 0",
        )?))
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

impl<'de> Deserialize<'de> for PhysicalFrequencyHz {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        PhysicalFrequencyHz::new(v).map_err(de::Error::custom)
    }
}

/// Absolute temperature in kelvin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct TemperatureKelvin(f64);

impl TemperatureKelvin {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        Ok(Self(require_finite_nonnegative(
            value,
            "temperature K must be finite and >= 0",
        )?))
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

impl<'de> Deserialize<'de> for TemperatureKelvin {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        TemperatureKelvin::new(v).map_err(de::Error::custom)
    }
}

/// Mass in kilograms.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct MassKg(f64);

impl MassKg {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        Ok(Self(require_finite_positive(
            value,
            "mass kg must be finite and > 0",
        )?))
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

impl<'de> Deserialize<'de> for MassKg {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        MassKg::new(v).map_err(de::Error::custom)
    }
}

/// Rest-mass accretion rate [kg/s] (authoritative Gate 2C0 knob).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct MdotKgPerS(f64);

impl MdotKgPerS {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        Ok(Self(require_finite_nonnegative(
            value,
            "mdot kg/s must be finite and >= 0",
        )?))
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

impl<'de> Deserialize<'de> for MdotKgPerS {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        MdotKgPerS::new(v).map_err(de::Error::custom)
    }
}

/// One-face radiant flux [W m⁻²] (energy per proper time per proper area).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FluxWPerM2(f64);

impl FluxWPerM2 {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        Ok(Self(require_finite_nonnegative(
            value,
            "flux W/m^2 must be finite and >= 0",
        )?))
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

impl<'de> Deserialize<'de> for FluxWPerM2 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        FluxWPerM2::new(v).map_err(de::Error::custom)
    }
}

/// Specific intensity `I_ν` [W m⁻² Hz⁻¹ sr⁻¹].
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SpecificIntensityNu(f64);

impl SpecificIntensityNu {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        Ok(Self(require_finite_nonnegative(
            value,
            "I_nu must be finite and >= 0",
        )?))
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

impl<'de> Deserialize<'de> for SpecificIntensityNu {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = f64::deserialize(deserializer)?;
        SpecificIntensityNu::new(v).map_err(de::Error::custom)
    }
}

/// Physical scale: mass from solar multiples via pinned constants.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicalScale {
    pub solar_masses: f64,
    pub mass_kg: MassKg,
    pub gravitational_radius_m: f64,
    pub constants_revision: &'static str,
}

impl PhysicalScale {
    pub fn from_solar_masses(solar_masses: f64) -> Result<Self, CoreError> {
        if !solar_masses.is_finite() || !(solar_masses > 0.0) {
            return Err(CoreError::InvalidPhysicalQuantity {
                context: "solar_masses must be finite and > 0",
            });
        }
        let mass_kg = MassKg::new(crate::physical_constants::mass_kg_from_solar_masses(
            solar_masses,
        ))?;
        let rg = crate::physical_constants::gravitational_radius_m(mass_kg.value());
        if !rg.is_finite() || !(rg > 0.0) {
            return Err(CoreError::InvalidPhysicalQuantity {
                context: "gravitational radius non-finite",
            });
        }
        Ok(Self {
            solar_masses,
            mass_kg,
            gravitational_radius_m: rg,
            constants_revision: crate::physical_constants::CONSTANTS_REVISION,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonpositive_frequency() {
        assert!(PhysicalFrequencyHz::new(0.0).is_err());
        assert!(PhysicalFrequencyHz::new(-1.0).is_err());
    }

    #[test]
    fn accepts_zero_temperature_and_mdot() {
        assert!(TemperatureKelvin::new(0.0).is_ok());
        assert!(MdotKgPerS::new(0.0).is_ok());
    }
}
