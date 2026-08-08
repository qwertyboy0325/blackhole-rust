//! Planck `B_ν(T)` and Stefan–Boltzmann closures (Gate 2C0).
//!
//! ```text
//! T_eff = (F_one_face / σ_SB)^{1/4}
//! I_ν,em = B_ν(ν, T_eff)
//! π ∫_0^∞ B_ν dν = σ_SB T⁴ = F_one_face
//! ```
//!
//! Factor π is mandatory (isotropic Lambert emitter: `F = π I`).

use crate::error::BolometricRenderError;
use relativity_core::{
    stefan_boltzmann_w_m2_k4, FluxWPerM2, PhysicalFrequencyHz, SpecificIntensityNu,
    TemperatureKelvin, BOLTZMANN_K_J_K, PLANCK_H_J_S, SPEED_OF_LIGHT_M_S,
};

pub const TEMPERATURE_MODEL_ID: &str = "stefan-boltzmann-teff-v1";
pub const PLANCK_MODEL_ID: &str = "planck-b-nu-v1";

/// Effective temperature from one-face flux.
pub fn teff_from_one_face_flux(
    flux: FluxWPerM2,
) -> Result<TemperatureKelvin, BolometricRenderError> {
    let f = flux.value();
    if f == 0.0 {
        return TemperatureKelvin::new(0.0)
            .map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()));
    }
    let sigma = stefan_boltzmann_w_m2_k4();
    let t = (f / sigma).powf(0.25);
    TemperatureKelvin::new(t).map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))
}

/// Planck function `B_ν(ν,T)` [W m⁻² Hz⁻¹ sr⁻¹].
///
/// Uses `expm1` in the mid-range; Rayleigh–Jeans / Wien asymptotes at extremes.
/// `T = 0 ⇒ B = 0`.
pub fn planck_b_nu(
    nu: PhysicalFrequencyHz,
    temperature: TemperatureKelvin,
) -> Result<SpecificIntensityNu, BolometricRenderError> {
    let t = temperature.value();
    if t == 0.0 {
        return SpecificIntensityNu::new(0.0)
            .map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()));
    }
    let nu_v = nu.value();
    let h = PLANCK_H_J_S;
    let c = SPEED_OF_LIGHT_M_S;
    let k = BOLTZMANN_K_J_K;
    let c2 = c * c;
    let nu2 = nu_v * nu_v;
    let nu3 = nu2 * nu_v;
    let x = h * nu_v / (k * t);
    if !x.is_finite() {
        return Err(BolometricRenderError::InvalidIntensity(
            "Planck x = hν/kT non-finite".into(),
        ));
    }
    let pre = 2.0 * h * nu3 / c2;
    let b = if x < 1e-5 {
        // Rayleigh–Jeans: B ≈ 2 ν² k T / c²
        2.0 * nu2 * k * t / c2
    } else if x > 700.0 {
        // Wien: exp(x) overflows; B ≈ pre * e^{-x}
        pre * (-x).exp()
    } else {
        // B = pre / (e^x − 1) via expm1
        pre / f64::exp_m1(x)
    };
    if !b.is_finite() || b < 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "Planck B_nu non-finite or negative".into(),
        ));
    }
    SpecificIntensityNu::new(b).map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))
}

/// `B_λ` from `B_ν` with SI Jacobian `c/λ²`.
pub fn planck_b_lambda_from_b_nu(
    b_nu: SpecificIntensityNu,
    wavelength_m: f64,
) -> Result<f64, BolometricRenderError> {
    if !wavelength_m.is_finite() || !(wavelength_m > 0.0) {
        return Err(BolometricRenderError::InvalidIntensity(
            "wavelength for B_lambda must be finite and > 0".into(),
        ));
    }
    let c = SPEED_OF_LIGHT_M_S;
    let out = b_nu.value() * c / (wavelength_m * wavelength_m);
    if !out.is_finite() || out < 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "B_lambda non-finite".into(),
        ));
    }
    Ok(out)
}

/// Numerically integrate `π ∫ B_ν dν` on a log-frequency grid (closure helper).
pub fn integrate_pi_b_nu_log_grid(
    temperature: TemperatureKelvin,
    nu_min_hz: f64,
    nu_max_hz: f64,
    n_bins: u32,
) -> Result<f64, BolometricRenderError> {
    if temperature.value() == 0.0 {
        return Ok(0.0);
    }
    if !(nu_min_hz > 0.0) || !(nu_max_hz > nu_min_hz) || n_bins == 0 {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "invalid Planck integration grid".into(),
        ));
    }
    let ln_min = nu_min_hz.ln();
    let ln_max = nu_max_hz.ln();
    let mut acc = 0.0;
    for i in 0..n_bins {
        let t0 = i as f64 / n_bins as f64;
        let t1 = (i + 1) as f64 / n_bins as f64;
        let lo = (ln_min + t0 * (ln_max - ln_min)).exp();
        let hi = (ln_min + t1 * (ln_max - ln_min)).exp();
        let c = (lo * hi).sqrt();
        let w = hi - lo;
        let nu = PhysicalFrequencyHz::new(c)
            .map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))?;
        let b = planck_b_nu(nu, temperature)?.value();
        acc += b * w;
    }
    let out = std::f64::consts::PI * acc;
    if !out.is_finite() || out < 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "π∫B_nu non-finite".into(),
        ));
    }
    Ok(out)
}

/// Analytic Stefan–Boltzmann surface flux `σ T⁴` (equals `F_one_face` by definition of `T_eff`).
pub fn stefan_boltzmann_flux(
    temperature: TemperatureKelvin,
) -> Result<FluxWPerM2, BolometricRenderError> {
    let t = temperature.value();
    let sigma = stefan_boltzmann_w_m2_k4();
    let t2 = t * t;
    let f = sigma * t2 * t2;
    FluxWPerM2::new(f).map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teff_roundtrip_flux() {
        let f = FluxWPerM2::new(1.0e6).unwrap();
        let t = teff_from_one_face_flux(f).unwrap();
        let back = stefan_boltzmann_flux(t).unwrap().value();
        assert!((back - f.value()).abs() / f.value() < 1e-12);
    }

    #[test]
    fn pi_b_nu_matches_sigma_t4() {
        let t = TemperatureKelvin::new(1.0e4).unwrap();
        let sigma_t4 = stefan_boltzmann_flux(t).unwrap().value();
        // Wide log grid: 1e8 .. 1e18 Hz covers the peak for T=1e4 K.
        let integ = integrate_pi_b_nu_log_grid(t, 1.0e8, 1.0e18, 4096).unwrap();
        let rel = (integ - sigma_t4).abs() / sigma_t4;
        assert!(
            rel < 5e-3,
            "π∫B vs σT⁴ rel {rel}: integ={integ} sb={sigma_t4}"
        );
    }

    #[test]
    fn missing_pi_would_fail_closure() {
        let t = TemperatureKelvin::new(5.0e3).unwrap();
        let sigma_t4 = stefan_boltzmann_flux(t).unwrap().value();
        let integ_without_pi =
            integrate_pi_b_nu_log_grid(t, 1.0e8, 1.0e18, 2048).unwrap() / std::f64::consts::PI;
        let rel_wrong = (integ_without_pi - sigma_t4).abs() / sigma_t4;
        assert!(
            rel_wrong > 0.5,
            "dropping π must break SB closure; rel={rel_wrong}"
        );
    }

    #[test]
    fn zero_temperature_b_nu_zero() {
        let nu = PhysicalFrequencyHz::new(1.0e14).unwrap();
        let t = TemperatureKelvin::new(0.0).unwrap();
        assert_eq!(planck_b_nu(nu, t).unwrap().value(), 0.0);
    }

    #[test]
    fn rayleigh_jeans_asymptote() {
        let t = TemperatureKelvin::new(1.0e4).unwrap();
        let nu = PhysicalFrequencyHz::new(1.0e8).unwrap(); // x = hν/kT ≪ 1
        let b = planck_b_nu(nu, t).unwrap().value();
        let c = SPEED_OF_LIGHT_M_S;
        let expected = 2.0 * nu.value() * nu.value() * BOLTZMANN_K_J_K * t.value() / (c * c);
        assert!((b - expected).abs() / expected < 1e-4);
    }

    #[test]
    fn b_nu_b_lambda_jacobian_roundtrip_energy() {
        // Check I_λ = I_ν c/λ² at a point.
        let t = TemperatureKelvin::new(5800.0).unwrap();
        let lambda = 500e-9;
        let nu = PhysicalFrequencyHz::new(SPEED_OF_LIGHT_M_S / lambda).unwrap();
        let b_nu = planck_b_nu(nu, t).unwrap();
        let b_l = planck_b_lambda_from_b_nu(b_nu, lambda).unwrap();
        let back = b_l * lambda * lambda / SPEED_OF_LIGHT_M_S;
        assert!((back - b_nu.value()).abs() / b_nu.value() < 1e-12);
    }
}
