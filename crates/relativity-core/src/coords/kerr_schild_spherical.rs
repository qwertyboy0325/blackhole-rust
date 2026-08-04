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
//!
//! # Axis / pole policy
//!
//! `AXIS_SIN_FLOOR = 1e-14` is the centralized `|sin θ|` floor used by both the
//! generic spherical recovery (typed chart singularity) and the celestial
//! pole-tolerant direction API (canonical `ψ = 0` with explicit status).

use crate::error::{CoreError, DomainReason};
use crate::kerr::KerrParams;
use crate::radius::evaluate_oblate_radius;
use crate::types::PositionKs;

/// Centralized `|sin θ|` floor for spherical-KS axis detection.
///
/// Below this floor, azimuth `ψ` is undefined for the generic chart conversion.
/// Celestial mapping uses the same floor for pole canonicalization.
pub const AXIS_SIN_FLOOR: f64 = 1e-14;

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

/// Azimuth status for celestial / direction recovery (Gate 2A1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SphericalKsAzimuthStatus {
    Defined,
    CanonicalizedNorthPole,
    CanonicalizedSouthPole,
}

impl SphericalKsAzimuthStatus {
    /// Stable project-owned digest tag (not Debug/Display/serde).
    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::Defined => "spherical-ks-azimuth:defined",
            Self::CanonicalizedNorthPole => "spherical-ks-azimuth:canonicalized-north-pole",
            Self::CanonicalizedSouthPole => "spherical-ks-azimuth:canonicalized-south-pole",
        }
    }
}

/// Coordinate-sphere direction recovered from Cartesian KS position.
///
/// `unit_coordinate_direction = [sinθ cosψ, sinθ sinψ, cosθ]` in the spherical
/// KS angular chart. This is **not** generally `normalize([x,y,z])` at nonzero
/// spin and finite radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalKsDirection {
    pub r: f64,
    pub theta: f64,
    pub psi: f64,
    pub unit_coordinate_direction: [f64; 3],
    pub azimuth_status: SphericalKsAzimuthStatus,
}

enum SphericalAngleRecovery {
    Defined { r: f64, theta: f64, psi: f64 },
    NorthPole { r: f64 },
    SouthPole { r: f64 },
}

/// Shared inverse algebra: oblate `r`, `θ`, and either defined `ψ` or a pole.
fn recover_spherical_ks_angles(
    params: &KerrParams,
    ks: &PositionKs,
) -> Result<SphericalAngleRecovery, CoreError> {
    ks.require_finite("Cartesian KS for spherical recovery")?;
    let obl = evaluate_oblate_radius(params, ks)?;
    let r = obl.r;
    let cth = (ks.z / r).clamp(-1.0, 1.0);
    let theta = cth.acos();
    let sth = theta.sin();
    if sth.abs() < AXIS_SIN_FLOOR {
        // Pole: azimuth undefined. Classify by cosθ sign (north ≈ +1, south ≈ −1).
        if cth >= 0.0 {
            return Ok(SphericalAngleRecovery::NorthPole { r });
        }
        return Ok(SphericalAngleRecovery::SouthPole { r });
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
    let re = (ks.x * denom_re + ks.y * denom_im) / denom2;
    let im = (ks.y * denom_re - ks.x * denom_im) / denom2;
    let psi = im.atan2(re);
    if !psi.is_finite() || !theta.is_finite() {
        return Err(CoreError::Unresolved {
            context: "spherical KS angles",
        });
    }
    Ok(SphericalAngleRecovery::Defined { r, theta, psi })
}

fn coordinate_unit_direction(theta: f64, psi: f64) -> [f64; 3] {
    let sth = theta.sin();
    let cth = theta.cos();
    let sps = psi.sin();
    let cps = psi.cos();
    [sth * cps, sth * sps, cth]
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
///
/// Preserves Gate 1A chart-domain rejection of undefined pole azimuth.
pub fn spherical_ks_from_cartesian(
    params: &KerrParams,
    ks: &PositionKs,
) -> Result<PositionSphericalKs, CoreError> {
    match recover_spherical_ks_angles(params, ks)? {
        SphericalAngleRecovery::Defined { r, theta, psi } => {
            Ok(PositionSphericalKs::new(ks.t, r, theta, psi))
        }
        SphericalAngleRecovery::NorthPole { .. } | SphericalAngleRecovery::SouthPole { .. } => {
            Err(CoreError::ChartDomain {
                x: ks.x,
                y: ks.y,
                z: ks.z,
                reason: DomainReason::BoyerLindquistSingular,
            })
        }
    }
}

/// Pole-tolerant spherical KS coordinate direction for celestial mapping.
///
/// Non-pole samples recover `ψ` via the same inverse formula as
/// [`spherical_ks_from_cartesian`]. Poles canonicalize `ψ = 0` with an explicit
/// [`SphericalKsAzimuthStatus`].
pub fn spherical_ks_direction_from_cartesian(
    params: &KerrParams,
    position: &PositionKs,
) -> Result<SphericalKsDirection, CoreError> {
    match recover_spherical_ks_angles(params, position)? {
        SphericalAngleRecovery::Defined { r, theta, psi } => {
            let psi = canonicalize_neg_zero(psi.rem_euclid(std::f64::consts::TAU));
            // rem_euclid already in [0, 2π); unwrap atan2 range into that for direction.
            // Defined recovery returns atan2 in (−π, π]; wrap for direction consistency.
            let dir = coordinate_unit_direction(theta, psi);
            Ok(SphericalKsDirection {
                r,
                theta,
                psi,
                unit_coordinate_direction: dir,
                azimuth_status: SphericalKsAzimuthStatus::Defined,
            })
        }
        SphericalAngleRecovery::NorthPole { r } => Ok(SphericalKsDirection {
            r,
            theta: 0.0,
            psi: 0.0,
            unit_coordinate_direction: [0.0, 0.0, 1.0],
            azimuth_status: SphericalKsAzimuthStatus::CanonicalizedNorthPole,
        }),
        SphericalAngleRecovery::SouthPole { r } => Ok(SphericalKsDirection {
            r,
            theta: std::f64::consts::PI,
            psi: 0.0,
            unit_coordinate_direction: [0.0, 0.0, -1.0],
            azimuth_status: SphericalKsAzimuthStatus::CanonicalizedSouthPole,
        }),
    }
}

fn canonicalize_neg_zero(v: f64) -> f64 {
    if v == 0.0 {
        0.0
    } else {
        v
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_api_still_rejects_poles() {
        let p = KerrParams::new(1.0, 0.0).unwrap();
        let north = PositionKs::new(0.0, 0.0, 0.0, 10.0);
        assert!(matches!(
            spherical_ks_from_cartesian(&p, &north),
            Err(CoreError::ChartDomain { .. })
        ));
    }

    #[test]
    fn direction_api_canonicalizes_poles() {
        let p = KerrParams::new(1.0, 0.5).unwrap();
        let north =
            spherical_ks_direction_from_cartesian(&p, &PositionKs::new(0.0, 0.0, 0.0, 10.0))
                .unwrap();
        assert_eq!(
            north.azimuth_status,
            SphericalKsAzimuthStatus::CanonicalizedNorthPole
        );
        assert_eq!(north.psi, 0.0);
        assert_eq!(north.theta, 0.0);
        assert_eq!(north.unit_coordinate_direction, [0.0, 0.0, 1.0]);

        let south =
            spherical_ks_direction_from_cartesian(&p, &PositionKs::new(0.0, 0.0, 0.0, -10.0))
                .unwrap();
        assert_eq!(
            south.azimuth_status,
            SphericalKsAzimuthStatus::CanonicalizedSouthPole
        );
        assert_eq!(south.psi, 0.0);
        assert!((south.theta - std::f64::consts::PI).abs() < 1e-15);
        assert_eq!(south.unit_coordinate_direction, [0.0, 0.0, -1.0]);
    }
}
