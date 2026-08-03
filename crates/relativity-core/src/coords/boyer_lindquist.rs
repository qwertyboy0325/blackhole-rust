//! Boyer–Lindquist ↔ Cartesian Kerr–Schild via spherical KS.
//!
//! ## Ingoing Kerr–Schild convention (project Gate 1A)
//!
//! Exterior differentials between BL `(t,r,θ,φ)` and spherical KS `(T,r,θ,ψ)`:
//! ```text
//! dT = dt_BL + (2 M r / Δ) dr
//! dψ = dφ_BL + (a / Δ) dr
//! Δ  = r² − 2 M r + a²
//! ```
//! with identity maps on `r` and `θ`.
//!
//! Spatial Cartesian embedding (matches `ℓ_μ` used by `metric::kerr_schild`):
//! ```text
//! x + i y = (r + i a) e^{iψ} sinθ
//! z = r cosθ
//! t_cart = T
//! ```
//!
//! Event placement gauge: at a BL event we set `T = t_BL` and `ψ = φ_BL`
//! (vanishing integration constants at that event). Vector/covector maps still
//! use the full Jacobian including `∂T/∂r` and `∂ψ/∂r`.
//!
//! Do **not** treat `t`/`φ` as identical to `T`/`ψ` outside this gauge choice.
//!
//! Sources: Boyer & Lindquist (1967); Carter (1968); Kerr–Schild embedding as
//! used with GRay2 `ℓ_μ`; owner Gate 1A remediation note.

use crate::coords::kerr_schild_spherical::{
    cartesian_from_spherical_ks, jacobian_cartesian_from_spherical_ks, spherical_ks_from_cartesian,
    PositionSphericalKs,
};
use crate::error::CoreError;
use crate::kerr::KerrParams;
use crate::types::{Covector, MetricTensor, PositionBl, PositionKs, Vector};

const AXIS_SIN_FLOOR: f64 = 1e-14;
const HORIZON_DELTA_FLOOR: f64 = 1e-14;

/// Independent Boyer–Lindquist Kerr metric at an exterior (or interior) BL event.
///
/// Standard components with signature `(-,+,+,+)`:
/// ```text
/// Σ = r² + a² cos²θ
/// Δ = r² − 2 M r + a²
/// A = (r²+a²)² − Δ a² sin²θ
/// g_tt = −(1 − 2 M r / Σ)
/// g_tφ = −2 M a r sin²θ / Σ
/// g_rr = Σ/Δ
/// g_θθ = Σ
/// g_φφ = A sin²θ / Σ
/// ```
pub fn bl_metric(params: &KerrParams, bl: &PositionBl) -> Result<MetricTensor, CoreError> {
    bl.require_finite("BL metric")?;
    if bl.r <= 0.0 {
        return Err(CoreError::IllConditioned {
            context: "BL metric requires r > 0",
        });
    }
    let sth = bl.theta.sin();
    if sth.abs() < AXIS_SIN_FLOOR {
        return Err(CoreError::IllConditioned {
            context: "BL metric ill-conditioned on axis",
        });
    }
    let m = params.mass();
    let a = params.spin();
    let r = bl.r;
    let cth = bl.theta.cos();
    let sigma = r * r + a * a * cth * cth;
    let delta = r * r - 2.0 * m * r + a * a;
    if !(sigma > 0.0 && sigma.is_finite()) {
        return Err(CoreError::Unresolved { context: "BL Σ" });
    }
    if delta.abs() < HORIZON_DELTA_FLOOR {
        return Err(CoreError::IllConditioned {
            context: "BL metric singular at horizon (Δ≈0)",
        });
    }
    let sth2 = sth * sth;
    let a_factor = (r * r + a * a).powi(2) - delta * a * a * sth2;
    let g_tt = -(1.0 - 2.0 * m * r / sigma);
    let g_tphi = -2.0 * m * a * r * sth2 / sigma;
    let g_rr = sigma / delta;
    let g_thth = sigma;
    let g_phiphi = a_factor * sth2 / sigma;

    // Lower triangle only; checked mirror constructor.
    MetricTensor::from_lower_triangle([
        [g_tt, 0.0, 0.0, g_tphi],
        [0.0, g_rr, 0.0, 0.0],
        [0.0, 0.0, g_thth, 0.0],
        [g_tphi, 0.0, 0.0, g_phiphi],
    ])
}

/// BL position → Cartesian KS using spherical-KS intermediate and placement gauge.
pub fn bl_to_ks_position(params: &KerrParams, bl: &PositionBl) -> Result<PositionKs, CoreError> {
    let sph = bl_to_spherical_ks_placement(params, bl)?;
    cartesian_from_spherical_ks(params, &sph)
}

/// Cartesian KS → BL reporting coordinates under the placement gauge `ψ ↔ φ`, `T ↔ t`.
pub fn ks_to_bl_position(params: &KerrParams, ks: &PositionKs) -> Result<PositionBl, CoreError> {
    let sph = spherical_ks_from_cartesian(params, ks)?;
    // Reporting gauge: identify φ with ψ and t with T at the event.
    Ok(PositionBl::new(sph.t, sph.r, sph.theta, sph.psi))
}

fn bl_to_spherical_ks_placement(
    params: &KerrParams,
    bl: &PositionBl,
) -> Result<PositionSphericalKs, CoreError> {
    bl.require_finite("BL→spherical KS")?;
    if bl.r <= 0.0 {
        return Err(CoreError::IllConditioned {
            context: "BL r must be > 0 for KS embedding",
        });
    }
    let _ = params;
    // Placement gauge: T=t, ψ=φ at this event.
    Ok(PositionSphericalKs::new(bl.t, bl.r, bl.theta, bl.phi))
}

/// Jacobian `∂x_cart^μ / ∂x_BL^ν` at a BL event (rows Cartesian KS, cols BL).
pub fn jacobian_cartesian_ks_from_bl(
    params: &KerrParams,
    bl: &PositionBl,
) -> Result<[[f64; 4]; 4], CoreError> {
    bl.require_finite("BL→KS jacobian")?;
    let sth = bl.theta.sin();
    if sth.abs() < AXIS_SIN_FLOOR {
        return Err(CoreError::IllConditioned {
            context: "BL axis: jacobian ill-conditioned",
        });
    }
    let m = params.mass();
    let a = params.spin();
    let r = bl.r;
    let delta = r * r - 2.0 * m * r + a * a;
    if delta.abs() < HORIZON_DELTA_FLOOR {
        return Err(CoreError::IllConditioned {
            context: "BL↔KS jacobian singular near horizon (Δ≈0)",
        });
    }
    if delta < 0.0 {
        // Interior: Δ < 0 is mathematically fine for the differential map, but
        // BL reporting remains singular for many other operations. Allow with care.
    }

    let sph = bl_to_spherical_ks_placement(params, bl)?;
    let j_cs = jacobian_cartesian_from_spherical_ks(params, &sph)?;

    // J_sph←BL: (T,r,θ,ψ) from (t,r,θ,φ)
    // T_t=1, T_r=2Mr/Δ; r_r=1; θ_θ=1; ψ_φ=1, ψ_r=a/Δ
    let mut j_sb = [[0.0; 4]; 4];
    j_sb[0][0] = 1.0;
    j_sb[0][1] = 2.0 * m * r / delta;
    j_sb[1][1] = 1.0;
    j_sb[2][2] = 1.0;
    j_sb[3][1] = a / delta;
    j_sb[3][3] = 1.0;

    // J_cart←BL = J_cart←sph · J_sph←BL
    let mut j = [[0.0; 4]; 4];
    for mu in 0..4 {
        for nu in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += j_cs[mu][k] * j_sb[k][nu];
            }
            j[mu][nu] = s;
        }
    }

    // Explicit exterior radial time/azimuth terms must be present when a,M ≠ 0.
    if m != 0.0 && (j[0][1] - 2.0 * m * r / delta).abs() > 1e-12 {
        return Err(CoreError::Unresolved {
            context: "jacobian ∂T/∂r mismatch",
        });
    }

    Ok(j)
}

/// Transform a BL contravariant vector to Cartesian KS.
pub fn vector_bl_to_ks(
    params: &KerrParams,
    bl: &PositionBl,
    v_bl: &Vector,
) -> Result<Vector, CoreError> {
    let j = jacobian_cartesian_ks_from_bl(params, bl)?;
    apply_jacobian_vector(&j, v_bl, "vector BL→KS")
}

/// Transform a Cartesian KS contravariant vector to BL.
pub fn vector_ks_to_bl(
    params: &KerrParams,
    bl: &PositionBl,
    v_ks: &Vector,
) -> Result<Vector, CoreError> {
    let j = jacobian_cartesian_ks_from_bl(params, bl)?;
    let j_inv = invert4(&j)?;
    apply_jacobian_vector(&j_inv, v_ks, "vector KS→BL")
}

/// Covector BL → KS.
pub fn covector_bl_to_ks(
    params: &KerrParams,
    bl: &PositionBl,
    p_bl: &Covector,
) -> Result<Covector, CoreError> {
    let j = jacobian_cartesian_ks_from_bl(params, bl)?;
    let j_inv = invert4(&j)?;
    let pb = p_bl.components();
    let mut out = [0.0; 4];
    for mu in 0..4 {
        for nu in 0..4 {
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

/// Covector KS → BL.
pub fn covector_ks_to_bl(
    params: &KerrParams,
    bl: &PositionBl,
    p_ks: &Covector,
) -> Result<Covector, CoreError> {
    let j = jacobian_cartesian_ks_from_bl(params, bl)?;
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

fn apply_jacobian_vector(
    j: &[[f64; 4]; 4],
    v: &Vector,
    context: &'static str,
) -> Result<Vector, CoreError> {
    let vc = v.components();
    let mut out = [0.0; 4];
    for mu in 0..4 {
        for nu in 0..4 {
            out[mu] += j[mu][nu] * vc[nu];
        }
    }
    let vout = Vector::from_components(out);
    if !vout.is_finite() {
        return Err(CoreError::Unresolved { context });
    }
    Ok(vout)
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
    use crate::error::{CoreError as CE, DomainReason};

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
    fn spatial_embedding_is_twisted_not_ra_form() {
        let p = KerrParams::new(1.0, 0.9).unwrap();
        let bl = PositionBl::new(0.0, 10.0, 1.0, 0.4);
        let ks = bl_to_ks_position(&p, &bl).unwrap();
        let ra = (bl.r * bl.r + p.spin() * p.spin()).sqrt();
        let naive_x = ra * bl.theta.sin() * bl.phi.cos();
        assert!(
            (ks.x - naive_x).abs() > 1e-3,
            "must not use √(r²+a²) embedding"
        );
    }

    #[test]
    fn jacobian_includes_radial_time_and_azimuth_terms() {
        let p = KerrParams::new(1.0, 0.7).unwrap();
        let bl = PositionBl::new(0.0, 8.0, 1.1, 0.2);
        let j = jacobian_cartesian_ks_from_bl(&p, &bl).unwrap();
        let delta = bl.r * bl.r - 2.0 * p.mass() * bl.r + p.spin() * p.spin();
        assert!((j[0][1] - 2.0 * p.mass() * bl.r / delta).abs() < 1e-12);
        // ∂ψ/∂r contributes to spatial columns via the spherical→Cartesian map.
        assert!(j[1][1].is_finite() && j[2][1].is_finite());
    }

    #[test]
    fn axis_and_horizon_are_typed_failures() {
        let p = KerrParams::new(1.0, 0.5).unwrap();
        let ks = PositionKs::spatial(0.0, 0.0, 10.0);
        assert!(matches!(
            ks_to_bl_position(&p, &ks),
            Err(CE::ChartDomain {
                reason: DomainReason::BoyerLindquistSingular,
                ..
            })
        ));
        let r_plus = p.outer_horizon_radius();
        let bl_h = PositionBl::new(0.0, r_plus, 1.0, 0.0);
        assert!(matches!(
            jacobian_cartesian_ks_from_bl(&p, &bl_h),
            Err(CE::IllConditioned { .. })
        ));
    }

    #[test]
    fn vector_and_covector_transforms_differ() {
        let p = KerrParams::new(1.0, 0.6).unwrap();
        let bl = PositionBl::new(0.0, 10.0, 1.2, 0.4);
        let v = Vector::new(1.0, 0.1, -0.2, 0.3);
        let v_ks = vector_bl_to_ks(&p, &bl, &v).unwrap();
        let p_as_cov = Covector::new(v.t, v.x, v.y, v.z);
        let p_ks = covector_bl_to_ks(&p, &bl, &p_as_cov).unwrap();
        let same = (v_ks.t - p_ks.t).abs()
            + (v_ks.x - p_ks.x).abs()
            + (v_ks.y - p_ks.y).abs()
            + (v_ks.z - p_ks.z).abs();
        assert!(same > 1e-6, "vector/covector maps must differ");
    }
}
