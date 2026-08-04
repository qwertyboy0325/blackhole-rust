//! Categorical PPM outcome map and PGM cost map (no radiometry).

use crate::outcome::{OutcomeClass, RayOutcome};
use crate::trace::TraceBundle;

/// Fixed Gate 1B2 categorical legend (RGB).
pub fn class_rgb(class: OutcomeClass) -> [u8; 3] {
    match class {
        OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => [0, 0, 0],
        OutcomeClass::DiskHit => [255, 128, 0],
        OutcomeClass::Escaped => [0, 64, 255],
        OutcomeClass::AffineLimit => [128, 0, 128],
        OutcomeClass::Failed => [255, 0, 0],
    }
}

pub fn write_outcome_ppm(bundle: &TraceBundle) -> Vec<u8> {
    let w = bundle.grid.width;
    let h = bundle.grid.height;
    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
    out.reserve(bundle.outcomes.len() * 3);
    for o in &bundle.outcomes {
        let rgb = class_rgb(o.class());
        out.extend_from_slice(&rgb);
    }
    out
}

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
