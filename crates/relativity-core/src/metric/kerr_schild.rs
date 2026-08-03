//! Covariant and contravariant Cartesian Kerr–Schild metric.
//!
//! Sign convention / form (geometrized units, signature `(-,+,+,+)`):
//!
//! ```text
//! g_μν = η_μν + 2 H ℓ_μ ℓ_ν
//! g^μν = η^μν − 2 H ℓ^μ ℓ^ν
//! H    = M r³ / (r⁴ + a² z²)
//! ℓ_μ  = (1, (r x + a y)/(r²+a²), (r y − a x)/(r²+a²), z/r)
//! ℓ^μ  = η^{μν} ℓ_ν  ⇒  ℓ^t = −1, ℓ^i = ℓ_i
//! ```
//!
//! The inverse is the Kerr–Schild identity for this null deformation, not a
//! numerical inverse of `g_μν`. Tests compare against an independent matrix
//! inverse of the covariant metric.
//!
//! Sources: Kerr (1963) / Kerr–Schild form; GRay2 (Chan et al.);
//! `docs/physics-assumptions.md`.

use crate::error::{CoreError, DomainReason};
use crate::kerr::KerrParams;
use crate::radius::{evaluate_oblate_radius, OblateRadius};
use crate::types::{MetricTensor, PositionKs, Vector};

/// Scalar, null covector/vector, and metric tensors at a KS event.
#[derive(Debug, Clone, Copy)]
pub struct KerrSchildQuantities {
    pub radius: OblateRadius,
    pub h: f64,
    /// Covariant null vector `ℓ_μ`.
    pub ell_cov: [f64; 4],
    /// Contravariant null vector `ℓ^μ`.
    pub ell_con: [f64; 4],
    pub metric: MetricTensor,
    pub inverse_metric: MetricTensor,
}

/// Evaluate Cartesian Kerr–Schild geometry at `pos`.
pub fn evaluate_kerr_schild(
    params: &KerrParams,
    pos: &PositionKs,
) -> Result<KerrSchildQuantities, CoreError> {
    pos.require_finite("kerr-schild position")?;
    let radius = evaluate_oblate_radius(params, pos)?;
    let r = radius.r;
    let r2 = radius.r2;
    let a = params.spin();
    let m = params.mass();
    let x = pos.x;
    let y = pos.y;
    let z = pos.z;

    let denom_h = r2 * r2 + a * a * z * z;
    if !(denom_h.is_finite() && denom_h > 0.0) {
        return Err(CoreError::ChartDomain {
            x,
            y,
            z,
            reason: DomainReason::MetricDenominatorUnresolved,
        });
    }
    let h = m * r2 * r / denom_h;
    if !h.is_finite() {
        return Err(CoreError::Unresolved {
            context: "Kerr-Schild H scalar",
        });
    }

    let r2_a2 = r2 + a * a;
    if !(r2_a2.is_finite() && r2_a2 > 0.0) {
        return Err(CoreError::Unresolved {
            context: "r^2 + a^2",
        });
    }

    let ell_x = (r * x + a * y) / r2_a2;
    let ell_y = (r * y - a * x) / r2_a2;
    let ell_z = z / r;
    if ![ell_x, ell_y, ell_z].iter().all(|v| v.is_finite()) {
        return Err(CoreError::Unresolved {
            context: "Kerr-Schild null vector",
        });
    }

    // ℓ_μ = (1, ℓ_x, ℓ_y, ℓ_z)
    let ell_cov = [1.0, ell_x, ell_y, ell_z];
    // ℓ^μ = η^{μν} ℓ_ν ⇒ (−1, ℓ_x, ℓ_y, ℓ_z)
    let ell_con = [-1.0, ell_x, ell_y, ell_z];

    let metric = assemble_metric(true, h, &ell_cov);
    let inverse_metric = assemble_metric(false, h, &ell_con);

    if !metric.is_finite() || !inverse_metric.is_finite() {
        return Err(CoreError::Unresolved {
            context: "Kerr-Schild metric tensors",
        });
    }

    Ok(KerrSchildQuantities {
        radius,
        h,
        ell_cov,
        ell_con,
        metric,
        inverse_metric,
    })
}

fn assemble_metric(covariant: bool, h: f64, ell: &[f64; 4]) -> MetricTensor {
    let mut data = MetricTensor::minkowski().components();
    let sign = if covariant { 1.0 } else { -1.0 };
    let factor = sign * 2.0 * h;
    for mu in 0..4 {
        for nu in 0..4 {
            data[mu][nu] += factor * ell[mu] * ell[nu];
        }
    }
    MetricTensor::from_symmetric(data)
}

/// Independent numerical inverse of a 4×4 matrix (Gauss–Jordan).
///
/// Used only as a test/diagnostic oracle; production `g^{μν}` uses the
/// Kerr–Schild closed form above.
pub fn matrix_inverse_oracle(m: &MetricTensor) -> Result<MetricTensor, CoreError> {
    let mut a = m.components();
    let mut inv = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    for col in 0..4 {
        let mut pivot = col;
        for row in col..4 {
            if a[row][col].abs() > a[pivot][col].abs() {
                pivot = row;
            }
        }
        if a[pivot][col].abs() < 1e-30 {
            return Err(CoreError::Unresolved {
                context: "metric matrix inverse pivot",
            });
        }
        if pivot != col {
            a.swap(pivot, col);
            inv.swap(pivot, col);
        }
        let diag = a[col][col];
        for j in 0..4 {
            a[col][j] /= diag;
            inv[col][j] /= diag;
        }
        for i in 0..4 {
            if i == col {
                continue;
            }
            let f = a[i][col];
            for j in 0..4 {
                a[i][j] -= f * a[col][j];
                inv[i][j] -= f * inv[col][j];
            }
        }
    }
    Ok(MetricTensor::from_symmetric(inv))
}

/// Lower a contravariant vector with the KS metric.
pub fn lower_vector(
    params: &KerrParams,
    pos: &PositionKs,
    v: &Vector,
) -> Result<crate::types::Covector, CoreError> {
    Ok(evaluate_kerr_schild(params, pos)?.metric.mul_vec(v))
}

/// Raise a covector with the KS inverse metric.
pub fn raise_covector(
    params: &KerrParams,
    pos: &PositionKs,
    p: &crate::types::Covector,
) -> Result<Vector, CoreError> {
    Ok(evaluate_kerr_schild(params, pos)?.inverse_metric.raise(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::identity_residual;

    #[test]
    fn metric_symmetric_and_inverse_identity() {
        let p = KerrParams::new(1.0, 0.9).unwrap();
        let pos = PositionKs::spatial(4.0, 1.0, 2.0);
        let q = evaluate_kerr_schild(&p, &pos).unwrap();
        assert!(q.metric.max_abs_asymmetry() < 1e-15);
        assert!(q.inverse_metric.max_abs_asymmetry() < 1e-15);
        let res = identity_residual(&q.metric, &q.inverse_metric);
        assert!(res < 1e-12, "identity residual {res}");
    }

    #[test]
    fn closed_form_inverse_matches_matrix_oracle() {
        let p = KerrParams::new(1.0, 0.5).unwrap();
        let pos = PositionKs::spatial(6.0, -2.0, 1.0);
        let q = evaluate_kerr_schild(&p, &pos).unwrap();
        let oracle = matrix_inverse_oracle(&q.metric).unwrap();
        let mut max: f64 = 0.0;
        for i in 0..4 {
            for j in 0..4 {
                max = max.max((q.inverse_metric.get(i, j) - oracle.get(i, j)).abs());
            }
        }
        assert!(max < 1e-10, "oracle disagreement {max}");
    }

    #[test]
    fn a_zero_schwarzschild_ks() {
        let p = KerrParams::new(1.0, 0.0).unwrap();
        let pos = PositionKs::spatial(0.0, 0.0, 10.0);
        let q = evaluate_kerr_schild(&p, &pos).unwrap();
        assert!((q.radius.r - 10.0).abs() < 1e-14);
        // H = M/r for a=0 on axis-equivalent Euclidean r.
        assert!((q.h - 0.1).abs() < 1e-14);
    }

    #[test]
    fn large_radius_approaches_minkowski() {
        let p = KerrParams::new(1.0, 0.7).unwrap();
        let pos = PositionKs::spatial(1.0e6, 0.0, 0.0);
        let q = evaluate_kerr_schild(&p, &pos).unwrap();
        let eta = MetricTensor::minkowski();
        let mut max: f64 = 0.0;
        for i in 0..4 {
            for j in 0..4 {
                max = max.max((q.metric.get(i, j) - eta.get(i, j)).abs());
            }
        }
        assert!(max < 1e-5, "weak-field residual {max}");
    }

    #[test]
    fn finite_across_outer_horizon() {
        let p = KerrParams::new(1.0, 0.9).unwrap();
        let r_plus = p.outer_horizon_radius();
        // Point with oblate r slightly inside r_+ (horizon-penetrating chart).
        let pos = PositionKs::spatial(r_plus * 0.95, 0.0, 0.0);
        let q = evaluate_kerr_schild(&p, &pos).unwrap();
        assert!(q.metric.is_finite());
        assert!(q.inverse_metric.is_finite());
        assert!(q.radius.r < r_plus);
    }

    #[test]
    fn finite_on_symmetry_axis() {
        let p = KerrParams::new(1.0, 0.999).unwrap();
        let pos = PositionKs::spatial(0.0, 0.0, 8.0);
        let q = evaluate_kerr_schild(&p, &pos).unwrap();
        assert!(q.metric.is_finite() && q.inverse_metric.is_finite());
    }
}
