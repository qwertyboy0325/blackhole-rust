//! Scene-linear RGB spaces for Gate 2C1 scientific colorimetry.
//!
//! Canonical V1: Rec.709 / sRGB primaries, D65 white, linear (no OETF).
//! No chromatic adaptation (Bradford/CAT02). Negatives and HDR values allowed.

use crate::colorimetry::ColorimetricXyz;
use crate::error::ColorimetryError;
use relativity_trace::hex_sha;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCENE_LINEAR_RGB_SPACE_ID: &str = "scene-linear-rec709-d65-v1";
pub const RGB_MATRIX_REVISION: &str = "iec-61966-2-1-rec709-d65-linear-v1";

/// Chromaticities (x, y) — IEC 61966-2-1 / Rec. ITU-R BT.709.
const REC709_R_XY: (f64, f64) = (0.64, 0.33);
const REC709_G_XY: (f64, f64) = (0.30, 0.60);
const REC709_B_XY: (f64, f64) = (0.15, 0.06);
const D65_XY: (f64, f64) = (0.3127, 0.3290);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneLinearRgbSpace {
    Rec709D65LinearV1,
}

impl SceneLinearRgbSpace {
    pub fn id(self) -> &'static str {
        match self {
            Self::Rec709D65LinearV1 => SCENE_LINEAR_RGB_SPACE_ID,
        }
    }

    pub fn parse(id: &str) -> Result<Self, ColorimetryError> {
        if id == SCENE_LINEAR_RGB_SPACE_ID {
            Ok(Self::Rec709D65LinearV1)
        } else {
            Err(ColorimetryError::UnsupportedRgbSpace(id.into()))
        }
    }
}

/// Unclamped scene-linear RGB. Finite negatives and values > 1 are allowed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SceneLinearRgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl SceneLinearRgb {
    pub fn new(r: f64, g: f64, b: f64) -> Result<Self, ColorimetryError> {
        if !(r.is_finite() && g.is_finite() && b.is_finite()) {
            return Err(ColorimetryError::NonFinite("scene-linear RGB".into()));
        }
        Ok(Self { r, g, b })
    }

    pub fn negative_component_count(self) -> u32 {
        let mut n = 0u32;
        if self.r < 0.0 {
            n += 1;
        }
        if self.g < 0.0 {
            n += 1;
        }
        if self.b < 0.0 {
            n += 1;
        }
        n
    }
}

/// Row-major 3×3: `[rgb]^T = M · [xyz]^T`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XyzToRgbMatrix {
    pub m: [[f64; 3]; 3],
    pub space: SceneLinearRgbSpace,
    pub revision: &'static str,
}

impl XyzToRgbMatrix {
    pub fn rec709_d65_linear_v1() -> Self {
        let m = xyz_to_rgb_from_chromaticities(REC709_R_XY, REC709_G_XY, REC709_B_XY, D65_XY);
        Self {
            m,
            space: SceneLinearRgbSpace::Rec709D65LinearV1,
            revision: RGB_MATRIX_REVISION,
        }
    }

    pub fn apply(&self, xyz: ColorimetricXyz) -> Result<SceneLinearRgb, ColorimetryError> {
        let x = xyz.x;
        let y = xyz.y;
        let z = xyz.z;
        let r = self.m[0][0] * x + self.m[0][1] * y + self.m[0][2] * z;
        let g = self.m[1][0] * x + self.m[1][1] * y + self.m[1][2] * z;
        let b = self.m[2][0] * x + self.m[2][1] * y + self.m[2][2] * z;
        SceneLinearRgb::new(r, g, b)
    }

    pub fn invert_apply(&self, rgb: SceneLinearRgb) -> Result<ColorimetricXyz, ColorimetryError> {
        let inv = invert3(self.m).ok_or_else(|| {
            ColorimetryError::InvalidMatrix("XYZ→RGB matrix not invertible".into())
        })?;
        let r = rgb.r;
        let g = rgb.g;
        let b = rgb.b;
        let x = inv[0][0] * r + inv[0][1] * g + inv[0][2] * b;
        let y = inv[1][0] * r + inv[1][1] * g + inv[1][2] * b;
        let z = inv[2][0] * r + inv[2][1] * g + inv[2][2] * b;
        ColorimetricXyz::new(x, y, z)
    }

    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(b"xyz-to-rgb-matrix-v1");
        h.update(self.space.id().as_bytes());
        h.update(self.revision.as_bytes());
        for row in &self.m {
            for &v in row {
                h.update(v.to_bits().to_le_bytes());
            }
        }
        hex_sha(&h.finalize())
    }
}

/// Build XYZ→RGB from primary/white chromaticities (IEC construction).
fn xyz_to_rgb_from_chromaticities(
    r: (f64, f64),
    g: (f64, f64),
    b: (f64, f64),
    w: (f64, f64),
) -> [[f64; 3]; 3] {
    let xr = r.0 / r.1;
    let yr = 1.0;
    let zr = (1.0 - r.0 - r.1) / r.1;
    let xg = g.0 / g.1;
    let yg = 1.0;
    let zg = (1.0 - g.0 - g.1) / g.1;
    let xb = b.0 / b.1;
    let yb = 1.0;
    let zb = (1.0 - b.0 - b.1) / b.1;
    let p = [[xr, xg, xb], [yr, yg, yb], [zr, zg, zb]];
    let wx = w.0 / w.1;
    let wy = 1.0;
    let wz = (1.0 - w.0 - w.1) / w.1;
    let p_inv = invert3(p).expect("primary matrix invertible");
    let sr = p_inv[0][0] * wx + p_inv[0][1] * wy + p_inv[0][2] * wz;
    let sg = p_inv[1][0] * wx + p_inv[1][1] * wy + p_inv[1][2] * wz;
    let sb = p_inv[2][0] * wx + p_inv[2][1] * wy + p_inv[2][2] * wz;
    let m_rgb_to_xyz = [
        [sr * xr, sg * xg, sb * xb],
        [sr * yr, sg * yg, sb * yb],
        [sr * zr, sg * zg, sb * zb],
    ];
    invert3(m_rgb_to_xyz).expect("RGB→XYZ invertible")
}

fn invert3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if !det.is_finite() || det.abs() < 1e-30 {
        return None;
    }
    let inv_det = 1.0 / det;
    Some([
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det,
        ],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d65_maps_to_unit_rgb() {
        // D65 XYZ with Y=1 → approximately (1,1,1) in Rec.709 linear.
        let (xw, yw) = D65_XY;
        let x = xw / yw;
        let y = 1.0;
        let z = (1.0 - xw - yw) / yw;
        let m = XyzToRgbMatrix::rec709_d65_linear_v1();
        let rgb = m.apply(ColorimetricXyz::new(x, y, z).unwrap()).unwrap();
        assert!((rgb.r - 1.0).abs() < 1e-10);
        assert!((rgb.g - 1.0).abs() < 1e-10);
        assert!((rgb.b - 1.0).abs() < 1e-10);
    }

    #[test]
    fn xyz_rgb_roundtrip() {
        let m = XyzToRgbMatrix::rec709_d65_linear_v1();
        let xyz = ColorimetricXyz::new(0.4, 0.5, 0.6).unwrap();
        let rgb = m.apply(xyz).unwrap();
        let back = m.invert_apply(rgb).unwrap();
        assert!((back.x - xyz.x).abs() < 1e-12);
        assert!((back.y - xyz.y).abs() < 1e-12);
        assert!((back.z - xyz.z).abs() < 1e-12);
    }

    #[test]
    fn negatives_allowed() {
        let rgb = SceneLinearRgb::new(-0.1, 0.5, 2.0).unwrap();
        assert_eq!(rgb.negative_component_count(), 1);
    }

    #[test]
    fn matrix_digest_stable() {
        let a = XyzToRgbMatrix::rec709_d65_linear_v1().digest();
        let b = XyzToRgbMatrix::rec709_d65_linear_v1().digest();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }
}
