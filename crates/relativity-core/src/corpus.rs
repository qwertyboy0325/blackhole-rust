//! Deterministic stratified spatial corpus for metric/derivative diagnostics.

use crate::error::DomainReason;
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
    ExpectedDomainFailure,
}

/// Expected evaluation outcome for an authoritative corpus point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedOutcome {
    Valid,
    ExpectedDomainFailure(DomainReason),
}

#[derive(Debug, Clone, Copy)]
pub struct CorpusPoint {
    pub tag: CorpusTag,
    pub pos: PositionKs,
    pub spin_override: Option<f64>,
    pub mass: f64,
    pub expected: ExpectedOutcome,
}

/// Stratified points covering Gate 1A conditioning regimes.
#[must_use]
pub fn stratified_corpus() -> Vec<CorpusPoint> {
    let mut pts = Vec::new();

    for &(x, y, z) in &[(100.0, 0.0, 0.0), (50.0, 40.0, 30.0), (-80.0, 10.0, -5.0)] {
        pts.push(valid(CorpusTag::WeakField, x, y, z, Some(0.5), 1.0));
    }

    for &(x, y, z) in &[(1e-12, 0.0, 12.0), (0.0, 1e-14, 6.0), (1e-10, 1e-10, 20.0)] {
        pts.push(valid(CorpusTag::NearAxis, x, y, z, Some(0.9), 1.0));
    }

    for &(x, y, z) in &[(8.0, 0.0, 1e-8), (5.0, 3.0, 1e-10), (15.0, -2.0, 0.0)] {
        pts.push(valid(CorpusTag::NearEquatorial, x, y, z, Some(0.7), 1.0));
    }

    let a_h = 0.9;
    let p_h = KerrParams::new(1.0, a_h).unwrap();
    let rp = p_h.outer_horizon_radius();
    for &scale in &[1.01, 1.05, 1.2] {
        pts.push(valid(
            CorpusTag::NearOuterHorizonExterior,
            rp * scale,
            0.0,
            0.1,
            Some(a_h),
            1.0,
        ));
    }

    for &scale in &[0.9, 0.7, 0.5] {
        let x = rp * scale;
        if x > 0.2 {
            pts.push(valid(
                CorpusTag::InsideHorizon,
                x,
                0.05,
                0.05,
                Some(a_h),
                1.0,
            ));
        }
    }

    for &(x, y, z) in &[(4.0, 1.0, 2.0), (2.5, 0.0, 1.0), (0.0, 0.0, 5.0)] {
        pts.push(valid(
            CorpusTag::NearExtremalSpin,
            x,
            y,
            z,
            Some(0.999),
            1.0,
        ));
    }

    for &(x, y, z) in &[
        (0.1, 0.0, 1e-8),
        (0.2, 0.1, 5e-8),
        (0.05, 0.0, 1e-7),
        (0.3, -0.2, 1e-6),
    ] {
        pts.push(valid(
            CorpusTag::CancellationProneOblate,
            x,
            y,
            z,
            Some(0.999),
            1.0,
        ));
    }

    // Explicit expected domain failures (must not be silently skipped).
    pts.push(CorpusPoint {
        tag: CorpusTag::ExpectedDomainFailure,
        pos: PositionKs::spatial(0.999, 0.0, 0.0),
        spin_override: Some(0.999),
        mass: 1.0,
        expected: ExpectedOutcome::ExpectedDomainFailure(
            DomainReason::RingSingularityOrExcludedDisk,
        ),
    });
    pts.push(CorpusPoint {
        tag: CorpusTag::ExpectedDomainFailure,
        pos: PositionKs::spatial(0.1, 0.0, 0.0),
        spin_override: Some(0.9),
        mass: 1.0,
        expected: ExpectedOutcome::ExpectedDomainFailure(
            DomainReason::RingSingularityOrExcludedDisk,
        ),
    });

    pts
}

fn valid(tag: CorpusTag, x: f64, y: f64, z: f64, spin: Option<f64>, mass: f64) -> CorpusPoint {
    CorpusPoint {
        tag,
        pos: PositionKs::spatial(x, y, z),
        spin_override: spin,
        mass,
        expected: ExpectedOutcome::Valid,
    }
}

impl CorpusPoint {
    pub fn params(&self) -> Result<KerrParams, crate::error::CoreError> {
        let a = self.spin_override.unwrap_or(0.5);
        KerrParams::new(self.mass, a)
    }
}
