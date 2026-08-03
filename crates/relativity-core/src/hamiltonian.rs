//! Canonical null Hamiltonian RHS evaluation at a single state.
//!
//! ```text
//! H = ½ g^{μν} p_μ p_ν
//! dx^μ/dλ = g^{μν} p_ν
//! dp_μ/dλ = −½ (∂_μ g^{αβ}) p_α p_β
//! ```
//!
//! Gate 1A does **not** project onto `H = 0` and does not own adaptive stepping.
//! Sources: Carter (1968); MTW (1973); ADR 0001.

use crate::error::{CoreError, EvalStatus};
use crate::kerr::KerrParams;
use crate::metric::{evaluate_kerr_schild, inverse_metric_spatial_derivatives};
use crate::types::{Covector, PositionKs, Vector};

/// Hamiltonian point evaluation diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct HamiltonianEval {
    pub h: f64,
    pub dx_dlambda: Vector,
    pub dp_dlambda: Covector,
    pub status: EvalStatus,
    /// `E = -p_t` (Killing energy from KS/BL shared t).
    pub energy_like: f64,
    /// Chart-space angular-momentum proxy `x p_y - y p_x` (KS Cartesian).
    pub lz_cartesian_proxy: f64,
    /// Provisional Carter-related scalar only when BL angles are well-conditioned.
    pub carter_related: Option<f64>,
}

/// Evaluate the Hamiltonian and RHS at `(x, p)` without modifying the state.
pub fn evaluate_hamiltonian(
    params: &KerrParams,
    pos: &PositionKs,
    p: &Covector,
) -> Result<HamiltonianEval, CoreError> {
    pos.require_finite("hamiltonian position")?;
    if !p.is_finite() {
        return Err(CoreError::NonFinite {
            context: "hamiltonian momentum",
        });
    }

    let geo = evaluate_kerr_schild(params, pos)?;
    let ginv = geo.inverse_metric;
    let h = 0.5 * ginv.contract_cov(p, p);
    let dx = ginv.raise(p);

    let dginv = inverse_metric_spatial_derivatives(params, pos)?;
    let pc = p.components();
    let mut dp = [0.0; 4];
    // ∂_t g^{αβ} = 0 ⇒ dp_t/dλ = 0
    dp[0] = 0.0;
    for i in 0..3 {
        let mut s = 0.0;
        let dg = dginv.spatial[i];
        for alpha in 0..4 {
            for beta in 0..4 {
                s += dg[alpha][beta] * pc[alpha] * pc[beta];
            }
        }
        dp[i + 1] = -0.5 * s;
    }

    let dp_cov = Covector::from_components(dp);
    if !h.is_finite() || !dx.is_finite() || !dp_cov.is_finite() {
        return Err(CoreError::Unresolved {
            context: "hamiltonian RHS non-finite",
        });
    }

    Ok(HamiltonianEval {
        h,
        dx_dlambda: dx,
        dp_dlambda: dp_cov,
        status: EvalStatus::Ok,
        energy_like: -p.t,
        lz_cartesian_proxy: pos.x * p.y - pos.y * p.x,
        carter_related: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dp_t_vanishes_for_stationary_metric() {
        let params = KerrParams::new(1.0, 0.9).unwrap();
        let pos = PositionKs::spatial(8.0, 1.0, 2.0);
        let p = Covector::new(-1.0, 0.2, -0.1, 0.05);
        let ev = evaluate_hamiltonian(&params, &pos, &p).unwrap();
        assert_eq!(ev.dp_dlambda.t, 0.0);
    }

    #[test]
    fn does_not_project_h_to_zero() {
        let params = KerrParams::new(1.0, 0.5).unwrap();
        let pos = PositionKs::spatial(10.0, 0.0, 0.0);
        let p = Covector::new(-1.0, 0.5, 0.0, 0.0);
        let ev = evaluate_hamiltonian(&params, &pos, &p).unwrap();
        // Generic covector is not null; H must be reported honestly.
        assert!(ev.h.abs() > 1e-6);
    }
}
