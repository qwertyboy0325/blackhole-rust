//! Circular equatorial geodesic four-velocity in Boyer–Lindquist (Gate 2B0).
//!
//! ```text
//! Ω_s = s √M / (r^(3/2) + s a √M)
//! u^μ_BL = u^t (1, 0, 0, Ω_s)
//! (u^t)^-2 = -(g_tt + 2 Ω_s g_tφ + Ω_s² g_φφ)
//! ```
//!
//! evaluated at `(t=0, r, θ=π/2, φ=0)`.
//!
//! Not a ZAMO. Radius is never clamped. Invalid orbits return typed errors.

use crate::bl_metric;
use crate::error::CoreError;
use crate::kerr::KerrParams;
use crate::types::{PositionBl, Vector};
use serde::{Deserialize, Serialize};

/// Coordinate sense of equatorial circular motion about `∂_φ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EquatorialAngularDirection {
    PositivePhi,
    NegativePhi,
}

impl EquatorialAngularDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PositivePhi => "positive-phi",
            Self::NegativePhi => "negative-phi",
        }
    }

    /// Stable project-owned digest tag (not Debug/Display/serde).
    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::PositivePhi => "equatorial-angular-direction:positive-phi",
            Self::NegativePhi => "equatorial-angular-direction:negative-phi",
        }
    }

    #[must_use]
    pub const fn sign(self) -> f64 {
        match self {
            Self::PositivePhi => 1.0,
            Self::NegativePhi => -1.0,
        }
    }
}

/// Circular equatorial geodesic kinematics at a BL radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CircularEquatorialOrbit {
    pub radius: f64,
    pub direction: EquatorialAngularDirection,
    pub angular_velocity_bl: f64,
    pub four_velocity_bl: Vector,
    pub normalization_residual: f64,
}

/// Prograde equatorial sense relative to black-hole spin.
///
/// ```text
/// a > 0  → PositivePhi
/// a < 0  → NegativePhi
/// a = 0  → PositivePhi (project convention)
/// ```
#[must_use]
pub fn prograde_equatorial_direction(params: &KerrParams) -> EquatorialAngularDirection {
    if params.spin() < 0.0 {
        EquatorialAngularDirection::NegativePhi
    } else {
        EquatorialAngularDirection::PositivePhi
    }
}

/// Circular equatorial geodesic four-velocity in BL coordinates.
pub fn circular_equatorial_geodesic_bl(
    params: &KerrParams,
    radius: f64,
    direction: EquatorialAngularDirection,
) -> Result<CircularEquatorialOrbit, CoreError> {
    if !radius.is_finite() {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "radius must be finite",
        });
    }
    let r_plus = params.outer_horizon_radius();
    if !(radius > r_plus) {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "radius must be strictly outside the outer horizon",
        });
    }

    let m = params.mass();
    let a = params.spin();
    let sqrt_m = m.sqrt();
    if !sqrt_m.is_finite() || !(sqrt_m > 0.0) {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "mass square-root unavailable",
        });
    }

    let s = direction.sign();
    let denom = radius.powf(1.5) + s * a * sqrt_m;
    if !denom.is_finite() || denom == 0.0 {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "angular-velocity denominator non-finite or zero",
        });
    }
    let omega = s * sqrt_m / denom;
    if !omega.is_finite() {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "angular velocity non-finite",
        });
    }

    let bl = PositionBl::new(0.0, radius, std::f64::consts::FRAC_PI_2, 0.0);
    let g = bl_metric(params, &bl).map_err(|_| CoreError::CircularOrbitUnavailable {
        context: "BL metric unavailable at equatorial evaluation event",
    })?;
    let g_tt = g.get(0, 0);
    let g_tphi = g.get(0, 3);
    let g_phiphi = g.get(3, 3);
    let norm_inv_sq = -(g_tt + 2.0 * omega * g_tphi + omega * omega * g_phiphi);
    if !norm_inv_sq.is_finite() || !(norm_inv_sq > 0.0) {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "no valid timelike circular orbit at this radius/direction",
        });
    }
    let u_t: f64 = 1.0 / norm_inv_sq.sqrt();
    if !u_t.is_finite() || !(u_t > 0.0) {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "u^t must be finite and strictly positive",
        });
    }
    let four_velocity_bl = Vector::new(u_t, 0.0, 0.0, u_t * omega);
    if !four_velocity_bl.is_finite() {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "four-velocity non-finite",
        });
    }
    let normalization_residual = g.contract(&four_velocity_bl, &four_velocity_bl) + 1.0;
    if !normalization_residual.is_finite() {
        return Err(CoreError::CircularOrbitUnavailable {
            context: "normalization residual non-finite",
        });
    }

    Ok(CircularEquatorialOrbit {
        radius,
        direction,
        angular_velocity_bl: omega,
        four_velocity_bl,
        normalization_residual,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn schwarzschild_omega_magnitude() {
        let params = KerrParams::new(1.0, 0.0).unwrap();
        let r = 10.0;
        let pos =
            circular_equatorial_geodesic_bl(&params, r, EquatorialAngularDirection::PositivePhi)
                .unwrap();
        let neg =
            circular_equatorial_geodesic_bl(&params, r, EquatorialAngularDirection::NegativePhi)
                .unwrap();
        let expected = (1.0 / r.powi(3)).sqrt();
        assert_relative_eq!(pos.angular_velocity_bl, expected, epsilon = 1e-14);
        assert_relative_eq!(neg.angular_velocity_bl, -expected, epsilon = 1e-14);
        assert_relative_eq!(
            pos.four_velocity_bl.t,
            neg.four_velocity_bl.t,
            epsilon = 1e-14
        );
        assert!(pos.four_velocity_bl.t > 0.0);
        assert!(neg.four_velocity_bl.t > 0.0);
    }

    #[test]
    fn prograde_follows_spin_sign() {
        let plus = KerrParams::new(1.0, 0.5).unwrap();
        let zero = KerrParams::new(1.0, 0.0).unwrap();
        let minus = KerrParams::new(1.0, -0.5).unwrap();
        assert_eq!(
            prograde_equatorial_direction(&plus),
            EquatorialAngularDirection::PositivePhi
        );
        assert_eq!(
            prograde_equatorial_direction(&zero),
            EquatorialAngularDirection::PositivePhi
        );
        assert_eq!(
            prograde_equatorial_direction(&minus),
            EquatorialAngularDirection::NegativePhi
        );
    }

    #[test]
    fn future_directed_and_normalized_corpus() {
        let spins = [0.0, 0.5, 0.999, -0.5];
        let radii = [3.0, 6.0, 10.0, 20.0];
        for &a in &spins {
            let params = KerrParams::new(1.0, a).unwrap();
            let dir = prograde_equatorial_direction(&params);
            for &r in &radii {
                match circular_equatorial_geodesic_bl(&params, r, dir) {
                    Ok(orbit) => {
                        assert!(orbit.four_velocity_bl.t > 0.0);
                        assert!(orbit.normalization_residual.abs() < 1e-12);
                        let g = bl_metric(
                            &params,
                            &PositionBl::new(0.0, r, std::f64::consts::FRAC_PI_2, 0.0),
                        )
                        .unwrap();
                        let uu = g.contract(&orbit.four_velocity_bl, &orbit.four_velocity_bl);
                        assert_relative_eq!(uu, -1.0, epsilon = 1e-12);
                    }
                    Err(e) => {
                        // Photon-sphere / no-timelike region: typed skip only.
                        assert!(
                            matches!(e, CoreError::CircularOrbitUnavailable { .. }),
                            "a={a} r={r}: unexpected {e}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_inside_horizon_without_clamp() {
        let params = KerrParams::new(1.0, 0.5).unwrap();
        let r_plus = params.outer_horizon_radius();
        let err = circular_equatorial_geodesic_bl(
            &params,
            0.5 * r_plus,
            EquatorialAngularDirection::PositivePhi,
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::CircularOrbitUnavailable { .. }));
    }

    #[test]
    fn digest_tags_stable() {
        assert_eq!(
            EquatorialAngularDirection::PositivePhi.digest_tag(),
            "equatorial-angular-direction:positive-phi"
        );
        assert_eq!(
            EquatorialAngularDirection::NegativePhi.digest_tag(),
            "equatorial-angular-direction:negative-phi"
        );
    }
}
