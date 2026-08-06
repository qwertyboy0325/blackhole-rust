//! Common leaf-local nearest-sample reconstruction.

use crate::e1_adaptive_sampling::quadtree::{PixelRect, QuadCell};
use crate::e1_adaptive_sampling::sample::AdaptiveRaySample;
use relativity_trace::OutcomeClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdaptiveReconstructedPixel {
    pub local_col: u32,
    pub local_row: u32,
    pub source_col: u32,
    pub source_row: u32,
    pub source_index: u64,
    pub provenance_source_index: u64,
    pub outcome_class: OutcomeClass,
    pub rhs_evaluations: u64,
    pub celestial: Option<crate::e1_adaptive_sampling::sample::AdaptiveCelestialSample>,
    pub disk: Option<crate::e1_adaptive_sampling::sample::AdaptiveDiskSample>,
    pub rgb: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdaptiveReconstruction {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<AdaptiveReconstructedPixel>,
}

pub fn find_leaf(leaves: &[QuadCell], local_col: u32, local_row: u32) -> Option<&QuadCell> {
    leaves
        .iter()
        .find(|leaf| leaf.rect.contains_local(local_col, local_row))
}

pub fn reconstruct(
    width: u32,
    height: u32,
    leaves: &[QuadCell],
    samples: &BTreeMap<u64, AdaptiveRaySample>,
) -> Result<AdaptiveReconstruction, Box<dyn Error>> {
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for local_row in 0..height {
        for local_col in 0..width {
            let leaf = find_leaf(leaves, local_col, local_row)
                .ok_or_else(|| format!("no leaf for ({local_col},{local_row})"))?;
            let support: Vec<&AdaptiveRaySample> = samples
                .values()
                .filter(|s| leaf.rect.contains_local(s.local_col, s.local_row))
                .collect();
            if support.is_empty() {
                return Err(format!(
                    "empty leaf support at depth {} rect {:?}",
                    leaf.depth, leaf.rect
                )
                .into());
            }
            let chosen = select_nearest(local_col, local_row, &support)?;
            pixels.push(AdaptiveReconstructedPixel {
                local_col,
                local_row,
                source_col: chosen.source_col,
                source_row: chosen.source_row,
                source_index: chosen.source_index, // will overwrite with target source below
                provenance_source_index: chosen.source_index,
                outcome_class: chosen.outcome_class,
                rhs_evaluations: chosen.rhs_evaluations,
                celestial: chosen.celestial.clone(),
                disk: chosen.disk.clone(),
                rgb: chosen.rgb,
            });
        }
    }
    // Fix target source coords: caller patches via domain mapping after.
    let _ = PixelRect {
        left: 0,
        top: 0,
        width,
        height,
    };
    Ok(AdaptiveReconstruction {
        width,
        height,
        pixels,
    })
}

pub fn select_nearest<'a>(
    local_col: u32,
    local_row: u32,
    support: &[&'a AdaptiveRaySample],
) -> Result<&'a AdaptiveRaySample, Box<dyn Error>> {
    let mut best: Option<&AdaptiveRaySample> = None;
    let mut best_key: Option<(u64, u64)> = None;
    for s in support {
        let dx = i64::from(s.local_col) - i64::from(local_col);
        let dy = i64::from(s.local_row) - i64::from(local_row);
        let dist2 = (dx * dx + dy * dy) as u64;
        let key = (dist2, s.source_index);
        if best_key.is_none_or(|b| key < b) {
            best_key = Some(key);
            best = Some(*s);
        }
    }
    best.ok_or_else(|| "empty support".into())
}

pub fn encode_reconstruction_ppm(recon: &AdaptiveReconstruction) -> Vec<u8> {
    let mut out = format!("P6\n{} {}\n255\n", recon.width, recon.height).into_bytes();
    for p in &recon.pixels {
        out.extend_from_slice(&p.rgb);
    }
    out
}

pub fn encode_sample_mask_pgm(width: u32, height: u32, traced_locals: &[(u32, u32)]) -> Vec<u8> {
    let mut mask = vec![0u8; (width * height) as usize];
    for &(c, r) in traced_locals {
        mask[(r * width + c) as usize] = 255;
    }
    let mut out = format!("P5\n{width} {height}\n255\n").into_bytes();
    out.extend_from_slice(&mask);
    out
}

pub fn encode_leaf_depth_pgm(width: u32, height: u32, leaves: &[QuadCell]) -> Vec<u8> {
    let max_depth = leaves.iter().map(|l| l.depth).max().unwrap_or(0).max(1);
    let mut vals = vec![0u8; (width * height) as usize];
    for leaf in leaves {
        let v = ((u64::from(leaf.depth) * 255) / u64::from(max_depth)) as u8;
        for r in leaf.rect.top..leaf.rect.top + leaf.rect.height {
            for c in leaf.rect.left..leaf.rect.left + leaf.rect.width {
                vals[(r * width + c) as usize] = v;
            }
        }
    }
    let mut out = format!("P5\n{width} {height}\n255\n").into_bytes();
    out.extend_from_slice(&vals);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e1_adaptive_sampling::sample::AdaptiveRaySample;
    use relativity_trace::OutcomeClass;

    fn sample(col: u32, row: u32, idx: u64, rgb: [u8; 3]) -> AdaptiveRaySample {
        AdaptiveRaySample {
            local_col: col,
            local_row: row,
            source_index: idx,
            source_col: col,
            source_row: row,
            outcome_class: OutcomeClass::Escaped,
            rhs_evaluations: 1,
            celestial: None,
            disk: None,
            rgb,
        }
    }

    #[test]
    fn reconstruction_selects_nearest_and_ties_source_index() {
        let a = sample(0, 0, 10, [1, 0, 0]);
        let b = sample(2, 0, 5, [0, 1, 0]); // same dist to (1,0)? dist to (1,0): a=1, b=1
        let chosen = select_nearest(1, 0, &[&a, &b]).unwrap();
        assert_eq!(chosen.source_index, 5);
        assert_eq!(chosen.rgb, [0, 1, 0]);
    }

    #[test]
    fn sampled_pixels_reconstruct_to_themselves() {
        let mut map = BTreeMap::new();
        let s = sample(3, 4, 100, [9, 8, 7]);
        map.insert(100, s.clone());
        let leaves = vec![QuadCell {
            rect: PixelRect {
                left: 0,
                top: 0,
                width: 8,
                height: 8,
            },
            depth: 0,
        }];
        let recon = reconstruct(8, 8, &leaves, &map).unwrap();
        let p = &recon.pixels[(4 * 8 + 3) as usize];
        assert_eq!(p.rgb, [9, 8, 7]);
        assert_eq!(p.provenance_source_index, 100);
    }

    #[test]
    fn empty_leaf_is_error() {
        let map = BTreeMap::new();
        let leaves = vec![QuadCell {
            rect: PixelRect {
                left: 0,
                top: 0,
                width: 2,
                height: 2,
            },
            depth: 0,
        }];
        assert!(reconstruct(2, 2, &leaves, &map).is_err());
    }

    #[test]
    fn reconstruction_never_blends_outcomes() {
        let mut map = BTreeMap::new();
        let mut a = sample(0, 0, 1, [255, 0, 0]);
        a.outcome_class = OutcomeClass::Escaped;
        let mut b = sample(1, 0, 2, [0, 255, 0]);
        b.outcome_class = OutcomeClass::DiskHit;
        map.insert(1, a);
        map.insert(2, b);
        let leaves = vec![QuadCell {
            rect: PixelRect {
                left: 0,
                top: 0,
                width: 2,
                height: 2,
            },
            depth: 0,
        }];
        let recon = reconstruct(2, 2, &leaves, &map).unwrap();
        for p in &recon.pixels {
            assert!(matches!(
                p.outcome_class,
                OutcomeClass::Escaped | OutcomeClass::DiskHit
            ));
            // RGB must be exact copy of a sample, not blend
            assert!(p.rgb == [255, 0, 0] || p.rgb == [0, 255, 0]);
        }
    }
}
