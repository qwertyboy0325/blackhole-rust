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
    fn srgb_zero_and_one() {
        assert!((srgb_oetf(0.0).unwrap() - 0.0).abs() < 1e-15);
        assert!((srgb_oetf(1.0).unwrap() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn srgb_breakpoint_neighborhood() {
        let below = SRGB_BREAKPOINT * (1.0 - 1e-9);
        let at = SRGB_BREAKPOINT;
        let above = SRGB_BREAKPOINT * (1.0 + 1e-6);
        let y_below = srgb_oetf(below).unwrap();
        let y_at = srgb_oetf(at).unwrap();
        let y_above = srgb_oetf(above).unwrap();
        assert!((y_below - 12.92 * below).abs() < 1e-15);
        assert!((y_at - 12.92 * at).abs() < 1e-15);
        let expect_above = 1.055 * above.powf(1.0 / 2.4) - 0.055;
        assert!((y_above - expect_above).abs() < 1e-14);
        // Branches are continuous to ~1e-4 absolute at the published breakpoint.
        let power_at = 1.055 * at.powf(1.0 / 2.4) - 0.055;
        assert!((y_at - power_at).abs() < 2e-4);
        assert!(y_above > y_at);
    }

    #[test]
    fn srgb_middle_gray_and_half() {
        let y18 = srgb_oetf(0.18).unwrap();
        let expect18 = 1.055 * 0.18_f64.powf(1.0 / 2.4) - 0.055;
        assert!((y18 - expect18).abs() < 1e-14);
        let y05 = srgb_oetf(0.5).unwrap();
        let expect05 = 1.055 * 0.5_f64.powf(1.0 / 2.4) - 0.055;
        assert!((y05 - expect05).abs() < 1e-14);
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
