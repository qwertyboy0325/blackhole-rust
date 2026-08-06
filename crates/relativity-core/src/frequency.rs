//! Measured frequency and frequency-ratio kinematics (Gate 2B0).
//!
//! Stored photon covector is **past-directed** (`p_backward`). The equivalent
//! future-directed momentum is `k_future = -p_backward`.
//!
//! For signature `(-,+,+,+)` and a future-directed timelike observer `u`:
//!
//! ```text
//! ν = p_backward_μ u^μ
//!   = -k_future_μ u^μ
//! ```
//!
//! Both expressions must be finite and strictly positive. Do not implement
//! `-p_backward_μ u^μ` for the backward API — that reverses the project
//! orientation convention.
//!
//! No metric tensor is required for a covector/vector contraction.

use crate::error::CoreError;
use crate::types::{Covector, Vector};

/// Checked covector/vector pairing `p_μ u^μ` (component zip-sum).
#[must_use]
pub fn contract_covector_vector(p: &Covector, u: &Vector) -> f64 {
    p.t * u.t + p.x * u.x + p.y * u.y + p.z * u.z
}

/// Positive frequency measured by a future-directed timelike observer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredFrequency(f64);

impl MeasuredFrequency {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        if !value.is_finite() {
            return Err(CoreError::InvalidFrequency {
                context: "non-finite measured frequency",
            });
        }
        if !(value > 0.0) {
            return Err(CoreError::InvalidFrequency {
                context: "measured frequency must be strictly positive",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn to_bits(self) -> u64 {
        self.0.to_bits()
    }
}

/// Frequency ratio `g = ν_obs / ν_em` (strictly positive and finite).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrequencyShift(f64);

impl FrequencyShift {
    pub fn new(value: f64) -> Result<Self, CoreError> {
        if !value.is_finite() {
            return Err(CoreError::InvalidFrequency {
                context: "non-finite frequency shift",
            });
        }
        if !(value > 0.0) {
            return Err(CoreError::InvalidFrequency {
                context: "frequency shift must be strictly positive",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn log2(self) -> f64 {
        self.0.log2()
    }

    #[must_use]
    pub fn to_bits(self) -> u64 {
        self.0.to_bits()
    }
}

/// `ν = p_backward_μ u^μ` (must be finite and > 0).
pub fn measured_frequency_from_backward_covector(
    p_backward: &Covector,
    observer_velocity: &Vector,
) -> Result<MeasuredFrequency, CoreError> {
    if !p_backward.is_finite() || !observer_velocity.is_finite() {
        return Err(CoreError::NonFinite {
            context: "frequency contraction inputs",
        });
    }
    MeasuredFrequency::new(contract_covector_vector(p_backward, observer_velocity))
}

/// `ν = -k_future_μ u^μ` (must be finite and > 0).
pub fn measured_frequency_from_future_covector(
    k_future: &Covector,
    observer_velocity: &Vector,
) -> Result<MeasuredFrequency, CoreError> {
    if !k_future.is_finite() || !observer_velocity.is_finite() {
        return Err(CoreError::NonFinite {
            context: "frequency contraction inputs",
        });
    }
    MeasuredFrequency::new(-contract_covector_vector(k_future, observer_velocity))
}

/// `g = ν_obs / ν_em`.
pub fn frequency_shift_ratio(
    observer: MeasuredFrequency,
    emitter: MeasuredFrequency,
) -> Result<FrequencyShift, CoreError> {
    FrequencyShift::new(observer.value() / emitter.value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn static_u() -> Vector {
        Vector::new(1.0, 0.0, 0.0, 0.0)
    }

    #[test]
    fn same_observer_emitter_g_unity() {
        let p = Covector::new(1.0, -1.0, 0.0, 0.0);
        let u = static_u();
        let nu = measured_frequency_from_backward_covector(&p, &u).unwrap();
        let g = frequency_shift_ratio(nu, nu).unwrap();
        assert_eq!(g.value(), 1.0);
    }

    #[test]
    fn past_future_orientation_equivalence() {
        let p_back = Covector::new(1.0, -0.3, 0.2, -0.1);
        let k_fut = p_back.scale(-1.0);
        let u = Vector::new(1.1, 0.05, -0.02, 0.01);
        let a = measured_frequency_from_backward_covector(&p_back, &u).unwrap();
        let b = measured_frequency_from_future_covector(&k_fut, &u).unwrap();
        assert_relative_eq!(a.value(), b.value(), epsilon = 0.0, max_relative = 1e-15);
    }

    #[test]
    fn positive_scaling_leaves_g_invariant() {
        let p = Covector::new(1.0, -1.0, 0.0, 0.0);
        let u_obs = static_u();
        let u_em = Vector::new(1.2, 0.1, 0.0, 0.0);
        let g0 = frequency_shift_ratio(
            measured_frequency_from_backward_covector(&p, &u_obs).unwrap(),
            measured_frequency_from_backward_covector(&p, &u_em).unwrap(),
        )
        .unwrap();
        for c in [0.25, 2.0, 7.5] {
            let pc = p.scale(c);
            let g = frequency_shift_ratio(
                measured_frequency_from_backward_covector(&pc, &u_obs).unwrap(),
                measured_frequency_from_backward_covector(&pc, &u_em).unwrap(),
            )
            .unwrap();
            assert_relative_eq!(g.value(), g0.value(), epsilon = 0.0, max_relative = 1e-14);
        }
    }

    #[test]
    fn minkowski_sr_doppler_corpus() {
        // p_backward = (1,-1,0,0), u_obs = (1,0,0,0), u_em = γ(1,β,0,0)
        // g = 1/[γ(1-β)] = sqrt((1+β)/(1-β))
        let p = Covector::new(1.0, -1.0, 0.0, 0.0);
        let u_obs = static_u();
        for beta in [-0.5_f64, -0.1, 0.0, 0.1, 0.5] {
            let gamma = 1.0 / (1.0 - beta * beta).sqrt();
            let u_em = Vector::new(gamma, gamma * beta, 0.0, 0.0);
            let nu_obs = measured_frequency_from_backward_covector(&p, &u_obs).unwrap();
            let nu_em = measured_frequency_from_backward_covector(&p, &u_em).unwrap();
            let g = frequency_shift_ratio(nu_obs, nu_em).unwrap().value();
            let expected = ((1.0 + beta) / (1.0 - beta)).sqrt();
            assert_relative_eq!(g, expected, epsilon = 1e-12);
            assert_relative_eq!(nu_obs.value(), 1.0, epsilon = 1e-15);
            assert_relative_eq!(nu_em.value(), gamma * (1.0 - beta), epsilon = 1e-12);
        }
    }

    #[test]
    fn rejects_non_positive_frequency() {
        let u = static_u();
        assert!(MeasuredFrequency::new(0.0).is_err());
        assert!(MeasuredFrequency::new(-1.0).is_err());
        assert!(MeasuredFrequency::new(f64::NAN).is_err());
        assert!(FrequencyShift::new(0.0).is_err());
        // Past covector with wrong sign relative to u → non-positive ν.
        let p = Covector::new(-1.0, 0.0, 0.0, 0.0);
        assert!(measured_frequency_from_backward_covector(&p, &u).is_err());
    }

    #[test]
    fn backward_frequency_sign_is_positive_for_camera_like() {
        // Camera-local past null lowered against static observer → ν = 1.
        let p = Covector::new(1.0, -1.0, 0.0, 0.0);
        let nu = measured_frequency_from_backward_covector(&p, &static_u()).unwrap();
        assert!(nu.value() > 0.0);
        assert_relative_eq!(nu.value(), 1.0, epsilon = 1e-15);
    }

    #[test]
    fn bl_ks_contraction_invariance_equatorial_corpus() {
        use crate::{
            circular_equatorial_geodesic_bl, covector_bl_to_ks, prograde_equatorial_direction,
            vector_bl_to_ks, KerrParams, PositionBl,
        };
        let spins = [0.0_f64, 0.5, 0.999, -0.5];
        let radii = [6.0_f64, 10.0, 20.0];
        let phis = [0.0_f64, 0.7, -1.1];
        for &a in &spins {
            let params = KerrParams::new(1.0, a).unwrap();
            let dir = prograde_equatorial_direction(&params);
            for &r in &radii {
                let Ok(orbit) = circular_equatorial_geodesic_bl(&params, r, dir) else {
                    continue;
                };
                for &phi in &phis {
                    let bl = PositionBl::new(0.0, r, std::f64::consts::FRAC_PI_2, phi);
                    // Past-directed sample covector with nonzero p_t, p_φ.
                    let p_bl = Covector::new(1.0, 0.1, 0.0, -0.4);
                    let p_ks = covector_bl_to_ks(&params, &bl, &p_bl).unwrap();
                    let u_ks = vector_bl_to_ks(&params, &bl, &orbit.four_velocity_bl).unwrap();
                    let nu_bl = contract_covector_vector(&p_bl, &orbit.four_velocity_bl);
                    let nu_ks = contract_covector_vector(&p_ks, &u_ks);
                    assert!(
                        (nu_bl - nu_ks).abs() < 1e-10,
                        "BL/KS ν mismatch a={a} r={r} φ={phi}: {nu_bl} vs {nu_ks}"
                    );
                }
            }
        }
    }
}
