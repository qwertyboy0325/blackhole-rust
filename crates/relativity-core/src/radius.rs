//! Oblate-spheroidal radius for Cartesian Kerr–Schild coordinates.
//!
//! Direct evaluation of `r² = ½(A + √(A² + 4 a² z²))` with `A = ρ² - a²` suffers
//! catastrophic cancellation when `A < 0` and `|z|` is small: `A + √(A²+ε) ≈ 0`
//! while each term is `O(|A|)`.
//!
//! Algebraically equivalent stable branch (`A < 0`):
//!
//! ```text
//! r² = 2 a² z² / (√(A² + 4 a² z²) - A)
//!    = 2 a² z² / (√(A² + 4 a² z²) + |A|)
//! ```
//!
//! Switching condition: use the direct form iff `A >= 0`; otherwise the
//! rationalized form. Derivation: multiply numerator and denominator of
//! `(A+D)/2` by `(D-A)` and use `D² - A² = 4 a² z²`.
//!
//! Sources: [physics-assumptions.md], [GRay2], Kerr–Schild chart definitions.

use crate::error::{CoreError, DomainReason};
use crate::kerr::KerrParams;
use crate::types::PositionKs;

/// Successfully evaluated oblate radius and auxiliaries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OblateRadius {
    pub r: f64,
    pub r2: f64,
    pub a: f64,
    /// `A = ρ² - a²`
    pub a_aux: f64,
    pub rho2: f64,
    /// Branch used: `true` if `A >= 0` direct form.
    pub used_direct_branch: bool,
}

impl OblateRadius {
    #[must_use]
    pub fn sigma_like(&self, z: f64) -> f64 {
        // r⁴ + a² z² appearing in H denominator.
        self.r2 * self.r2 + self.a * self.a * z * z
    }
}

/// Evaluate nonnegative oblate-spheroidal radius at a spatial KS point.
///
/// Returns a typed domain error on the ring singularity / excluded disk
/// (`r = 0`) and on non-finite or unresolved evaluation. Never maps failure to
/// a silent `r = 0` success.
pub fn evaluate_oblate_radius(
    params: &KerrParams,
    pos: &PositionKs,
) -> Result<OblateRadius, CoreError> {
    pos.require_finite("oblate radius position")?;
    let a = params.spin();
    let x = pos.x;
    let y = pos.y;
    let z = pos.z;
    let rho2 = x * x + y * y + z * z;
    let a2 = a * a;
    let a_aux = rho2 - a2;
    let disc_inner = a_aux * a_aux + 4.0 * a2 * z * z;
    if !disc_inner.is_finite() || disc_inner < 0.0 {
        return Err(CoreError::ChartDomain {
            x,
            y,
            z,
            reason: DomainReason::RadiusUnresolved,
        });
    }
    let d = disc_inner.sqrt();
    if !d.is_finite() {
        return Err(CoreError::ChartDomain {
            x,
            y,
            z,
            reason: DomainReason::RadiusUnresolved,
        });
    }

    let (r2, used_direct_branch) = if a_aux >= 0.0 {
        (0.5 * (a_aux + d), true)
    } else {
        // Stable rationalization. Denominator = D - A = D + |A| > 0 when A < 0.
        let denom = d - a_aux;
        if !(denom.is_finite() && denom > 0.0) {
            return Err(CoreError::ChartDomain {
                x,
                y,
                z,
                reason: DomainReason::RadiusUnresolved,
            });
        }
        (2.0 * a2 * z * z / denom, false)
    };

    if !r2.is_finite() {
        return Err(CoreError::ChartDomain {
            x,
            y,
            z,
            reason: DomainReason::RadiusUnresolved,
        });
    }
    if r2 < 0.0 {
        return Err(CoreError::ChartDomain {
            x,
            y,
            z,
            reason: DomainReason::RadiusUnresolved,
        });
    }
    if r2 == 0.0 {
        // Includes ring singularity (ρ² = a², z = 0) and the excluded equatorial
        // disk interior in the KS chart where the BL polar angle jumps.
        return Err(CoreError::ChartDomain {
            x,
            y,
            z,
            reason: DomainReason::RingSingularityOrExcludedDisk,
        });
    }

    let r = r2.sqrt();
    if !r.is_finite() || r <= 0.0 {
        return Err(CoreError::Unresolved {
            context: "oblate radius sqrt",
        });
    }

    Ok(OblateRadius {
        r,
        r2,
        a,
        a_aux,
        rho2,
        used_direct_branch,
    })
}

/// Relative difference between direct and rationalized formulas (diagnostic).
#[must_use]
pub fn branch_agreement_diagnostic(params: &KerrParams, pos: &PositionKs) -> Option<f64> {
    let a = params.spin();
    let rho2 = pos.x * pos.x + pos.y * pos.y + pos.z * pos.z;
    let a_aux = rho2 - a * a;
    let d = (a_aux * a_aux + 4.0 * a * a * pos.z * pos.z).sqrt();
    let direct = 0.5 * (a_aux + d);
    let rationalized = {
        let denom = d - a_aux;
        if denom == 0.0 {
            return None;
        }
        2.0 * a * a * pos.z * pos.z / denom
    };
    let scale = direct.abs().max(rationalized.abs()).max(1e-300);
    Some((direct - rationalized).abs() / scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schwarzschild_reduces_to_euclidean_radius() {
        let p = KerrParams::new(1.0, 0.0).unwrap();
        let pos = PositionKs::spatial(3.0, 4.0, 12.0);
        let o = evaluate_oblate_radius(&p, &pos).unwrap();
        assert!((o.r - 13.0).abs() < 1e-14);
        assert!(o.used_direct_branch);
    }

    #[test]
    fn cancellation_prone_region_uses_stable_branch() {
        let p = KerrParams::new(1.0, 0.9).unwrap();
        // A = x²+y²+z² - a² < 0 with tiny z.
        let pos = PositionKs::spatial(0.1, 0.0, 1e-16);
        let o = evaluate_oblate_radius(&p, &pos).unwrap();
        assert!(!o.used_direct_branch);
        assert!(o.r.is_finite() && o.r > 0.0);
        // Direct formula would under/overflow to ~0 incorrectly for tinier z;
        // ensure r is consistent with the implicit definition.
        let lhs =
            (pos.x * pos.x + pos.y * pos.y) / (o.r2 + p.spin() * p.spin()) + (pos.z * pos.z) / o.r2;
        assert!((lhs - 1.0).abs() < 1e-10, "implicit residual {lhs}");
    }

    #[test]
    fn rejects_ring_and_excluded_disk() {
        let p = KerrParams::new(1.0, 0.9).unwrap();
        let ring = PositionKs::spatial(0.9, 0.0, 0.0);
        assert!(matches!(
            evaluate_oblate_radius(&p, &ring),
            Err(CoreError::ChartDomain {
                reason: DomainReason::RingSingularityOrExcludedDisk,
                ..
            })
        ));
        let disk = PositionKs::spatial(0.1, 0.0, 0.0);
        assert!(matches!(
            evaluate_oblate_radius(&p, &disk),
            Err(CoreError::ChartDomain {
                reason: DomainReason::RingSingularityOrExcludedDisk,
                ..
            })
        ));
    }

    #[test]
    fn rejects_non_finite() {
        let p = KerrParams::new(1.0, 0.5).unwrap();
        let pos = PositionKs::spatial(f64::NAN, 0.0, 1.0);
        assert!(matches!(
            evaluate_oblate_radius(&p, &pos),
            Err(CoreError::NonFinite { .. })
        ));
    }

    #[test]
    fn stable_beats_naive_in_deep_cancellation() {
        let p = KerrParams::new(1.0, 0.999).unwrap();
        // z small enough that 4 a² z² underflows relative to ulp(A²), so the
        // direct A+D sum collapses to 0 in f64, but z² itself remains nonzero.
        let z = 1e-9;
        let pos = PositionKs::spatial(0.05, 0.0, z);
        let o = evaluate_oblate_radius(&p, &pos).unwrap();
        assert!(o.r2.is_finite() && o.r2 > 0.0);
        let a = p.spin();
        let a_aux = pos.x * pos.x + pos.y * pos.y + z * z - a * a;
        let d = (a_aux * a_aux + 4.0 * a * a * z * z).sqrt();
        let naive = 0.5 * (a_aux + d);
        assert_eq!(naive, 0.0, "fixture must be in naive-collapse regime");
        assert!(o.r2 > 0.0 && o.r2.is_finite());
        // Record: stable r² ≈ a² z² / |A| order; must match implicit definition.
        let lhs = (pos.x * pos.x + pos.y * pos.y) / (o.r2 + a * a) + (z * z) / o.r2;
        assert!((lhs - 1.0).abs() < 1e-6, "implicit residual {lhs}");
    }
}
