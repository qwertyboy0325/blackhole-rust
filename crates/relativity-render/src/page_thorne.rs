//! Page–Thorne (1974) one-face thin-disk flux + Newtonian asymptotic oracle.
//!
//! Production path: closed-form algebraic `Q(x)` from Page & Thorne / Novikov–Thorne.
//! Independent oracle: numerical quadrature of the conservation-law integrand.
//!
//! `F` is energy per proper time per proper area from **one face** (PT74).
//! Zero torque at prograde ISCO. Gate 2C0 rejects retrograde spin.
//!
//! Do not copy GPL GRRT implementations; formulas re-derived from primary paper.

use crate::error::BolometricRenderError;
use relativity_core::{
    prograde_isco_radius, FluxWPerM2, KerrParams, MassKg, MdotKgPerS, PhysicalScale,
    GRAVITATIONAL_G_M3_KG_S2, SPEED_OF_LIGHT_M_S,
};
use serde::{Deserialize, Serialize};

pub const FLUX_MODEL_ID: &str = "page-thorne-zero-torque-v1";
pub const FACE_POLICY: &str = "one-face-upper-equals-lower-by-pt74-symmetry";
pub const NEWTONIAN_ORACLE_ID: &str = "newtonian-zero-torque-asymptotic-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThinDiskFluxModel {
    PageThorneZeroTorqueV1,
}

impl ThinDiskFluxModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PageThorneZeroTorqueV1 => FLUX_MODEL_ID,
        }
    }
}

/// Dimensionless Page–Thorne roots of `x³ − 3x + 2 a* = 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageThorneRoots {
    pub x0: f64,
    pub x1: f64,
    pub x2: f64,
    pub x3: f64,
    pub a_star: f64,
}

impl PageThorneRoots {
    pub fn for_prograde(params: &KerrParams) -> Result<Self, BolometricRenderError> {
        let a_star = params.spin_over_mass();
        if !(a_star >= 0.0) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "page-thorne-zero-torque-v1 rejects retrograde a*/M < 0".into(),
            ));
        }
        if a_star > 1.0 || !a_star.is_finite() {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "invalid a* for Page-Thorne roots".into(),
            ));
        }
        let r_isco = prograde_isco_radius(params).map_err(|e| {
            BolometricRenderError::InvalidEmissionSpec(format!("prograde ISCO: {e}"))
        })?;
        let x0 = (r_isco / params.mass()).sqrt();
        // Page & Thorne (1974): x1,x2,x3 roots via trigonometric solution.
        let theta = a_star.clamp(-1.0, 1.0).acos();
        let x1 = 2.0 * (theta / 3.0 - std::f64::consts::FRAC_PI_3).cos();
        let x2 = 2.0 * (theta / 3.0 + std::f64::consts::FRAC_PI_3).cos();
        let x3 = -2.0 * (theta / 3.0).cos();
        if ![x0, x1, x2, x3].iter().all(|v| v.is_finite()) {
            return Err(BolometricRenderError::InvalidEmissionSpec(
                "Page-Thorne roots non-finite".into(),
            ));
        }
        Ok(Self {
            x0,
            x1,
            x2,
            x3,
            a_star,
        })
    }
}

#[inline]
fn metric_b(x: f64, a_star: f64) -> f64 {
    1.0 + a_star / (x * x * x)
}

#[inline]
fn metric_c(x: f64, a_star: f64) -> f64 {
    // Page & Thorne: C = 1 − 3/x² + 2 a*/x³  (linear in a*, not a*²).
    1.0 - 3.0 / (x * x) + 2.0 * a_star / (x * x * x)
}

fn log_term(
    x: f64,
    x0: f64,
    xi: f64,
    a_star: f64,
    xj: f64,
    xk: f64,
) -> Result<f64, BolometricRenderError> {
    // Coefficient vanishes as xi → 0 (Schwarzschild x2); skip singular log.
    if !xi.is_finite() || xi.abs() < 1e-14 {
        return Ok(0.0);
    }
    let denom = xi * (xi - xj) * (xi - xk);
    if !denom.is_finite() || denom.abs() < 1e-30 {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "Page-Thorne log-term denominator unresolved".into(),
        ));
    }
    let num = 3.0 * (xi - a_star) * (xi - a_star);
    let arg_num = x - xi;
    let arg_den = x0 - xi;
    if !(arg_num > 0.0) || !(arg_den.abs() > 0.0) || arg_num / arg_den <= 0.0 {
        // At x→x0⁺, each ln → 0; outside domain reject.
        if (x - x0).abs() <= 1e-14 * (1.0 + x0) {
            return Ok(0.0);
        }
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "Page-Thorne log argument non-positive".into(),
        ));
    }
    Ok((num / denom) * (arg_num / arg_den).ln())
}

/// Dimensionless Page–Thorne factor `Q(x)` with `Q → 1 − x₀/x` at large `x` (a*=0).
///
/// ```text
/// Q = B C^{-1/2} x^{-1} [ x − x₀ − (3/2) a* ln(x/x₀)
///     − Σ_i 3(x_i−a*)²/(x_i (x_i−x_j)(x_i−x_k)) ln((x−x_i)/(x₀−x_i)) ]
/// ```
pub fn page_thorne_q(x: f64, roots: &PageThorneRoots) -> Result<f64, BolometricRenderError> {
    if !x.is_finite() || !(x > roots.x0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "Page-Thorne Q requires x > x0 (strictly outside ISCO)".into(),
        ));
    }
    let a = roots.a_star;
    let b = metric_b(x, a);
    let c = metric_c(x, a);
    if !(c > 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "Page-Thorne C <= 0 (orbit not circular/timelike)".into(),
        ));
    }
    let bracket = x
        - roots.x0
        - 1.5 * a * (x / roots.x0).ln()
        - log_term(x, roots.x0, roots.x1, a, roots.x2, roots.x3)?
        - log_term(x, roots.x0, roots.x2, a, roots.x1, roots.x3)?
        - log_term(x, roots.x0, roots.x3, a, roots.x1, roots.x2)?;
    let q = b / c.sqrt() * (1.0 / x) * bracket;
    if !q.is_finite() || q < 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "Page-Thorne Q non-finite or negative".into(),
        ));
    }
    Ok(q)
}

/// One-face Page–Thorne flux [W m⁻²] at geometrized radius `r_over_m` (units of `M`).
///
/// With [`page_thorne_q`] (PT74 `Q` including `B C^{-1/2} x^{-1}`):
/// ```text
/// F = (3 c⁶ Ṁ) / (8 π G² M²) · Q / (B √C x⁶)
///   = (3 G M Ṁ) / (8 π r_phys³) · Q / (B √C)
/// ```
/// The `1/(B √C)` factor is mandatory for this `Q` convention (vertical comoving
/// one-face flux). Omitting it is a scientific-model error.
pub fn page_thorne_one_face_flux(
    scale: &PhysicalScale,
    mdot: MdotKgPerS,
    params: &KerrParams,
    r_over_m: f64,
) -> Result<FluxWPerM2, BolometricRenderError> {
    if mdot.value() == 0.0 {
        return FluxWPerM2::new(0.0)
            .map_err(|e| BolometricRenderError::InvalidIntensity(format!("zero mdot flux: {e}")));
    }
    if !r_over_m.is_finite() || !(r_over_m > 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "r/M must be finite and > 0".into(),
        ));
    }
    // Mass consistency: PhysicalScale mass vs Kerr geometrized mass unit.
    let _ = MassKg::new(scale.mass_kg.value())
        .map_err(|e| BolometricRenderError::InvalidEmissionSpec(format!("physical mass: {e}")))?;
    let roots = PageThorneRoots::for_prograde(params)?;
    let x = r_over_m.sqrt();
    let a = roots.a_star;
    let b = metric_b(x, a);
    let c_metric = metric_c(x, a);
    if !(c_metric > 0.0) || !(b > 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "Page-Thorne B or C non-positive at evaluation radius".into(),
        ));
    }
    let q = page_thorne_q(x, &roots)?;
    let m_kg = scale.mass_kg.value();
    let c = SPEED_OF_LIGHT_M_S;
    let g = GRAVITATIONAL_G_M3_KG_S2;
    let c2 = c * c;
    let c6 = c2 * c2 * c2;
    let g2 = g * g;
    let pre = 3.0 * c6 * mdot.value() / (8.0 * std::f64::consts::PI * g2 * m_kg * m_kg);
    let x2 = x * x;
    let x6 = x2 * x2 * x2;
    let f = pre * q / (b * c_metric.sqrt() * x6);
    if !f.is_finite() || f < 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "Page-Thorne flux non-finite or negative".into(),
        ));
    }
    FluxWPerM2::new(f).map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))
}

/// Newtonian zero-torque one-face flux with inner edge at prograde ISCO (oracle B).
///
/// ```text
/// F_N = (3 G M Ṁ)/(8 π r_phys³) (1 − √(r_isco / r))
/// ```
pub fn newtonian_zero_torque_flux(
    scale: &PhysicalScale,
    mdot: MdotKgPerS,
    params: &KerrParams,
    r_over_m: f64,
) -> Result<FluxWPerM2, BolometricRenderError> {
    if mdot.value() == 0.0 {
        return FluxWPerM2::new(0.0).map_err(|e| {
            BolometricRenderError::InvalidIntensity(format!("zero mdot newtonian: {e}"))
        });
    }
    let r_isco = prograde_isco_radius(params)
        .map_err(|e| BolometricRenderError::InvalidEmissionSpec(format!("newtonian ISCO: {e}")))?;
    let r_isco_over_m = r_isco / params.mass();
    if !(r_over_m > r_isco_over_m) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "newtonian flux requires r > r_isco".into(),
        ));
    }
    let rg = scale.gravitational_radius_m;
    let r_phys = rg * r_over_m;
    let m_kg = scale.mass_kg.value();
    let g = GRAVITATIONAL_G_M3_KG_S2;
    let factor = 1.0 - (r_isco_over_m / r_over_m).sqrt();
    let f = 3.0 * g * m_kg * mdot.value() / (8.0 * std::f64::consts::PI * r_phys * r_phys * r_phys)
        * factor;
    FluxWPerM2::new(f).map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))
}

// --- Independent numerical flux oracle (conservation law; not algebraic Q) ---

#[inline]
fn omega_m(r_over_m: f64, a_star: f64) -> f64 {
    // Dimensionless Ω̃ = Ω M = 1 / (x³ + a*), x² = r/M.
    let x = r_over_m.sqrt();
    1.0 / (x * x * x + a_star)
}

#[inline]
fn specific_energy(r_over_m: f64, a_star: f64) -> Result<f64, BolometricRenderError> {
    let x = r_over_m.sqrt();
    let c = metric_c(x, a_star);
    if !(c > 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "circular energy C <= 0".into(),
        ));
    }
    // E = (1 − 2/x² + a*/x³) / √C
    let num = 1.0 - 2.0 / (x * x) + a_star / (x * x * x);
    Ok(num / c.sqrt())
}

#[inline]
fn specific_angular_momentum_over_m(
    r_over_m: f64,
    a_star: f64,
) -> Result<f64, BolometricRenderError> {
    let x = r_over_m.sqrt();
    let c = metric_c(x, a_star);
    if !(c > 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "circular L C <= 0".into(),
        ));
    }
    // L/M = x (1 − 2 a*/x³ + a*²/x⁴) / √C   [Page–Thorne / NT]
    let f = 1.0 - 2.0 * a_star / (x * x * x) + (a_star * a_star) / (x * x * x * x);
    Ok(x * f / c.sqrt())
}

/// Independent numerical one-face Page–Thorne flux from the conservation law.
///
/// Geometrized (`M=1`) / continuum-fitting form (e.g. Penna+2012 B.11, C=0):
/// ```text
/// F̃ = (−Ω̃_,r̃) / (4 π r̃ (E−ΩL)²) · ∫_{r̃_ms}^{r̃} (E−ΩL) L̃_,r̃ dr̃
/// F_SI = (c⁶ Ṁ) / (G² M²) · F̃
/// ```
/// Uses `−Ω_,r` in the **numerator** (not its reciprocal) and compares **flux**
/// to the algebraic path — never a self-defined intermediate `Q`.
pub fn page_thorne_one_face_flux_numerical(
    scale: &PhysicalScale,
    mdot: MdotKgPerS,
    params: &KerrParams,
    r_over_m: f64,
    n_quad: usize,
) -> Result<FluxWPerM2, BolometricRenderError> {
    if mdot.value() == 0.0 {
        return FluxWPerM2::new(0.0).map_err(|e| {
            BolometricRenderError::InvalidIntensity(format!("zero mdot numerical: {e}"))
        });
    }
    let a_star = params.spin_over_mass();
    if !(a_star >= 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "numerical PT rejects retrograde".into(),
        ));
    }
    let r_isco = prograde_isco_radius(params)
        .map_err(|e| BolometricRenderError::InvalidEmissionSpec(format!("numerical ISCO: {e}")))?;
    let r0 = r_isco / params.mass();
    if !(r_over_m > r0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "numerical PT requires r > r_isco".into(),
        ));
    }
    let n = n_quad.max(256);
    let r_start = r0 * (1.0 + 1e-8);
    if !(r_over_m > r_start) {
        return FluxWPerM2::new(0.0).map_err(|e| {
            BolometricRenderError::InvalidIntensity(format!("near-isco numerical: {e}"))
        });
    }

    // ∫ (E−ΩL) dL = ∫ (E−ΩL) L_,r dr  (trapezoid in L along a radius path).
    let mut integral = 0.0;
    let e0 = specific_energy(r_start, a_star)?;
    let l0 = specific_angular_momentum_over_m(r_start, a_star)?;
    let omega0 = omega_m(r_start, a_star);
    let mut prev_el = e0 - omega0 * l0;
    let mut prev_l = l0;
    for i in 1..=n {
        let t = i as f64 / n as f64;
        let u = 0.5 * (1.0 - (std::f64::consts::PI * t).cos());
        let r = r_start + u * (r_over_m - r_start);
        let e = specific_energy(r, a_star)?;
        let l = specific_angular_momentum_over_m(r, a_star)?;
        let omega = omega_m(r, a_star);
        let el = e - omega * l;
        let dl = l - prev_l;
        integral += 0.5 * (el + prev_el) * dl;
        prev_el = el;
        prev_l = l;
    }

    let e = specific_energy(r_over_m, a_star)?;
    let l = specific_angular_momentum_over_m(r_over_m, a_star)?;
    let omega = omega_m(r_over_m, a_star);
    let el = e - omega * l;
    if !(el > 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "E−ΩL non-positive at evaluation radius".into(),
        ));
    }
    let h = (1e-6_f64).max(1e-8 * r_over_m);
    let om_p = omega_m(r_over_m + h, a_star);
    let om_m = omega_m((r_over_m - h).max(r_start), a_star);
    let domega_dr = (om_p - om_m) / (2.0 * h);
    if !domega_dr.is_finite() || !(domega_dr < 0.0) {
        return Err(BolometricRenderError::InvalidEmissionSpec(
            "dΩ̃/dr̃ must be finite and < 0".into(),
        ));
    }
    // F̃ = (−Ω̃_,r̃) / (4π r̃ (E−ΩL)²) · Ĩ
    let f_tilde = (-domega_dr) / (4.0 * std::f64::consts::PI * r_over_m * el * el) * integral;
    if !f_tilde.is_finite() || f_tilde < 0.0 {
        return Err(BolometricRenderError::InvalidIntensity(
            "numerical Page-Thorne F̃ non-finite or negative".into(),
        ));
    }
    let m_kg = scale.mass_kg.value();
    let c = SPEED_OF_LIGHT_M_S;
    let g = GRAVITATIONAL_G_M3_KG_S2;
    let c2 = c * c;
    let c6 = c2 * c2 * c2;
    let g2 = g * g;
    let f = (c6 / (g2 * m_kg * m_kg)) * mdot.value() * f_tilde;
    FluxWPerM2::new(f).map_err(|e| BolometricRenderError::InvalidIntensity(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_core::PhysicalScale;

    fn scale_1e8() -> PhysicalScale {
        PhysicalScale::from_solar_masses(1.0e8).unwrap()
    }

    #[test]
    fn q_vanishes_at_isco_limit() {
        let k = KerrParams::new(1.0, 0.0).unwrap();
        let roots = PageThorneRoots::for_prograde(&k).unwrap();
        let q = page_thorne_q(roots.x0 * (1.0 + 1e-6), &roots).unwrap();
        assert!(q < 1e-5, "Q near ISCO should be tiny, got {q}");
    }

    #[test]
    fn large_r_matches_newtonian_schwarzschild() {
        let k = KerrParams::new(1.0, 0.0).unwrap();
        let scale = scale_1e8();
        let mdot = MdotKgPerS::new(1.0e14).unwrap();
        // GR corrections decay slowly; compare in the deep asymptotic.
        let r = 100_000.0;
        let f_pt = page_thorne_one_face_flux(&scale, mdot, &k, r)
            .unwrap()
            .value();
        let f_n = newtonian_zero_torque_flux(&scale, mdot, &k, r)
            .unwrap()
            .value();
        let rel = (f_pt - f_n).abs() / f_n;
        assert!(rel < 5e-3, "large-r rel err {rel}: pt={f_pt} newt={f_n}");
    }

    #[test]
    fn algebraic_vs_numerical_flux_domain() {
        let scale = scale_1e8();
        let mdot = MdotKgPerS::new(1.0e14).unwrap();
        // Cover Schwarzschild / moderate / high-spin and near-ISCO / mid / outer.
        let cases: &[(f64, &[f64])] = &[
            (0.0, &[8.0, 20.0, 200.0]),
            (0.5, &[6.0, 20.0, 200.0]),
            (0.999, &[1.5, 3.0, 20.0, 200.0]),
        ];
        let mut worst = 0.0_f64;
        for &(a, radii) in cases {
            let k = KerrParams::new(1.0, a).unwrap();
            let r_isco = prograde_isco_radius(&k).unwrap() / 1.0;
            for &r in radii {
                if !(r > r_isco) {
                    continue;
                }
                let f_a = page_thorne_one_face_flux(&scale, mdot, &k, r)
                    .unwrap()
                    .value();
                let f_n = page_thorne_one_face_flux_numerical(&scale, mdot, &k, r, 16_384)
                    .unwrap()
                    .value();
                let rel = (f_a - f_n).abs() / f_a.max(f_n).max(1e-30);
                worst = worst.max(rel);
                assert!(
                    rel < 5e-3,
                    "a={a} r={r}: algebraic vs numerical flux rel {rel}: a={f_a} n={f_n}"
                );
            }
        }
        assert!(worst.is_finite());
    }

    #[test]
    fn numerical_quad_converges_high_spin() {
        let k = KerrParams::new(1.0, 0.999).unwrap();
        let scale = scale_1e8();
        let mdot = MdotKgPerS::new(1.0e14).unwrap();
        let r = 3.0;
        let f_lo = page_thorne_one_face_flux_numerical(&scale, mdot, &k, r, 1024)
            .unwrap()
            .value();
        let f_hi = page_thorne_one_face_flux_numerical(&scale, mdot, &k, r, 16_384)
            .unwrap()
            .value();
        let rel = (f_lo - f_hi).abs() / f_hi.max(1e-30);
        assert!(
            rel < 1e-2,
            "quad convergence rel {rel}: lo={f_lo} hi={f_hi}"
        );
    }

    #[test]
    fn mdot_zero_gives_zero_flux() {
        let k = KerrParams::new(1.0, 0.999).unwrap();
        let scale = scale_1e8();
        let mdot = MdotKgPerS::new(0.0).unwrap();
        let f = page_thorne_one_face_flux(&scale, mdot, &k, 5.0)
            .unwrap()
            .value();
        assert_eq!(f, 0.0);
    }

    #[test]
    fn retrograde_rejected() {
        let k = KerrParams::new(1.0, -0.3).unwrap();
        assert!(PageThorneRoots::for_prograde(&k).is_err());
    }

    #[test]
    fn high_spin_finite_outside_isco() {
        let k = KerrParams::new(1.0, 0.999).unwrap();
        let scale = scale_1e8();
        let mdot = MdotKgPerS::new(1.0e14).unwrap();
        let r_isco = prograde_isco_radius(&k).unwrap();
        let f = page_thorne_one_face_flux(&scale, mdot, &k, r_isco * 1.05 + 0.01)
            .unwrap()
            .value();
        assert!(f.is_finite() && f > 0.0);
    }

    #[test]
    fn missing_b_sqrt_c_would_disagree_high_spin() {
        // Regression: F∝Q (without /(B√C)) is wrong; near-ISCO high-spin C≪1.
        let k = KerrParams::new(1.0, 0.999).unwrap();
        let scale = scale_1e8();
        let mdot = MdotKgPerS::new(1.0e14).unwrap();
        let r = 2.0_f64;
        let roots = PageThorneRoots::for_prograde(&k).unwrap();
        let x = r.sqrt();
        let q = page_thorne_q(x, &roots).unwrap();
        let b = 1.0 + roots.a_star / (x * x * x);
        let c = 1.0 - 3.0 / (x * x) + 2.0 * roots.a_star / (x * x * x);
        let f_correct = page_thorne_one_face_flux(&scale, mdot, &k, r)
            .unwrap()
            .value();
        let m_kg = scale.mass_kg.value();
        let c_light = SPEED_OF_LIGHT_M_S;
        let g = GRAVITATIONAL_G_M3_KG_S2;
        let c2 = c_light * c_light;
        let c6 = c2 * c2 * c2;
        let pre = 3.0 * c6 * mdot.value() / (8.0 * std::f64::consts::PI * g * g * m_kg * m_kg);
        let x6 = x * x * x * x * x * x;
        let f_wrong = pre * q / x6;
        let ratio = f_wrong / f_correct;
        let expect = b * c.sqrt();
        assert!(
            (ratio - expect).abs() / expect < 1e-10,
            "wrong/correct should be B√C={expect}, got {ratio}"
        );
        // Near-ISCO high spin: C≪1 ⇒ B√C≪1 ⇒ omitting the factor overstates F.
        assert!(
            expect < 0.95,
            "high-spin near-ISCO must make B√C materially < 1, got {expect}"
        );
    }
}
