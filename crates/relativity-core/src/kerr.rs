//! Checked Kerr parameters in geometrized units.

use crate::error::CoreError;

/// Kerr mass and spin with validated `|a| <= M`, `M > 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KerrParams {
    mass: f64,
    spin: f64,
}

impl KerrParams {
    /// Construct checked parameters. Extremal `|a| = M` is allowed explicitly.
    ///
    /// No silent clamping.
    pub fn new(mass: f64, spin: f64) -> Result<Self, CoreError> {
        if !mass.is_finite() || mass <= 0.0 {
            return Err(CoreError::InvalidMass { mass });
        }
        if !spin.is_finite() || spin.abs() > mass {
            return Err(CoreError::InvalidSpin { mass, spin });
        }
        Ok(Self { mass, spin })
    }

    #[must_use]
    pub fn mass(&self) -> f64 {
        self.mass
    }

    #[must_use]
    pub fn spin(&self) -> f64 {
        self.spin
    }

    #[must_use]
    pub fn spin_over_mass(&self) -> f64 {
        self.spin / self.mass
    }

    #[must_use]
    pub fn is_extremal(&self) -> bool {
        (self.spin.abs() - self.mass).abs() <= f64::EPSILON * self.mass.max(1.0)
    }

    /// Outer horizon radius `r_+ = M + sqrt(M^2 - a^2)` in Boyer–Lindquist `r`.
    #[must_use]
    pub fn outer_horizon_radius(&self) -> f64 {
        let m = self.mass;
        let a = self.spin;
        let disc = (m * m - a * a).max(0.0);
        m + disc.sqrt()
    }

    /// Inner horizon radius `r_-`.
    #[must_use]
    pub fn inner_horizon_radius(&self) -> f64 {
        let m = self.mass;
        let a = self.spin;
        let disc = (m * m - a * a).max(0.0);
        m - disc.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_subextremal_and_extremal() {
        let k = KerrParams::new(1.0, 0.999).unwrap();
        assert!((k.spin_over_mass() - 0.999).abs() < 1e-15);
        let e = KerrParams::new(1.0, 1.0).unwrap();
        assert!(e.is_extremal());
        assert!((e.outer_horizon_radius() - 1.0).abs() < 1e-14);
    }

    #[test]
    fn rejects_invalid_mass_and_spin() {
        assert!(matches!(
            KerrParams::new(0.0, 0.0),
            Err(CoreError::InvalidMass { .. })
        ));
        assert!(matches!(
            KerrParams::new(-1.0, 0.0),
            Err(CoreError::InvalidMass { .. })
        ));
        assert!(matches!(
            KerrParams::new(f64::NAN, 0.0),
            Err(CoreError::InvalidMass { .. })
        ));
        assert!(matches!(
            KerrParams::new(1.0, 1.0000001),
            Err(CoreError::InvalidSpin { .. })
        ));
        assert!(matches!(
            KerrParams::new(1.0, f64::INFINITY),
            Err(CoreError::InvalidSpin { .. })
        ));
    }

    #[test]
    fn no_silent_clamp() {
        // |a| > M must error, not clamp to extremal.
        let err = KerrParams::new(1.0, 1.5).unwrap_err();
        assert!(matches!(err, CoreError::InvalidSpin { spin: 1.5, .. }));
    }
}
