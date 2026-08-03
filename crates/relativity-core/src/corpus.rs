//! Deterministic stratified spatial corpus for metric/derivative diagnostics.

use crate::kerr::KerrParams;
use crate::types::PositionKs;

/// Fixed seed reported in Gate 1A evaluations (corpus is deterministic, not RNG).
pub const CORPUS_SEED: u64 = 0x0001_a001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusTag {
    WeakField,
    NearAxis,
    NearEquatorial,
    NearOuterHorizonExterior,
    InsideHorizon,
    NearExtremalSpin,
    CancellationProneOblate,
}

#[derive(Debug, Clone, Copy)]
pub struct CorpusPoint {
    pub tag: CorpusTag,
    pub pos: PositionKs,
    pub spin_override: Option<f64>,
    pub mass: f64,
}

/// Stratified points covering Gate 1A conditioning regimes.
#[must_use]
pub fn stratified_corpus() -> Vec<CorpusPoint> {
    let mut pts = Vec::new();

    // Weak field
    for &(x, y, z) in &[(100.0, 0.0, 0.0), (50.0, 40.0, 30.0), (-80.0, 10.0, -5.0)] {
        pts.push(CorpusPoint {
            tag: CorpusTag::WeakField,
            pos: PositionKs::spatial(x, y, z),
            spin_override: Some(0.5),
            mass: 1.0,
        });
    }

    // Near axis
    for &(x, y, z) in &[(1e-12, 0.0, 12.0), (0.0, 1e-14, 6.0), (1e-10, 1e-10, 20.0)] {
        pts.push(CorpusPoint {
            tag: CorpusTag::NearAxis,
            pos: PositionKs::spatial(x, y, z),
            spin_override: Some(0.9),
            mass: 1.0,
        });
    }

    // Near equatorial
    for &(x, y, z) in &[(8.0, 0.0, 1e-8), (5.0, 3.0, 1e-10), (15.0, -2.0, 0.0)] {
        pts.push(CorpusPoint {
            tag: CorpusTag::NearEquatorial,
            pos: PositionKs::spatial(x, y, z),
            spin_override: Some(0.7),
            mass: 1.0,
        });
    }

    // Near but outside outer horizon (a=0.9 ⇒ r+ ≈ 1.435)
    let a_h = 0.9;
    let p_h = KerrParams::new(1.0, a_h).unwrap();
    let rp = p_h.outer_horizon_radius();
    for &scale in &[1.01, 1.05, 1.2] {
        pts.push(CorpusPoint {
            tag: CorpusTag::NearOuterHorizonExterior,
            pos: PositionKs::spatial(rp * scale, 0.0, 0.1),
            spin_override: Some(a_h),
            mass: 1.0,
        });
    }

    // Inside horizon, outside excluded singular domain
    for &scale in &[0.9, 0.7, 0.5] {
        let x = rp * scale;
        if x > 0.2 {
            pts.push(CorpusPoint {
                tag: CorpusTag::InsideHorizon,
                pos: PositionKs::spatial(x, 0.05, 0.05),
                spin_override: Some(a_h),
                mass: 1.0,
            });
        }
    }

    // Near-extremal spin
    for &(x, y, z) in &[(4.0, 1.0, 2.0), (2.5, 0.0, 1.0), (0.0, 0.0, 5.0)] {
        pts.push(CorpusPoint {
            tag: CorpusTag::NearExtremalSpin,
            pos: PositionKs::spatial(x, y, z),
            spin_override: Some(0.999),
            mass: 1.0,
        });
    }

    // Cancellation-prone oblate radius (A < 0, tiny z)
    // A < 0 with small z. Points with |z| ≲ 1e-9 make f64 central differences of
    // g^{μν} underflow (FD→0) while analytic ∂ remains O(10^{-3}); those depths
    // are covered by the stable-radius unit tests, not the FD derivative oracle.
    for &(x, y, z) in &[
        (0.1, 0.0, 1e-8),
        (0.2, 0.1, 5e-8),
        (0.05, 0.0, 1e-7),
        (0.3, -0.2, 1e-6),
    ] {
        pts.push(CorpusPoint {
            tag: CorpusTag::CancellationProneOblate,
            pos: PositionKs::spatial(x, y, z),
            spin_override: Some(0.999),
            mass: 1.0,
        });
    }

    pts
}

impl CorpusPoint {
    pub fn params(&self) -> Result<KerrParams, crate::error::CoreError> {
        let a = self.spin_override.unwrap_or(0.5);
        KerrParams::new(self.mass, a)
    }
}
