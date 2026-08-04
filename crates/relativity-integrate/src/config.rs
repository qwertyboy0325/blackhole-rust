//! Checked DOP853 configuration with per-component vector tolerances.

use crate::error::IntegrationError;

/// Controls when event candidates become armed (no geometry mutation).
///
/// Does not move surfaces or alter ray state — only whether a localized
/// candidate at affine λ may terminate the ray.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EventArmingPolicy {
    /// Finite, non-negative. Candidates with `lambda < minimum_affine_parameter`
    /// are ignored (remain disarmed).
    pub minimum_affine_parameter: f64,
}

impl EventArmingPolicy {
    pub fn immediate() -> Self {
        Self {
            minimum_affine_parameter: 0.0,
        }
    }

    pub fn after(minimum_affine_parameter: f64) -> Result<Self, IntegrationError> {
        let p = Self {
            minimum_affine_parameter,
        };
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), IntegrationError> {
        if !self.minimum_affine_parameter.is_finite() || self.minimum_affine_parameter < 0.0 {
            return Err(IntegrationError::InvalidConfig {
                field: "event_arming.minimum_affine_parameter",
            });
        }
        Ok(())
    }

    #[inline]
    pub fn is_armed(&self, lambda: f64) -> bool {
        lambda >= self.minimum_affine_parameter
    }
}

/// Opt-in policy for OuterHorizon proximity termination only.
///
/// Distinct from `event_value_tolerance` (root-localization convergence).
/// Does **not** apply to EscapeSphere or arbitrary EventSurface values.
///
/// A SurfaceApproach under this policy is **not** proof of horizon crossing;
/// the f64 Cartesian-KS adaptive stall near `r → r₊⁺` remains an open numerical
/// investigation item.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HorizonProximityPolicy {
    pub enabled: bool,
    /// Finite positive tolerance on `|f| = |r_oblate - r₊|` for approach capture.
    pub approach_tolerance: f64,
}

impl HorizonProximityPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            approach_tolerance: 1e-10,
        }
    }

    pub fn enabled(approach_tolerance: f64) -> Result<Self, IntegrationError> {
        let p = Self {
            enabled: true,
            approach_tolerance,
        };
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), IntegrationError> {
        if !self.approach_tolerance.is_finite() || self.approach_tolerance <= 0.0 {
            return Err(IntegrationError::InvalidConfig {
                field: "horizon_proximity.approach_tolerance",
            });
        }
        Ok(())
    }
}

/// Production DOP853 settings. All tolerances are finite and positive.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Dop853Config {
    pub relative_tolerance: [f64; 8],
    pub absolute_tolerance: [f64; 8],
    pub max_step: f64,
    pub affine_limit: f64,
    pub event_time_tolerance: f64,
    pub event_value_tolerance: f64,
    pub max_accepted_steps: u64,
    /// Opt-in OuterHorizon-only proximity policy (default: disabled).
    pub horizon_proximity: HorizonProximityPolicy,
    /// Event arming threshold (default: armed from λ = 0).
    pub event_arming: EventArmingPolicy,
}

impl Dop853Config {
    /// Conservative diagnostic defaults — not cinematic presets.
    pub fn diagnostic_default() -> Self {
        Self {
            relative_tolerance: [1e-10; 8],
            absolute_tolerance: [
                1e-12, 1e-12, 1e-12, 1e-12, // position
                1e-14, 1e-14, 1e-14, 1e-14, // momentum (tighter)
            ],
            max_step: 1.0,
            affine_limit: 1.0e3,
            event_time_tolerance: 1e-12,
            event_value_tolerance: 1e-12,
            max_accepted_steps: 100_000,
            horizon_proximity: HorizonProximityPolicy::disabled(),
            event_arming: EventArmingPolicy::immediate(),
        }
    }

    pub fn validate(&self) -> Result<(), IntegrationError> {
        for (i, &v) in self.relative_tolerance.iter().enumerate() {
            if !v.is_finite() || v <= 0.0 {
                return Err(IntegrationError::InvalidConfig {
                    field: "relative_tolerance",
                });
            }
            let _ = i;
        }
        for &v in &self.absolute_tolerance {
            if !v.is_finite() || v <= 0.0 {
                return Err(IntegrationError::InvalidConfig {
                    field: "absolute_tolerance",
                });
            }
        }
        if !self.max_step.is_finite() || self.max_step <= 0.0 {
            return Err(IntegrationError::InvalidConfig { field: "max_step" });
        }
        if !self.affine_limit.is_finite() || self.affine_limit <= 0.0 {
            return Err(IntegrationError::InvalidConfig {
                field: "affine_limit",
            });
        }
        if !self.event_time_tolerance.is_finite() || self.event_time_tolerance <= 0.0 {
            return Err(IntegrationError::InvalidConfig {
                field: "event_time_tolerance",
            });
        }
        if !self.event_value_tolerance.is_finite() || self.event_value_tolerance <= 0.0 {
            return Err(IntegrationError::InvalidConfig {
                field: "event_value_tolerance",
            });
        }
        if self.max_accepted_steps == 0 {
            return Err(IntegrationError::InvalidConfig {
                field: "max_accepted_steps",
            });
        }
        self.horizon_proximity.validate()?;
        self.event_arming.validate()?;
        Ok(())
    }

    pub fn with_tighter_tol(mut self, factor: f64) -> Self {
        for v in &mut self.relative_tolerance {
            *v *= factor;
        }
        for v in &mut self.absolute_tolerance {
            *v *= factor;
        }
        self
    }

    pub fn with_horizon_proximity(mut self, policy: HorizonProximityPolicy) -> Self {
        self.horizon_proximity = policy;
        self
    }

    pub fn with_event_arming(mut self, policy: EventArmingPolicy) -> Self {
        self.event_arming = policy;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_positive_tolerance() {
        let mut c = Dop853Config::diagnostic_default();
        c.absolute_tolerance[0] = 0.0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn rejects_bad_affine_limit() {
        let mut c = Dop853Config::diagnostic_default();
        c.affine_limit = -1.0;
        assert!(c.validate().is_err());
    }

    #[test]
    fn rejects_bad_horizon_approach_tol() {
        let mut c = Dop853Config::diagnostic_default();
        c.horizon_proximity.enabled = true;
        c.horizon_proximity.approach_tolerance = 0.0;
        assert!(c.validate().is_err());
    }
}
