//! Categorical PPM encoding and PGM cost maps (no radiometry).

use crate::outcome::RayOutcome;
use crate::shade::{shade_diagnostic, DiagnosticShadeStyle, RgbFrame};
use crate::trace::TraceBundle;

/// Encode an RGB frame as binary PPM (P6), row-major.
pub fn encode_ppm(frame: &RgbFrame) -> Vec<u8> {
    let w = frame.grid().width;
    let h = frame.grid().height;
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    out.reserve(frame.pixels().len() * 3);
    for rgb in frame.pixels() {
        out.extend_from_slice(rgb);
    }
    out
}

/// Compatibility wrapper: Gate 1B2 categorical shade then PPM encode.
///
/// Equivalent to `encode_ppm(&shade_diagnostic(bundle, Gate1b2Categorical))`.
pub fn write_outcome_ppm(bundle: &TraceBundle) -> Vec<u8> {
    encode_ppm(&shade_diagnostic(
        bundle,
        DiagnosticShadeStyle::Gate1b2Categorical,
    ))
}

/// RHS cost map from trace data (not a shade style).
pub fn write_rhs_pgm(bundle: &TraceBundle) -> Vec<u8> {
    let w = bundle.grid.width;
    let h = bundle.grid.height;
    let vals: Vec<u64> = bundle
        .outcomes
        .iter()
        .map(RayOutcome::rhs_evaluations)
        .collect();
    let max_v = vals.iter().copied().max().unwrap_or(1).max(1);
    let mut out = format!("P5\n{w} {h}\n255\n").into_bytes();
    out.reserve(vals.len());
    for v in vals {
        let g = ((v as f64) / (max_v as f64) * 255.0).round() as u8;
        out.push(g);
    }
    out
}

/// Legacy name retained for callers; prefer [`crate::shade::categorical_rgb`].
pub fn class_rgb(class: crate::outcome::OutcomeClass) -> [u8; 3] {
    crate::shade::categorical_rgb(class)
}
