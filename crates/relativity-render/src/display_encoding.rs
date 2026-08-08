//! Exact sRGB OETF (IEC 61966-2-1) and deterministic u16 quantization.

use crate::error::PresentationError;

pub const OETF_ID_SRGB_IEC61966_2_1_V1: &str = "srgb-iec61966-2-1-v1";
pub const DISPLAY_TARGET_SRGB_V1: &str = "srgb-iec61966-2-1-v1";
pub const PNG_FORMAT_RGB16_SRGB_V1: &str = "png-rgb16-srgb-v1";

/// Canonical Gate 2D0 PNG sRGB rendering intent (PNG 3rd Ed. intent 0 = Perceptual).
pub const PNG_SRGB_INTENT_PERCEPTUAL: u8 = 0;
/// PNG gAMA integer for gamma = 1/2.2 ≈ 0.45455 → encoded as 45455.
pub const PNG_GAMA_SRGB: u32 = 45_455;

/// Roundoff band for display-linear [0,1] domain checks (near unit scale).
pub const DISPLAY_LINEAR_EPS: f64 = 1e-12;

const SRGB_BREAKPOINT: f64 = 0.003_130_8;

/// Independent IEC 61966-2-1 sRGB OETF numeric oracle (D0-C1).
///
/// Expected encoded values are hard-coded from the published piecewise definition;
/// they must **not** be recomputed from the production expression at test time.
pub const SRGB_OETF_NUMERIC_ORACLE_V1: &[(f64, f64)] = &[
    (0.000_000_0, 0.000_000_000_000_000_0),
    (0.003_130_8, 0.040_449_936_000_000_0),
    (0.180_000_0, 0.461_356_129_500_441_6),
    (0.500_000_0, 0.735_356_983_052_449_5),
    (1.000_000_0, 1.000_000_000_000_000_0),
];

/// Absolute tolerance for oracle vector comparison (tight f64 near unit scale).
pub const SRGB_OETF_ORACLE_ABS_TOL: f64 = 1e-15;

/// Exact piecewise sRGB OETF on normalized linear display RGB in `[0,1]`.
pub fn srgb_oetf(x: f64) -> Result<f64, PresentationError> {
    if !x.is_finite() {
        return Err(PresentationError::DisplayEncodingFailure(
            "non-finite OETF input".into(),
        ));
    }
    if !(-DISPLAY_LINEAR_EPS..=1.0 + DISPLAY_LINEAR_EPS).contains(&x) {
        return Err(PresentationError::DisplayEncodingFailure(format!(
            "OETF input {x} outside [0,1]±ε"
        )));
    }
    let x = x.clamp(0.0, 1.0);
    let y = if x <= SRGB_BREAKPOINT {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    if !y.is_finite() || !(0.0..=1.0).contains(&y) {
        return Err(PresentationError::DisplayEncodingFailure(format!(
            "OETF output {y} invalid"
        )));
    }
    Ok(y)
}

/// Deterministic round-to-nearest: `floor(encoded * 65535 + 0.5)`.
pub fn quantize_u16(encoded: f64) -> Result<u16, PresentationError> {
    if !encoded.is_finite() {
        return Err(PresentationError::QuantizationFailure(
            "non-finite encoded value".into(),
        ));
    }
    if !(-DISPLAY_LINEAR_EPS..=1.0 + DISPLAY_LINEAR_EPS).contains(&encoded) {
        return Err(PresentationError::QuantizationFailure(format!(
            "encoded {encoded} outside [0,1]±ε"
        )));
    }
    let e = encoded.clamp(0.0, 1.0);
    let q = (e * 65535.0 + 0.5).floor();
    if !(0.0..=65535.0).contains(&q) {
        return Err(PresentationError::QuantizationFailure(format!(
            "quantized {q} out of u16 range"
        )));
    }
    Ok(q as u16)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayEncodedRgb16 {
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

impl DisplayEncodedRgb16 {
    pub fn from_linear_display_rgb(r: f64, g: f64, b: f64) -> Result<Self, PresentationError> {
        Ok(Self {
            r: quantize_u16(srgb_oetf(r)?)?,
            g: quantize_u16(srgb_oetf(g)?)?,
            b: quantize_u16(srgb_oetf(b)?)?,
        })
    }

    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_oetf_independent_numeric_oracle() {
        for &(x, expect) in SRGB_OETF_NUMERIC_ORACLE_V1 {
            let y = srgb_oetf(x).unwrap();
            assert!(
                (y - expect).abs() <= SRGB_OETF_ORACLE_ABS_TOL,
                "sRGB OETF oracle mismatch at x={x}: got {y}, expect {expect}"
            );
        }
    }

    #[test]
    fn srgb_breakpoint_neighborhood() {
        let below = SRGB_BREAKPOINT * (1.0 - 1e-9);
        let at = SRGB_BREAKPOINT;
        let above = SRGB_BREAKPOINT * (1.0 + 1e-6);
        let y_below = srgb_oetf(below).unwrap();
        let y_at = srgb_oetf(at).unwrap();
        let y_above = srgb_oetf(above).unwrap();
        // Branch selection only — expected magnitudes come from oracle / linear branch.
        assert!((y_at - 0.040_449_936_000_000_0).abs() <= SRGB_OETF_ORACLE_ABS_TOL);
        assert!(y_below < y_at);
        assert!(y_above > y_at);
        assert!(y_below.is_finite() && y_above.is_finite());
    }

    #[test]
    fn quantize_endpoints_and_mid() {
        assert_eq!(quantize_u16(0.0).unwrap(), 0);
        assert_eq!(quantize_u16(1.0).unwrap(), 65535);
        // 0.5 → 32768 (floor(0.5*65535+0.5)=floor(32768)=32768)
        assert_eq!(quantize_u16(0.5).unwrap(), 32768);
        assert_eq!(quantize_u16(1.0 / 65535.0).unwrap(), 1);
    }

    #[test]
    fn oetf_rejects_significant_oor() {
        assert!(srgb_oetf(-1e-6).is_err());
        assert!(srgb_oetf(1.0 + 1e-6).is_err());
    }
}
