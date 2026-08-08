//! Prograde Kerr ISCO radius (Bardeen–Press–Teukolsky).
//!
//! Gate 2C0 uses prograde ISCO as the Page–Thorne zero-torque boundary.
//! Retrograde is a typed reject at the flux-model boundary.

use crate::error::CoreError;
use crate::kerr::KerrParams;

/// Prograde ISCO radius in geometrized units (`G=c=1`), same mass unit as `params.mass()`.
///
/// ```text
/// Z₁ = 1 + (1−a*²)^{1/3} [(1+a*)^{1/3} + (1−a*)^{1/3}]
/// Z₂ = (3 a*² + Z₁²)^{1/2}
/// r_isco/M = 3 + Z₂ − [(3−Z₁)(3+Z₁+2 Z₂)]^{1/2}   (prograde)
/// ```
///
/// Source: Bardeen, Press & Teukolsky (1972). V1 rejects `a* < 0`.
pub fn prograde_isco_radius(params: &KerrParams) -> Result<f64, CoreError> {
    let a_star = params.spin_over_mass();
    if !(a_star >= 0.0) {
        return Err(CoreError::InvalidPhysicalQuantity {
            context: "Gate 2C0 Page-Thorne V1 rejects retrograde spin (a*/M < 0)",
        });
    }
    if !a_star.is_finite() || a_star > 1.0 {
        return Err(CoreError::InvalidSpin {
            mass: params.mass(),
            spin: params.spin(),
        });
    }
    let m = params.mass();
    let z1 = 1.0
        + (1.0 - a_star * a_star).powf(1.0 / 3.0)
            * ((1.0 + a_star).powf(1.0 / 3.0) + (1.0 - a_star).powf(1.0 / 3.0));
    let z2 = (3.0 * a_star * a_star + z1 * z1).sqrt();
    let inner = (3.0 - z1) * (3.0 + z1 + 2.0 * z2);
    if !(inner >= 0.0) || !inner.is_finite() || !z1.is_finite() || !z2.is_finite() {
        return Err(CoreError::Unresolved {
            context: "prograde ISCO discriminant unresolved",
        });
    }
    let r_over_m = 3.0 + z2 - inner.sqrt();
    let r = r_over_m * m;
    if !r.is_finite() || !(r > params.outer_horizon_radius()) {
        return Err(CoreError::Unresolved {
            context: "prograde ISCO radius non-finite or not outside horizon",
        });
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schwarzschild_isco_is_six_m() {
        let k = KerrParams::new(1.0, 0.0).unwrap();
        let r = prograde_isco_radius(&k).unwrap();
        assert!((r - 6.0).abs() < 1e-12);
    }

    #[test]
    fn high_spin_isco_approaches_horizon() {
        let k = KerrParams::new(1.0, 0.999).unwrap();
        let r = prograde_isco_radius(&k).unwrap();
        let r_plus = k.outer_horizon_radius();
        assert!(r > r_plus);
        assert!(r < 1.5);
    }

    #[test]
    fn retrograde_rejected() {
        let k = KerrParams::new(1.0, -0.5).unwrap();
        assert!(prograde_isco_radius(&k).is_err());
    }
}
