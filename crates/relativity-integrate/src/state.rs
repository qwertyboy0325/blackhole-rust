//! Production geodesic state and affine parameter.

use relativity_core::{Covector, PositionKs};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{IntegrationError, IntegrationStage};

/// Affine parameter λ along the geodesic.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AffineParameter(pub f64);

impl AffineParameter {
    pub fn require_finite(self) -> Result<Self, IntegrationError> {
        if self.0.is_finite() {
            Ok(self)
        } else {
            Err(IntegrationError::NonFiniteState {
                stage: IntegrationStage::InitialState,
            })
        }
    }
}

/// Eight-dimensional Kerr Hamiltonian state in Cartesian Kerr–Schild.
///
/// Vector ordering (fixed, tested): `[t, x, y, z, p_t, p_x, p_y, p_z]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeodesicState {
    pub position: PositionKs,
    pub momentum: Covector,
}

impl Serialize for GeodesicState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_array().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GeodesicState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let a = <[f64; 8]>::deserialize(deserializer)?;
        Self::from_array(&a).map_err(serde::de::Error::custom)
    }
}

impl GeodesicState {
    pub fn new(position: PositionKs, momentum: Covector) -> Result<Self, IntegrationError> {
        let s = Self { position, momentum };
        s.require_finite(IntegrationStage::InitialState)?;
        Ok(s)
    }

    pub fn require_finite(&self, stage: IntegrationStage) -> Result<(), IntegrationError> {
        if !self.position.t.is_finite()
            || !self.position.x.is_finite()
            || !self.position.y.is_finite()
            || !self.position.z.is_finite()
            || !self.momentum.is_finite()
        {
            return Err(IntegrationError::NonFiniteState { stage });
        }
        Ok(())
    }

    /// Pack to solver vector `[t,x,y,z,p_t,p_x,p_y,p_z]`.
    pub fn to_array(&self) -> [f64; 8] {
        [
            self.position.t,
            self.position.x,
            self.position.y,
            self.position.z,
            self.momentum.t,
            self.momentum.x,
            self.momentum.y,
            self.momentum.z,
        ]
    }

    /// Unpack from solver vector; rejects non-finite components.
    pub fn from_array(y: &[f64]) -> Result<Self, IntegrationError> {
        if y.len() != 8 {
            return Err(IntegrationError::NonFiniteState {
                stage: IntegrationStage::Outcome,
            });
        }
        if y.iter().any(|v| !v.is_finite()) {
            return Err(IntegrationError::NonFiniteState {
                stage: IntegrationStage::Outcome,
            });
        }
        Ok(Self {
            position: PositionKs::new(y[0], y[1], y[2], y[3]),
            momentum: Covector::from_components([y[4], y[5], y[6], y[7]]),
        })
    }

    pub fn bits_hex(&self) -> String {
        self.to_array()
            .iter()
            .map(|v| format!("{:016x}", v.to_bits()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_vector_round_trip() {
        let s = GeodesicState::new(
            PositionKs::new(1.0, 2.0, 3.0, 4.0),
            Covector::new(-1.0, 0.1, 0.2, 0.3),
        )
        .unwrap();
        let a = s.to_array();
        assert_eq!(a, [1.0, 2.0, 3.0, 4.0, -1.0, 0.1, 0.2, 0.3]);
        let back = GeodesicState::from_array(&a).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn rejects_non_finite() {
        let mut y = [1.0; 8];
        y[3] = f64::NAN;
        assert!(GeodesicState::from_array(&y).is_err());
    }
}
