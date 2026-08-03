//! Explicit spacetime algebra types (no bare `[f64; 4]` across public APIs).

use crate::error::CoreError;

/// Cartesian Kerr–Schild event `(t, x, y, z)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionKs {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl PositionKs {
    #[must_use]
    pub const fn new(t: f64, x: f64, y: f64, z: f64) -> Self {
        Self { t, x, y, z }
    }

    #[must_use]
    pub const fn spatial(x: f64, y: f64, z: f64) -> Self {
        Self::new(0.0, x, y, z)
    }

    pub fn require_finite(&self, context: &'static str) -> Result<(), CoreError> {
        if self.t.is_finite() && self.x.is_finite() && self.y.is_finite() && self.z.is_finite() {
            Ok(())
        } else {
            Err(CoreError::NonFinite { context })
        }
    }

    #[must_use]
    pub fn components(&self) -> [f64; 4] {
        [self.t, self.x, self.y, self.z]
    }
}

/// Boyer–Lindquist event `(t, r, θ, φ)` with `θ, φ` in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionBl {
    pub t: f64,
    pub r: f64,
    pub theta: f64,
    pub phi: f64,
}

impl PositionBl {
    #[must_use]
    pub const fn new(t: f64, r: f64, theta: f64, phi: f64) -> Self {
        Self { t, r, theta, phi }
    }

    pub fn require_finite(&self, context: &'static str) -> Result<(), CoreError> {
        if self.t.is_finite()
            && self.r.is_finite()
            && self.theta.is_finite()
            && self.phi.is_finite()
        {
            Ok(())
        } else {
            Err(CoreError::NonFinite { context })
        }
    }
}

/// Contravariant four-vector `v^μ`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector {
    #[must_use]
    pub const fn new(t: f64, x: f64, y: f64, z: f64) -> Self {
        Self { t, x, y, z }
    }

    #[must_use]
    pub fn components(&self) -> [f64; 4] {
        [self.t, self.x, self.y, self.z]
    }

    #[must_use]
    pub fn from_components(c: [f64; 4]) -> Self {
        Self::new(c[0], c[1], c[2], c[3])
    }

    #[must_use]
    pub fn scale(self, s: f64) -> Self {
        Self::new(self.t * s, self.x * s, self.y * s, self.z * s)
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.t.is_finite() && self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// Covariant four-momentum / covector `p_μ`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Covector {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Covector {
    #[must_use]
    pub const fn new(t: f64, x: f64, y: f64, z: f64) -> Self {
        Self { t, x, y, z }
    }

    #[must_use]
    pub fn components(&self) -> [f64; 4] {
        [self.t, self.x, self.y, self.z]
    }

    #[must_use]
    pub fn from_components(c: [f64; 4]) -> Self {
        Self::new(c[0], c[1], c[2], c[3])
    }

    #[must_use]
    pub fn scale(self, s: f64) -> Self {
        Self::new(self.t * s, self.x * s, self.y * s, self.z * s)
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.t.is_finite() && self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// Local orthonormal frame components `v^(a)` with `a = 0..3`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalComponents {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl LocalComponents {
    #[must_use]
    pub const fn new(t: f64, x: f64, y: f64, z: f64) -> Self {
        Self { t, x, y, z }
    }

    #[must_use]
    pub fn components(&self) -> [f64; 4] {
        [self.t, self.x, self.y, self.z]
    }
}

/// Symmetric `(0,2)` tensor stored as a full 4×4 (symmetry enforced by constructors).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricTensor {
    data: [[f64; 4]; 4],
}

impl MetricTensor {
    #[must_use]
    pub fn from_symmetric(lower: [[f64; 4]; 4]) -> Self {
        let mut data = lower;
        for i in 0..4 {
            for j in (i + 1)..4 {
                let avg = 0.5 * (data[i][j] + data[j][i]);
                data[i][j] = avg;
                data[j][i] = avg;
            }
        }
        Self { data }
    }

    #[must_use]
    pub fn minkowski() -> Self {
        Self {
            data: [
                [-1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    #[must_use]
    pub fn get(&self, mu: usize, nu: usize) -> f64 {
        self.data[mu][nu]
    }

    #[must_use]
    pub fn components(&self) -> [[f64; 4]; 4] {
        self.data
    }

    #[must_use]
    pub fn mul_vec(&self, v: &Vector) -> Covector {
        let vc = v.components();
        let mut out = [0.0; 4];
        for mu in 0..4 {
            let mut s = 0.0;
            for nu in 0..4 {
                s += self.data[mu][nu] * vc[nu];
            }
            out[mu] = s;
        }
        Covector::from_components(out)
    }

    #[must_use]
    pub fn raise(&self, p: &Covector) -> Vector {
        // Caller must pass inverse metric.
        let pc = p.components();
        let mut out = [0.0; 4];
        for mu in 0..4 {
            let mut s = 0.0;
            for nu in 0..4 {
                s += self.data[mu][nu] * pc[nu];
            }
            out[mu] = s;
        }
        Vector::from_components(out)
    }

    #[must_use]
    pub fn contract(&self, a: &Vector, b: &Vector) -> f64 {
        let ac = a.components();
        let bc = b.components();
        let mut s = 0.0;
        for mu in 0..4 {
            for nu in 0..4 {
                s += self.data[mu][nu] * ac[mu] * bc[nu];
            }
        }
        s
    }

    #[must_use]
    pub fn contract_cov(&self, a: &Covector, b: &Covector) -> f64 {
        let ac = a.components();
        let bc = b.components();
        let mut s = 0.0;
        for mu in 0..4 {
            for nu in 0..4 {
                s += self.data[mu][nu] * ac[mu] * bc[nu];
            }
        }
        s
    }

    /// Matrix product `self * other` (not index-raised).
    #[must_use]
    pub fn matmul(&self, other: &Self) -> [[f64; 4]; 4] {
        let mut out = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += self.data[i][k] * other.data[k][j];
                }
                out[i][j] = s;
            }
        }
        out
    }

    #[must_use]
    pub fn max_abs_asymmetry(&self) -> f64 {
        let mut m: f64 = 0.0;
        for i in 0..4 {
            for j in (i + 1)..4 {
                m = m.max((self.data[i][j] - self.data[j][i]).abs());
            }
        }
        m
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.data.iter().flatten().all(|v| v.is_finite())
    }
}

/// Identity residual of `g * g^{-1}`.
#[must_use]
pub fn identity_residual(g: &MetricTensor, g_inv: &MetricTensor) -> f64 {
    let prod = g.matmul(g_inv);
    let mut max: f64 = 0.0;
    for i in 0..4 {
        for j in 0..4 {
            let target = if i == j { 1.0 } else { 0.0 };
            max = max.max((prod[i][j] - target).abs());
        }
    }
    max
}
