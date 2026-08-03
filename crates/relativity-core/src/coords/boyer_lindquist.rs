//! Boyer–Lindquist ↔ Cartesian Kerr–Schild maps.
//!
//! Spatial KS embedding of BL coordinates (standard):
//! ```text
//! x = √(r²+a²) sinθ cosφ
//! y = √(r²+a²) sinθ sinφ
//! z = r cosθ
//! t_KS = t_BL   (same Killing time for this chart identification)
//! ```
//!
//! Vector/covector transforms use the Jacobian; they are **not** interchangeable.
//! Axis (`sinθ → 0`) and horizon (`Δ → 0` for some BL-dependent ops) return
//! typed ill-conditioned / singular errors rather than silent NaNs.
//!
//! Sources: Boyer & Lindquist (1967); Carter (1968); project physics-assumptions.

use crate::error::{CoreError, DomainReason};
use crate::kerr::KerrParams;
use crate::radius::evaluate_oblate_radius;
use crate::types::{Covector, PositionBl, PositionKs, Vector};

const AXIS_SIN_FLOOR: f64 = 1e-14;

/// BL position → Cartesian KS position.
pub fn bl_to_ks_position(params: &KerrParams, bl: &PositionBl) -> Result<PositionKs, CoreError> {
    bl.require_finite("BL position")?;
    if bl.r <= 0.0 {
        return Err(CoreError::IllConditioned {
            context: "BL r must be > 0 for KS embedding used in Gate 1A",
        });
    }
    let a = params.spin();
    let sth = bl.theta.sin();
    let cth = bl.theta.cos();
    let sph = bl.phi.sin();
    let cph = bl.phi.cos();
    let ra = (bl.r * bl.r + a * a).sqrt();
    if !ra.is_finite() {
        return Err(CoreError::Unresolved {
            context: "sqrt(r^2+a^2)",
        });
    }
    Ok(PositionKs::new(
        bl.t,
        ra * sth * cph,
        ra * sth * sph,
        bl.r * cth,
    ))
}

/// Cartesian KS → BL position (reporting chart).
pub fn ks_to_bl_position(params: &KerrParams, ks: &PositionKs) -> Result<PositionBl, CoreError> {
    ks.require_finite("KS position")?;
    let obl = evaluate_oblate_radius(params, ks)?;
    let r = obl.r;
    let cth = ks.z / r;
    if !cth.is_finite() || cth.abs() > 1.0 + 1e-12 {
        return Err(CoreError::IllConditioned {
            context: "KS to BL cosθ",
        });
    }
    let cth = cth.clamp(-1.0, 1.0);
    let theta = cth.acos();
    let sth = theta.sin();
    if sth.abs() < AXIS_SIN_FLOOR {
        return Err(CoreError::ChartDomain {
            x: ks.x,
            y: ks.y,
            z: ks.z,
            reason: DomainReason::BoyerLindquistSingular,
        });
    }
    // φ from x,y with the a-twist removed via standard atan2 on cylindrical coords.
    // x + i y = √(r²+a²) sinθ e^{iφ}
    let phi = ks.y.atan2(ks.x);
    if !phi.is_finite() || !theta.is_finite() {
        return Err(CoreError::Unresolved {
            context: "KS to BL angles",
        });
    }
    Ok(PositionBl::new(ks.t, r, theta, phi))
}

/// Jacobian `∂x_KS^μ / ∂x_BL^ν` at a BL event (rows KS, cols BL).
fn jacobian_ks_from_bl(params: &KerrParams, bl: &PositionBl) -> Result<[[f64; 4]; 4], CoreError> {
    bl.require_finite("BL jacobian")?;
    let a = params.spin();
    let r = bl.r;
    let sth = bl.theta.sin();
    let cth = bl.theta.cos();
    let sph = bl.phi.sin();
    let cph = bl.phi.cos();
    if sth.abs() < AXIS_SIN_FLOOR {
        return Err(CoreError::IllConditioned {
            context: "BL axis: jacobian ill-conditioned",
        });
    }
    let ra2 = r * r + a * a;
    let ra = ra2.sqrt();
    let dra_dr = r / ra;

    // x = ra sinθ cosφ, y = ra sinθ sinφ, z = r cosθ, t = t
    let mut j = [[0.0; 4]; 4];
    j[0][0] = 1.0; // ∂t/∂t

    // ∂x/∂r, ∂x/∂θ, ∂x/∂φ
    j[1][1] = dra_dr * sth * cph;
    j[1][2] = ra * cth * cph;
    j[1][3] = -ra * sth * sph;

    j[2][1] = dra_dr * sth * sph;
    j[2][2] = ra * cth * sph;
    j[2][3] = ra * sth * cph;

    j[3][1] = cth;
    j[3][2] = -r * sth;
    j[3][3] = 0.0;

    Ok(j)
}

/// Transform a BL contravariant vector to KS: `v_KS^μ = (∂x_KS^μ/∂x_BL^ν) v_BL^ν`.
pub fn vector_bl_to_ks(
    params: &KerrParams,
    bl: &PositionBl,
    v_bl: &Vector,
) -> Result<Vector, CoreError> {
    let j = jacobian_ks_from_bl(params, bl)?;
    let vb = v_bl.components();
    let mut out = [0.0; 4];
    for mu in 0..4 {
        for nu in 0..4 {
            out[mu] += j[mu][nu] * vb[nu];
        }
    }
    let v = Vector::from_components(out);
    if !v.is_finite() {
        return Err(CoreError::Unresolved {
            context: "vector BL→KS",
        });
    }
    Ok(v)
}

/// Transform a KS contravariant vector to BL via inverse Jacobian.
pub fn vector_ks_to_bl(
    params: &KerrParams,
    bl: &PositionBl,
    v_ks: &Vector,
) -> Result<Vector, CoreError> {
    let j = jacobian_ks_from_bl(params, bl)?;
    let j_inv = invert4(&j)?;
    let vk = v_ks.components();
    let mut out = [0.0; 4];
    for mu in 0..4 {
        for nu in 0..4 {
            out[mu] += j_inv[mu][nu] * vk[nu];
        }
    }
    let v = Vector::from_components(out);
    if !v.is_finite() {
        return Err(CoreError::Unresolved {
            context: "vector KS→BL",
        });
    }
    Ok(v)
}

/// Covector BL → KS: `p_μ^KS = p_ν^BL (∂x_BL^ν / ∂x_KS^μ) = p_ν^BL (J^{-1})^ν_μ`.
pub fn covector_bl_to_ks(
    params: &KerrParams,
    bl: &PositionBl,
    p_bl: &Covector,
) -> Result<Covector, CoreError> {
    let j = jacobian_ks_from_bl(params, bl)?;
    let j_inv = invert4(&j)?;
    let pb = p_bl.components();
    let mut out = [0.0; 4];
    for mu in 0..4 {
        for nu in 0..4 {
            // p_μ^KS = p_ν^BL * ∂x_BL^ν/∂x_KS^μ = p_ν * (J^{-1})^ν_μ
            out[mu] += pb[nu] * j_inv[nu][mu];
        }
    }
    let p = Covector::from_components(out);
    if !p.is_finite() {
        return Err(CoreError::Unresolved {
            context: "covector BL→KS",
        });
    }
    Ok(p)
}

/// Covector KS → BL: `p_ν^BL = p_μ^KS (∂x_KS^μ / ∂x_BL^ν) = p_μ^KS J^μ_ν`.
pub fn covector_ks_to_bl(
    params: &KerrParams,
    bl: &PositionBl,
    p_ks: &Covector,
) -> Result<Covector, CoreError> {
    let j = jacobian_ks_from_bl(params, bl)?;
    let pk = p_ks.components();
    let mut out = [0.0; 4];
    for nu in 0..4 {
        for mu in 0..4 {
            out[nu] += pk[mu] * j[mu][nu];
        }
    }
    let p = Covector::from_components(out);
    if !p.is_finite() {
        return Err(CoreError::Unresolved {
            context: "covector KS→BL",
        });
    }
    Ok(p)
}

fn invert4(m: &[[f64; 4]; 4]) -> Result<[[f64; 4]; 4], CoreError> {
    let mut a = *m;
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
        if a[pivot][col].abs() < 1e-14 {
            return Err(CoreError::IllConditioned {
                context: "coordinate jacobian singular",
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
    Ok(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_round_trip_off_axis() {
        let p = KerrParams::new(1.0, 0.8).unwrap();
        let bl = PositionBl::new(0.0, 12.0, 1.0, 0.3);
        let ks = bl_to_ks_position(&p, &bl).unwrap();
        let back = ks_to_bl_position(&p, &ks).unwrap();
        assert!((back.r - bl.r).abs() < 1e-12);
        assert!((back.theta - bl.theta).abs() < 1e-12);
        assert!((back.phi - bl.phi).abs() < 1e-12);
    }

    #[test]
    fn axis_is_typed_failure() {
        let p = KerrParams::new(1.0, 0.5).unwrap();
        let ks = PositionKs::spatial(0.0, 0.0, 10.0);
        assert!(matches!(
            ks_to_bl_position(&p, &ks),
            Err(CoreError::ChartDomain {
                reason: DomainReason::BoyerLindquistSingular,
                ..
            })
        ));
    }

    #[test]
    fn vector_and_covector_transforms_differ() {
        let p = KerrParams::new(1.0, 0.6).unwrap();
        let bl = PositionBl::new(0.0, 10.0, 1.2, 0.4);
        let v = Vector::new(1.0, 0.1, -0.2, 0.3);
        let v_ks = vector_bl_to_ks(&p, &bl, &v).unwrap();
        // Treating v components as a covector must not match the vector map.
        let p_as_cov = Covector::new(v.t, v.x, v.y, v.z);
        let p_ks = covector_bl_to_ks(&p, &bl, &p_as_cov).unwrap();
        let same = (v_ks.t - p_ks.t).abs()
            + (v_ks.x - p_ks.x).abs()
            + (v_ks.y - p_ks.y).abs()
            + (v_ks.z - p_ks.z).abs();
        assert!(same > 1e-6, "vector/covector maps must differ");
    }
}
