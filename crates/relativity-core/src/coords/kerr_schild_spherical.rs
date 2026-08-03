//! Spherical Kerr–Schild chart `(T, r, θ, ψ)`.
//!
//! Spatial embedding into Cartesian KS (ingoing convention matching `ℓ_μ`):
//! ```text
//! x + i y = (r + i a) e^{iψ} sinθ
//! x = (r cosψ − a sinψ) sinθ
//! y = (r sinψ + a cosψ) sinθ
//! z = r cosθ
//! t = T
//! ```
//!
//! Sources: Kerr–Schild form as used with GRay2 `ℓ_μ`; Visser KS notes;
//! owner Gate 1A remediation (ingoing exterior differentials).

use crate::error::{CoreError, DomainReason};
use crate::kerr::KerrParams;
use crate::radius::evaluate_oblate_radius;
use crate::types::PositionKs;

const AXIS_SIN_FLOOR: f64 = 1e-14;

/// Spherical Kerr–Schild event. `t` is the KS time `T`; `psi` is the KS azimuth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionSphericalKs {
    pub t: f64,
    pub r: f64,
    pub theta: f64,
    pub psi: f64,
}

impl PositionSphericalKs {
    #[must_use]
    pub const fn new(t: f64, r: f64, theta: f64, psi: f64) -> Self {
        Self { t, r, theta, psi }
    }

    pub fn require_finite(&self, context: &'static str) -> Result<(), CoreError> {
        if self.t.is_finite()
            && self.r.is_finite()
            && self.theta.is_finite()
            && self.psi.is_finite()
        {
            Ok(())
        } else {
            Err(CoreError::NonFinite { context })
        }
    }
}

/// Cartesian KS position from spherical KS (same `T`).
pub fn cartesian_from_spherical_ks(
    params: &KerrParams,
    sph: &PositionSphericalKs,
) -> Result<PositionKs, CoreError> {
    sph.require_finite("spherical KS position")?;
    if sph.r <= 0.0 {
        return Err(CoreError::IllConditioned {
            context: "spherical KS r must be > 0",
        });
    }
    let a = params.spin();
    let sth = sph.theta.sin();
    let cth = sph.theta.cos();
    let sps = sph.psi.sin();
    let cps = sph.psi.cos();
    let x = (sph.r * cps - a * sps) * sth;
    let y = (sph.r * sps + a * cps) * sth;
    let z = sph.r * cth;
    if ![x, y, z].iter().all(|v| v.is_finite()) {
        return Err(CoreError::Unresolved {
            context: "spherical→Cartesian KS",
        });
    }
    Ok(PositionKs::new(sph.t, x, y, z))
}

/// Recover spherical KS angles from Cartesian KS (axis → typed singular).
pub fn spherical_ks_from_cartesian(
    params: &KerrParams,
    ks: &PositionKs,
) -> Result<PositionSphericalKs, CoreError> {
    ks.require_finite("Cartesian KS for spherical recovery")?;
    let obl = evaluate_oblate_radius(params, ks)?;
    let r = obl.r;
    let cth = (ks.z / r).clamp(-1.0, 1.0);
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
    let a = params.spin();
    // e^{iψ} = (x+iy) / ((r+ia) sinθ)
    let denom_re = r * sth;
    let denom_im = a * sth;
    let denom2 = denom_re * denom_re + denom_im * denom_im;
    if !(denom2 > 0.0) {
        return Err(CoreError::IllConditioned {
            context: "spherical KS ψ denominator",
        });
    }
    // (x+iy)/(denom_re + i denom_im)
    let re = (ks.x * denom_re + ks.y * denom_im) / denom2;
    let im = (ks.y * denom_re - ks.x * denom_im) / denom2;
    let psi = im.atan2(re);
    if !psi.is_finite() || !theta.is_finite() {
        return Err(CoreError::Unresolved {
            context: "spherical KS angles",
        });
    }
    Ok(PositionSphericalKs::new(ks.t, r, theta, psi))
}

/// Jacobian `∂x_cart^μ / ∂x_sph^ν` with ordering `(T,r,θ,ψ)` → `(t,x,y,z)`.
pub fn jacobian_cartesian_from_spherical_ks(
    params: &KerrParams,
    sph: &PositionSphericalKs,
) -> Result<[[f64; 4]; 4], CoreError> {
    sph.require_finite("spherical KS jacobian")?;
    if sph.r <= 0.0 {
        return Err(CoreError::IllConditioned {
            context: "spherical KS jacobian r <= 0",
        });
    }
    let a = params.spin();
    let r = sph.r;
    let sth = sph.theta.sin();
    let cth = sph.theta.cos();
    let sps = sph.psi.sin();
    let cps = sph.psi.cos();
    if sth.abs() < AXIS_SIN_FLOOR {
        return Err(CoreError::IllConditioned {
            context: "spherical KS axis: jacobian ill-conditioned",
        });
    }

    let mut j = [[0.0; 4]; 4];
    // t = T
    j[0][0] = 1.0;

    // x = (r cps - a sps) sth
    j[1][1] = cps * sth;
    j[1][2] = (r * cps - a * sps) * cth;
    j[1][3] = (-r * sps - a * cps) * sth;

    // y = (r sps + a cps) sth
    j[2][1] = sps * sth;
    j[2][2] = (r * sps + a * cps) * cth;
    j[2][3] = (r * cps - a * sps) * sth;

    // z = r cth
    j[3][1] = cth;
    j[3][2] = -r * sth;
    j[3][3] = 0.0;

    Ok(j)
}
