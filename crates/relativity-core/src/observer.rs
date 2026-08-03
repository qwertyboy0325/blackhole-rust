//! Observers and orthonormal tetrads for Gate 1A.
//!
//! Projection, observer motion, and coordinate transformation remain separate.
//! Sources: BPT1972 (ZAMO/LNRF); James2015 (camera tetrads); ADR 0003.

use crate::coords::{bl_to_ks_position, vector_bl_to_ks};
use crate::error::CoreError;
use crate::kerr::KerrParams;
use crate::metric::evaluate_kerr_schild;
use crate::metric::MinkowskiMetric;
use crate::types::{LocalComponents, MetricTensor, PositionBl, PositionKs, Vector};

/// Orthonormal tetrad `e_(a)^μ` with `e_(0) = u`.
#[derive(Debug, Clone, Copy)]
pub struct Tetrad {
    pub legs: [Vector; 4],
}

impl Tetrad {
    #[must_use]
    pub fn time_leg(&self) -> Vector {
        self.legs[0]
    }

    /// `v^μ = e_(a)^μ v^(a)`.
    #[must_use]
    pub fn push_local(&self, local: &LocalComponents) -> Vector {
        let c = local.components();
        let mut out = [0.0; 4];
        for a in 0..4 {
            let ea = self.legs[a].components();
            for mu in 0..4 {
                out[mu] += ea[mu] * c[a];
            }
        }
        Vector::from_components(out)
    }
}

/// Future-directed unit timelike observer at a KS event.
#[derive(Debug, Clone, Copy)]
pub struct Observer {
    pub event: PositionKs,
    pub four_velocity: Vector,
    pub tetrad: Tetrad,
}

/// Minkowski static observer with standard orthonormal frame.
pub fn minkowski_static_observer(event: PositionKs) -> Result<Observer, CoreError> {
    event.require_finite("minkowski observer event")?;
    let u = Vector::new(1.0, 0.0, 0.0, 0.0);
    let tetrad = Tetrad {
        legs: [
            u,
            Vector::new(0.0, 1.0, 0.0, 0.0),
            Vector::new(0.0, 0.0, 1.0, 0.0),
            Vector::new(0.0, 0.0, 0.0, 1.0),
        ],
    };
    let g = MinkowskiMetric.metric(&event);
    check_tetrad(&g, &tetrad)?;
    Ok(Observer {
        event,
        four_velocity: u,
        tetrad,
    })
}

/// Baseline ZAMO (LNRF) at a Boyer–Lindquist event, exported to Cartesian KS.
pub fn zamo_observer(params: &KerrParams, bl: &PositionBl) -> Result<Observer, CoreError> {
    bl.require_finite("ZAMO BL event")?;
    if bl.r <= 0.0 {
        return Err(CoreError::InvalidObserver {
            context: "ZAMO requires r > 0",
        });
    }
    let sth = bl.theta.sin();
    if sth.abs() < 1e-14 {
        return Err(CoreError::InvalidObserver {
            context: "ZAMO undefined / ill-conditioned on BL axis",
        });
    }

    let m = params.mass();
    let a = params.spin();
    let r = bl.r;
    let cth = bl.theta.cos();
    let sigma = r * r + a * a * cth * cth;
    let delta = r * r - 2.0 * m * r + a * a;
    if delta <= 0.0 {
        return Err(CoreError::InvalidObserver {
            context: "baseline ZAMO requires outside outer horizon (Δ > 0)",
        });
    }
    let a_factor = (r * r + a * a).powi(2) - delta * a * a * sth * sth;
    if !(a_factor.is_finite() && a_factor > 0.0 && sigma > 0.0) {
        return Err(CoreError::InvalidObserver {
            context: "ZAMO metric factors invalid",
        });
    }

    // Ω = 2 M a r / A ; u^t = √(A/(Δ Σ)); u^φ = Ω u^t
    let omega = 2.0 * m * a * r / a_factor;
    let u_t = (a_factor / (delta * sigma)).sqrt();
    let u_phi = omega * u_t;
    if !u_t.is_finite() || !u_phi.is_finite() || u_t <= 0.0 {
        return Err(CoreError::InvalidObserver {
            context: "ZAMO four-velocity non-finite",
        });
    }
    let u_bl = Vector::new(u_t, 0.0, 0.0, u_phi);

    // Spatial BL legs before Gram-Schmidt in KS: ∂_r, ∂_θ, ∂_φ directions.
    // Build in BL, push to KS, then orthonormalize against KS metric.
    let e_r_bl = Vector::new(0.0, 1.0, 0.0, 0.0);
    let e_theta_bl = Vector::new(0.0, 0.0, 1.0, 0.0);
    let e_phi_bl = Vector::new(0.0, 0.0, 0.0, 1.0);

    let event = bl_to_ks_position(params, bl)?;
    let u = vector_bl_to_ks(params, bl, &u_bl)?;
    let v1 = vector_bl_to_ks(params, bl, &e_r_bl)?;
    let v2 = vector_bl_to_ks(params, bl, &e_theta_bl)?;
    let v3 = vector_bl_to_ks(params, bl, &e_phi_bl)?;

    let g = evaluate_kerr_schild(params, &event)?.metric;
    let uu = g.contract(&u, &u);
    if !uu.is_finite() || uu >= -0.5 {
        return Err(CoreError::InvalidObserver {
            context: "ZAMO not timelike after KS push",
        });
    }
    // Renormalize in KS metric (Jacobian path can leave tiny drift).
    let u = normalize_timelike(&g, u)?;

    let tetrad = gram_schmidt_tetrad(&g, u, [v1, v2, v3])?;
    check_tetrad(&g, &tetrad)?;

    Ok(Observer {
        event,
        four_velocity: tetrad.time_leg(),
        tetrad,
    })
}

fn normalize_timelike(g: &MetricTensor, u: Vector) -> Result<Vector, CoreError> {
    let uu = g.contract(&u, &u);
    if !(uu < 0.0) {
        return Err(CoreError::InvalidObserver {
            context: "cannot normalize non-timelike vector",
        });
    }
    let n = (-uu).sqrt();
    let out = u.scale(1.0 / n);
    // Ensure future-directed: u^t > 0 in KS.
    if out.t <= 0.0 {
        return Err(CoreError::InvalidObserver {
            context: "observer not future-directed (u^t <= 0)",
        });
    }
    Ok(out)
}

fn gram_schmidt_tetrad(
    g: &MetricTensor,
    u: Vector,
    candidates: [Vector; 3],
) -> Result<Tetrad, CoreError> {
    let mut legs = [
        u,
        Vector::new(0.0, 0.0, 0.0, 0.0),
        Vector::new(0.0, 0.0, 0.0, 0.0),
        Vector::new(0.0, 0.0, 0.0, 0.0),
    ];
    for (idx, mut v) in candidates.into_iter().enumerate() {
        // Metric Gram–Schmidt: v ← v − [g(v,e)/g(e,e)] e
        // For timelike e0 with g(e0,e0)=−1: v ← v + g(v,e0) e0
        let gv0 = g.contract(&v, &legs[0]);
        v = add(v, legs[0].scale(gv0));
        for j in 1..=idx {
            let alpha = g.contract(&v, &legs[j]);
            v = add(v, legs[j].scale(-alpha));
        }
        let vv = g.contract(&v, &v);
        if !(vv > 1e-14) {
            return Err(CoreError::TetradFailure {
                context: "Gram-Schmidt spatial leg collapsed",
            });
        }
        legs[idx + 1] = v.scale(1.0 / vv.sqrt());
    }

    // Enforce right-handed spatial triad relative to e0 via spacetime volume form
    // approx: ε_{txyz} orientation with Minkowski-like check on components at weak field,
    // and metric-aware scalar triple product on spatial parts projected orthogonally.
    if spacetime_handedness(g, &legs) < 0.0 {
        legs[3] = legs[3].scale(-1.0);
    }

    Ok(Tetrad { legs })
}

fn add(a: Vector, b: Vector) -> Vector {
    Vector::new(a.t + b.t, a.x + b.x, a.y + b.y, a.z + b.z)
}

fn spacetime_handedness(g: &MetricTensor, legs: &[Vector; 4]) -> f64 {
    // Oriented volume: det(e_a^μ) * sqrt(|det g|) sign. Use det of components.
    det4([
        legs[0].components(),
        legs[1].components(),
        legs[2].components(),
        legs[3].components(),
    ]) * det4_metric_sign(g)
}

fn det4_metric_sign(g: &MetricTensor) -> f64 {
    // For Lorentzian (−+++) det g < 0; orientation factor uses sign(det e).
    let d = det4(g.components());
    if d < 0.0 {
        1.0
    } else {
        -1.0
    }
}

fn det4(m: [[f64; 4]; 4]) -> f64 {
    // Leibniz formula
    let mut det = 0.0;
    let perm = [
        ([0, 1, 2, 3], 1.0),
        ([0, 1, 3, 2], -1.0),
        ([0, 2, 1, 3], -1.0),
        ([0, 2, 3, 1], 1.0),
        ([0, 3, 1, 2], 1.0),
        ([0, 3, 2, 1], -1.0),
        ([1, 0, 2, 3], -1.0),
        ([1, 0, 3, 2], 1.0),
        ([1, 2, 0, 3], 1.0),
        ([1, 2, 3, 0], -1.0),
        ([1, 3, 0, 2], -1.0),
        ([1, 3, 2, 0], 1.0),
        ([2, 0, 1, 3], 1.0),
        ([2, 0, 3, 1], -1.0),
        ([2, 1, 0, 3], -1.0),
        ([2, 1, 3, 0], 1.0),
        ([2, 3, 0, 1], 1.0),
        ([2, 3, 1, 0], -1.0),
        ([3, 0, 1, 2], -1.0),
        ([3, 0, 2, 1], 1.0),
        ([3, 1, 0, 2], 1.0),
        ([3, 1, 2, 0], -1.0),
        ([3, 2, 0, 1], -1.0),
        ([3, 2, 1, 0], 1.0),
    ];
    for (p, s) in perm {
        det += s * m[0][p[0]] * m[1][p[1]] * m[2][p[2]] * m[3][p[3]];
    }
    det
}

/// Verify orthonormality, future-direction, and handedness.
pub fn check_tetrad(g: &MetricTensor, tetrad: &Tetrad) -> Result<(), CoreError> {
    let eta = [
        [-1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut max_err: f64 = 0.0;
    for a in 0..4 {
        for b in 0..4 {
            let gab = g.contract(&tetrad.legs[a], &tetrad.legs[b]);
            max_err = max_err.max((gab - eta[a][b]).abs());
        }
    }
    // Tolerance provenance: algebraic fp noise for orthonormalization; provisional smoke.
    if max_err > 1e-10 {
        return Err(CoreError::TetradFailure {
            context: "orthonormality residual exceeds 1e-10",
        });
    }
    if tetrad.legs[0].t <= 0.0 {
        return Err(CoreError::TetradFailure {
            context: "time leg not future-directed",
        });
    }
    if spacetime_handedness(g, &tetrad.legs) <= 0.0 {
        return Err(CoreError::TetradFailure {
            context: "tetrad not right-handed",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minkowski_tetrad_ok() {
        let obs = minkowski_static_observer(PositionKs::spatial(0.0, 0.0, 0.0)).unwrap();
        let g = MinkowskiMetric.metric(&obs.event);
        assert!((g.contract(&obs.four_velocity, &obs.four_velocity) + 1.0).abs() < 1e-15);
        check_tetrad(&g, &obs.tetrad).unwrap();
    }

    #[test]
    fn zamo_normalized_and_orthonormal() {
        let p = KerrParams::new(1.0, 0.999).unwrap();
        let bl = PositionBl::new(0.0, 20.0, 85.0_f64.to_radians(), 0.0);
        let obs = zamo_observer(&p, &bl).unwrap();
        let g = evaluate_kerr_schild(&p, &obs.event).unwrap().metric;
        let uu = g.contract(&obs.four_velocity, &obs.four_velocity);
        assert!((uu + 1.0).abs() < 1e-10, "uu={uu}");
        check_tetrad(&g, &obs.tetrad).unwrap();
    }

    #[test]
    fn zamo_rejects_inside_horizon() {
        let p = KerrParams::new(1.0, 0.5).unwrap();
        let bl = PositionBl::new(0.0, 1.2, 1.0, 0.0);
        assert!(zamo_observer(&p, &bl).is_err());
    }
}
