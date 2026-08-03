//! Analytic toy ODE systems for experiments A–F.

use std::f64::consts::PI;

pub const EXP_LAMBDA: f64 = -0.7;
pub const EXP_X0: f64 = 0.0;
pub const EXP_X_END: f64 = 2.5;
pub const EXP_Y0: f64 = 1.0;

pub fn exp_analytic(x: f64) -> f64 {
    (EXP_LAMBDA * x).exp() * EXP_Y0
}

pub fn exp_derivative(_x: f64, y: f64) -> f64 {
    EXP_LAMBDA * y
}

pub const SHO_X0: f64 = 0.0;
pub const SHO_X_END: f64 = 4.0 * PI;
pub const SHO_Q0: f64 = 1.0;
pub const SHO_P0: f64 = 0.0;

pub fn sho_analytic_q(x: f64) -> f64 {
    SHO_Q0 * x.cos()
}

pub fn sho_analytic_p(x: f64) -> f64 {
    -SHO_Q0 * x.sin()
}

pub fn sho_energy(q: f64, p: f64) -> f64 {
    0.5 * (q * q + p * p)
}

pub fn sho_analytic_energy() -> f64 {
    sho_energy(SHO_Q0, SHO_P0)
}

/// Eight-component analytic system with mixed scales.
/// y_i' = λ_i y_i, λ_i and y0_i differ by orders of magnitude.
pub const MIXED8_DIM: usize = 8;

pub fn mixed8_lambdas() -> [f64; MIXED8_DIM] {
    [-0.01, -0.1, -1.0, -10.0, 0.01, 0.1, 1.0, 10.0]
}

pub fn mixed8_y0() -> [f64; MIXED8_DIM] {
    [1e6, 1e3, 1.0, 1e-3, 1e6, 1e3, 1.0, 1e-3]
}

pub fn mixed8_analytic(x: f64) -> [f64; MIXED8_DIM] {
    let mut y = mixed8_y0();
    let lambdas = mixed8_lambdas();
    for i in 0..MIXED8_DIM {
        y[i] *= (lambdas[i] * x).exp();
    }
    y
}

pub fn mixed8_derivative(_x: f64, y: &[f64], dy: &mut [f64]) {
    let lambdas = mixed8_lambdas();
    for i in 0..MIXED8_DIM {
        dy[i] = lambdas[i] * y[i];
    }
}

/// Event: q crosses zero at x = π/2 for SHO from (1,0).
pub const SHO_EVENT_X: f64 = PI / 2.0;

/// Shallow crossing: q = cos(x) - 0.01, root near x ≈ 0.14
pub fn shallow_event_fn(x: f64) -> f64 {
    x.cos() - 0.01
}

pub fn shallow_event_root_analytic() -> f64 {
    0.01_f64.acos()
}

/// Toy domain: valid for x in [0, 1.5)
pub const DOMAIN_X_MAX: f64 = 1.5;

pub fn domain_event(x: f64) -> f64 {
    DOMAIN_X_MAX - x
}
