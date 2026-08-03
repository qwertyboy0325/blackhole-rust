//! Analytic spatial derivatives of the inverse Kerr–Schild metric.
//!
//! Production path differentiates the closed form
//! `g^{μν} = η^{μν} − 2 H ℓ^μ ℓ^ν` with branch-aware `∂r`.
//! Tests compare against central finite differences of `g^{μν}` in a separate
//! oracle that must not call this module.

use crate::error::CoreError;
use crate::kerr::KerrParams;
use crate::metric::kerr_schild::evaluate_kerr_schild;
use crate::types::PositionKs;

/// Spatial coordinate index: `X=0, Y=1, Z=2` corresponding to `∂/∂x^i`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialDerivativeIndex {
    X = 0,
    Y = 1,
    Z = 2,
}

impl SpatialDerivativeIndex {
    #[must_use]
    pub fn all() -> [Self; 3] {
        [Self::X, Self::Y, Self::Z]
    }

    #[must_use]
    pub fn as_usize(self) -> usize {
        self as usize
    }
}

/// `∂_i g^{αβ}` for `i ∈ {x,y,z}` and `∂_t g^{αβ} = 0`.
#[derive(Debug, Clone, Copy)]
pub struct InverseMetricDerivatives {
    /// `partial[i][α][β] = ∂_{x^i} g^{αβ}` with `i=0,1,2` → x,y,z.
    pub spatial: [[[f64; 4]; 4]; 3],
}

impl InverseMetricDerivatives {
    #[must_use]
    pub fn dt(&self) -> [[f64; 4]; 4] {
        [[0.0; 4]; 4]
    }

    #[must_use]
    pub fn get(&self, mu_coord: usize, alpha: usize, beta: usize) -> f64 {
        if mu_coord == 0 {
            0.0
        } else {
            self.spatial[mu_coord - 1][alpha][beta]
        }
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.spatial
            .iter()
            .flatten()
            .flatten()
            .all(|v| v.is_finite())
    }
}

/// Analytic `∂_i g^{αβ}` at a KS event.
pub fn inverse_metric_spatial_derivatives(
    params: &KerrParams,
    pos: &PositionKs,
) -> Result<InverseMetricDerivatives, CoreError> {
    let q = evaluate_kerr_schild(params, pos)?;
    let radius = q.radius;
    let r = radius.r;
    let r2 = radius.r2;
    let a = params.spin();
    let m = params.mass();
    let x = pos.x;
    let y = pos.y;
    let z = pos.z;

    let a2 = a * a;
    let a_aux = radius.a_aux;
    let disc_inner = a_aux * a_aux + 4.0 * a2 * z * z;
    let d = disc_inner.sqrt();
    if !(d.is_finite() && d > 0.0) {
        // a=0 and z=0 with A=0 is measure-zero; treat as unresolved if needed.
        if a == 0.0 {
            // Euclidean r: ∂r/∂x_i = x_i / r
        } else {
            return Err(CoreError::Unresolved {
                context: "radius derivative discriminant",
            });
        }
    }

    let dr2 = partial_r2(a, a_aux, d, x, y, z, radius.used_direct_branch);
    let mut dr = [0.0; 3];
    for i in 0..3 {
        dr[i] = dr2[i] / (2.0 * r);
        if !dr[i].is_finite() {
            return Err(CoreError::Unresolved {
                context: "partial r",
            });
        }
    }

    let denom_h = r2 * r2 + a2 * z * z;
    let denom_h2 = denom_h * denom_h;
    let mut d_denom = [0.0; 3];
    for i in 0..3 {
        d_denom[i] = 4.0 * r2 * r * dr[i];
    }
    d_denom[2] += 2.0 * a2 * z;

    let mut dh = [0.0; 3];
    for i in 0..3 {
        // H = M r^3 / denom
        let num = m * r2 * r;
        let dnum = m * 3.0 * r2 * dr[i];
        dh[i] = (dnum * denom_h - num * d_denom[i]) / denom_h2;
        if !dh[i].is_finite() {
            return Err(CoreError::Unresolved {
                context: "partial H",
            });
        }
    }

    let ell = q.ell_con;
    let r2a2 = r2 + a2;
    let r2a2_sq = r2a2 * r2a2;

    let mut dell = [[0.0; 4]; 3]; // dell[i][μ] = ∂_i ℓ^μ; ℓ^t = -1 ⇒ ∂=0
    for i in 0..3 {
        // ℓ^x = (r x + a y) / (r²+a²)
        let num_x = r * x + a * y;
        let dnum_x = dr[i] * x + r * kron(i, 0) + a * kron(i, 1);
        dell[i][1] = (dnum_x * r2a2 - num_x * 2.0 * r * dr[i]) / r2a2_sq;

        let num_y = r * y - a * x;
        let dnum_y = dr[i] * y + r * kron(i, 1) - a * kron(i, 0);
        dell[i][2] = (dnum_y * r2a2 - num_y * 2.0 * r * dr[i]) / r2a2_sq;

        // ℓ^z = z/r
        dell[i][3] = (kron(i, 2) * r - z * dr[i]) / r2;

        for mu in 0..4 {
            if !dell[i][mu].is_finite() {
                return Err(CoreError::Unresolved {
                    context: "partial ell",
                });
            }
        }
    }

    let mut spatial = [[[0.0; 4]; 4]; 3];
    for i in 0..3 {
        for alpha in 0..4 {
            for beta in 0..4 {
                // ∂_i g^{αβ} = -2[ (∂_i H) ℓ^α ℓ^β + H (∂_i ℓ^α) ℓ^β + H ℓ^α (∂_i ℓ^β) ]
                let val = -2.0
                    * (dh[i] * ell[alpha] * ell[beta]
                        + q.h * dell[i][alpha] * ell[beta]
                        + q.h * ell[alpha] * dell[i][beta]);
                spatial[i][alpha][beta] = val;
            }
        }
    }

    let out = InverseMetricDerivatives { spatial };
    if !out.is_finite() {
        return Err(CoreError::Unresolved {
            context: "inverse metric derivatives",
        });
    }
    Ok(out)
}

fn kron(i: usize, j: usize) -> f64 {
    if i == j {
        1.0
    } else {
        0.0
    }
}

fn partial_r2(a: f64, a_aux: f64, d: f64, x: f64, y: f64, z: f64, used_direct: bool) -> [f64; 3] {
    let a2 = a * a;
    let da = [2.0 * x, 2.0 * y, 2.0 * z];
    let mut dd = [0.0; 3];
    if d > 0.0 {
        for i in 0..3 {
            let dz_term = if i == 2 { 4.0 * a2 * z } else { 0.0 };
            dd[i] = (a_aux * da[i] + dz_term) / d;
        }
    }

    if used_direct || a_aux >= 0.0 {
        [
            0.5 * (da[0] + dd[0]),
            0.5 * (da[1] + dd[1]),
            0.5 * (da[2] + dd[2]),
        ]
    } else {
        // r² = 2 a² z² / (D - A)
        let u = 2.0 * a2 * z * z;
        let v = d - a_aux;
        let du = [0.0, 0.0, 4.0 * a2 * z];
        let mut out = [0.0; 3];
        for i in 0..3 {
            let dv = dd[i] - da[i];
            out[i] = (du[i] * v - u * dv) / (v * v);
        }
        out
    }
}
