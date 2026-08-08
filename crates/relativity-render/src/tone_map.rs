//! Khronos PBR Neutral tone mapper (canonical Gate 2D0).

use crate::display_encoding::DISPLAY_LINEAR_EPS;
use crate::error::PresentationError;
use serde::Serialize;

pub const TONE_MAPPER_ID_KHRONOS_PBR_NEUTRAL_V1: &str = "khronos-pbr-neutral-v1";

/// Scene/display-linear RGB triple used by presentation stages.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LinearRgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// Official Khronos PBR Neutral parameters (CC-BY-4.0 specification).
const F90: f64 = 0.04;
const K_S: f64 = 0.8 - F90; // 0.76
const K_D: f64 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMapOperator {
    KhronosPbrNeutralV1,
}

impl ToneMapOperator {
    pub fn id(self) -> &'static str {
        match self {
            Self::KhronosPbrNeutralV1 => TONE_MAPPER_ID_KHRONOS_PBR_NEUTRAL_V1,
        }
    }

    pub fn parse(id: &str) -> Result<Self, PresentationError> {
        if id == TONE_MAPPER_ID_KHRONOS_PBR_NEUTRAL_V1 {
            Ok(Self::KhronosPbrNeutralV1)
        } else {
            Err(PresentationError::UnsupportedOperator(id.into()))
        }
    }
}

/// Apply Khronos PBR Neutral to non-negative linear Rec.709 RGB.
///
/// Spec: <https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md>
/// License: CC-BY-4.0.
pub fn khronos_pbr_neutral(c: LinearRgb) -> Result<(LinearRgb, u32), PresentationError> {
    if !(c.r.is_finite() && c.g.is_finite() && c.b.is_finite()) {
        return Err(PresentationError::ToneMapDomainError(
            "non-finite tone-map input".into(),
        ));
    }
    if c.r < -DISPLAY_LINEAR_EPS || c.g < -DISPLAY_LINEAR_EPS || c.b < -DISPLAY_LINEAR_EPS {
        return Err(PresentationError::ToneMapDomainError(format!(
            "negative tone-map input ({},{},{})",
            c.r, c.g, c.b
        )));
    }
    let r = c.r.max(0.0);
    let g = c.g.max(0.0);
    let b = c.b.max(0.0);

    let x = r.min(g).min(b);
    let f = if x <= 2.0 * F90 {
        x - (x * x) / (4.0 * F90)
    } else {
        F90
    };

    let p = (r - f).max(g - f).max(b - f);
    let out = if p <= K_S {
        LinearRgb {
            r: r - f,
            g: g - f,
            b: b - f,
        }
    } else {
        let p_n = 1.0 - ((1.0 - K_S) * (1.0 - K_S)) / (p + 1.0 - 2.0 * K_S);
        let desat = 1.0 / (K_D * (p - p_n) + 1.0);
        LinearRgb {
            r: (r - f) * (p_n / p) * desat + p_n * (1.0 - desat),
            g: (g - f) * (p_n / p) * desat + p_n * (1.0 - desat),
            b: (b - f) * (p_n / p) * desat + p_n * (1.0 - desat),
        }
    };

    canonicalize_tone_output(out)
}

/// Fail-closed range check with ε-only endpoint canonicalization.
pub fn canonicalize_tone_output(c: LinearRgb) -> Result<(LinearRgb, u32), PresentationError> {
    if !(c.r.is_finite() && c.g.is_finite() && c.b.is_finite()) {
        return Err(PresentationError::ToneMapRangeFailure(
            "non-finite tone-map output".into(),
        ));
    }
    let mut count = 0u32;
    let mut out = [c.r, c.g, c.b];
    for v in &mut out {
        if *v < -DISPLAY_LINEAR_EPS || *v > 1.0 + DISPLAY_LINEAR_EPS {
            return Err(PresentationError::ToneMapRangeFailure(format!(
                "tone-map output {v} outside [0,1]±ε"
            )));
        }
        if *v < 0.0 {
            *v = 0.0;
            count += 1;
        } else if *v > 1.0 {
            *v = 1.0;
            count += 1;
        }
    }
    Ok((
        LinearRgb {
            r: out[0],
            g: out[1],
            b: out[2],
        },
        count,
    ))
}

pub fn apply_tone_map(
    op: ToneMapOperator,
    c: LinearRgb,
) -> Result<(LinearRgb, u32), PresentationError> {
    match op {
        ToneMapOperator::KhronosPbrNeutralV1 => khronos_pbr_neutral(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_maps_to_black() {
        let (o, n) = khronos_pbr_neutral(LinearRgb {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        })
        .unwrap();
        assert_eq!(n, 0);
        assert!((o.r).abs() < 1e-15 && (o.g).abs() < 1e-15 && (o.b).abs() < 1e-15);
    }

    #[test]
    fn neutral_ramp_monotonic_and_bounded() {
        let mut prev = 0.0;
        for i in 0..=64 {
            let x = (i as f64) / 8.0; // 0 .. 8
            let (o, _) = khronos_pbr_neutral(LinearRgb { r: x, g: x, b: x }).unwrap();
            assert!((0.0..=1.0).contains(&o.r));
            assert!((o.r - o.g).abs() < 1e-12 && (o.g - o.b).abs() < 1e-12);
            assert!(o.r + 1e-12 >= prev);
            prev = o.r;
        }
    }

    #[test]
    fn mid_gray_1_to_1_band() {
        // Spec: for 0.08..0.8, c_out = c_in - F90 on neutrals in that band.
        let x = 0.18;
        let (o, _) = khronos_pbr_neutral(LinearRgb { r: x, g: x, b: x }).unwrap();
        assert!((o.r - (x - F90)).abs() < 1e-12);
    }

    #[test]
    fn rejects_significant_negative() {
        assert!(khronos_pbr_neutral(LinearRgb {
            r: -1e-6,
            g: 0.1,
            b: 0.1
        })
        .is_err());
    }

    #[test]
    fn extreme_hdr_finite_in_unit_interval() {
        let (o, _) = khronos_pbr_neutral(LinearRgb {
            r: 1e6,
            g: 1e6,
            b: 1e6,
        })
        .unwrap();
        assert!(o.r.is_finite() && o.r <= 1.0 + DISPLAY_LINEAR_EPS);
        assert!(o.r > 0.99);
    }
}
