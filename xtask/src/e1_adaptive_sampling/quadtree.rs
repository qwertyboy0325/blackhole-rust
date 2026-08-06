//! Integer pixel quadtree domain, split, and conservative stencil.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PixelRect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn validate_domain(&self, source_width: u32, source_height: u32) -> Result<(), String> {
        if self.width == 0 || self.height == 0 {
            return Err("zero dimensions".into());
        }
        if self.width != self.height {
            return Err("non-square domain".into());
        }
        if !self.width.is_power_of_two() {
            return Err("non-power-of-two dimensions".into());
        }
        let right = self
            .left
            .checked_add(self.width)
            .ok_or("rectangle overflow")?;
        let bottom = self
            .top
            .checked_add(self.height)
            .ok_or("rectangle overflow")?;
        if right > source_width || bottom > source_height {
            return Err("out-of-source rectangle".into());
        }
        Ok(())
    }

    pub fn area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub fn contains_local(&self, local_col: u32, local_row: u32) -> bool {
        local_col >= self.left
            && local_col < self.left + self.width
            && local_row >= self.top
            && local_row < self.top + self.height
    }

    pub fn is_splittable(&self) -> bool {
        self.width > 1 && self.height > 1
    }

    /// Child order: top-left, top-right, bottom-left, bottom-right.
    pub fn split(&self) -> Result<[PixelRect; 4], String> {
        if !self.is_splittable() {
            return Err("cannot split 1x1 cell".into());
        }
        let mid_x = self.left + self.width / 2;
        let mid_y = self.top + self.height / 2;
        let half_w = self.width / 2;
        let half_h = self.height / 2;
        Ok([
            PixelRect {
                left: self.left,
                top: self.top,
                width: half_w,
                height: half_h,
            },
            PixelRect {
                left: mid_x,
                top: self.top,
                width: half_w,
                height: half_h,
            },
            PixelRect {
                left: self.left,
                top: mid_y,
                width: half_w,
                height: half_h,
            },
            PixelRect {
                left: mid_x,
                top: mid_y,
                width: half_w,
                height: half_h,
            },
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourcePixel {
    pub source_col: u32,
    pub source_row: u32,
}

impl SourcePixel {
    pub fn source_index(&self, source_width: u32) -> u64 {
        u64::from(self.source_row) * u64::from(source_width) + u64::from(self.source_col)
    }
}

/// Local domain coordinates map 1:1 for full-frame; for crops, local==(col-left,row-top)
/// while source coordinates remain absolute in the 128×128 source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainMapping {
    pub source_width: u32,
    pub source_height: u32,
    pub domain: PixelRect,
}

impl DomainMapping {
    pub fn local_width(&self) -> u32 {
        self.domain.width
    }
    pub fn local_height(&self) -> u32 {
        self.domain.height
    }

    pub fn local_to_source(&self, local_col: u32, local_row: u32) -> SourcePixel {
        SourcePixel {
            source_col: self.domain.left + local_col,
            source_row: self.domain.top + local_row,
        }
    }

    pub fn source_to_local(&self, source_col: u32, source_row: u32) -> Option<(u32, u32)> {
        if source_col < self.domain.left
            || source_row < self.domain.top
            || source_col >= self.domain.left + self.domain.width
            || source_row >= self.domain.top + self.domain.height
        {
            return None;
        }
        Some((source_col - self.domain.left, source_row - self.domain.top))
    }
}

/// Cell rectangle in **local** coordinates of the experiment domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuadCell {
    pub rect: PixelRect,
    pub depth: u32,
}

pub fn stencil_local_points(rect: &PixelRect) -> Vec<(u32, u32)> {
    let x0 = rect.left;
    let y0 = rect.top;
    let x1 = rect.left + rect.width - 1;
    let y1 = rect.top + rect.height - 1;
    let center_x = (x0 + x1) / 2;
    let center_y = (y0 + y1) / 2;

    let mut pts = vec![(x0, y0), (x1, y0), (x0, y1), (x1, y1), (center_x, center_y)];
    if rect.is_splittable() {
        if let Ok(children) = rect.split() {
            for child in children {
                let cx0 = child.left;
                let cy0 = child.top;
                let cx1 = child.left + child.width - 1;
                let cy1 = child.top + child.height - 1;
                pts.push(((cx0 + cx1) / 2, (cy0 + cy1) / 2));
            }
        }
    }
    // Dedupe while preserving later sort.
    let mut set = BTreeSet::new();
    for p in pts {
        set.insert(p);
    }
    set.into_iter().collect()
}

pub fn stencil_source_indices(mapping: &DomainMapping, local_rect: &PixelRect) -> Vec<u64> {
    let mut idxs = stencil_local_points(local_rect)
        .into_iter()
        .map(|(lc, lr)| {
            let sp = mapping.local_to_source(lc, lr);
            sp.source_index(mapping.source_width)
        })
        .collect::<Vec<_>>();
    idxs.sort_unstable();
    idxs.dedup();
    idxs
}

/// Uniform leaf tiling for a square power-of-two domain.
pub fn build_uniform_leaves(domain_size: u32, leaf_size: u32) -> Vec<QuadCell> {
    let mut leaves = vec![QuadCell {
        rect: PixelRect {
            left: 0,
            top: 0,
            width: domain_size,
            height: domain_size,
        },
        depth: 0,
    }];
    while leaves.iter().any(|l| l.rect.width > leaf_size) {
        let mut next = Vec::new();
        for leaf in leaves {
            if leaf.rect.width > leaf_size {
                for child in leaf.rect.split().unwrap() {
                    next.push(QuadCell {
                        rect: child,
                        depth: leaf.depth + 1,
                    });
                }
            } else {
                next.push(leaf);
            }
        }
        leaves = next;
    }
    leaves
}

/// Exact unique stencil-index count for a uniform leaf size (no tracing).
pub fn uniform_unique_ray_count(mapping: &DomainMapping, leaf_size: u32) -> u64 {
    let leaves = build_uniform_leaves(mapping.local_width(), leaf_size);
    let mut set = BTreeSet::new();
    for leaf in &leaves {
        for idx in stencil_source_indices(mapping, &leaf.rect) {
            set.insert(idx);
        }
    }
    set.len() as u64
}

/// Screen diagonal from pixel-edge sensor bounds on the **source** grid.
pub fn screen_diagonal_source(mapping: &DomainMapping, local_rect: &PixelRect) -> f64 {
    let sw = f64::from(mapping.source_width);
    let sh = f64::from(mapping.source_height);
    let src_left = mapping.domain.left + local_rect.left;
    let src_top = mapping.domain.top + local_rect.top;
    let src_right = src_left + local_rect.width;
    let src_bottom = src_top + local_rect.height;
    let x0 = 2.0 * f64::from(src_left) / sw - 1.0;
    let x1 = 2.0 * f64::from(src_right) / sw - 1.0;
    let y0 = 1.0 - 2.0 * f64::from(src_top) / sh;
    let y1 = 1.0 - 2.0 * f64::from(src_bottom) / sh;
    ((x1 - x0).hypot(y0 - y1)).max(1e-12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_of_two_domains_validate() {
        let r = PixelRect {
            left: 0,
            top: 0,
            width: 64,
            height: 64,
        };
        assert!(r.validate_domain(128, 128).is_ok());
    }

    #[test]
    fn invalid_domains_reject() {
        assert!(PixelRect {
            left: 0,
            top: 0,
            width: 0,
            height: 0
        }
        .validate_domain(128, 128)
        .is_err());
        assert!(PixelRect {
            left: 0,
            top: 0,
            width: 64,
            height: 32
        }
        .validate_domain(128, 128)
        .is_err());
        assert!(PixelRect {
            left: 0,
            top: 0,
            width: 3,
            height: 3
        }
        .validate_domain(128, 128)
        .is_err());
        assert!(PixelRect {
            left: 100,
            top: 0,
            width: 64,
            height: 64
        }
        .validate_domain(128, 128)
        .is_err());
    }

    #[test]
    fn split_covers_parent_exactly() {
        let parent = PixelRect {
            left: 8,
            top: 16,
            width: 16,
            height: 16,
        };
        let kids = parent.split().unwrap();
        assert_eq!(kids[0].left, 8);
        assert_eq!(kids[0].top, 16);
        assert_eq!(kids[1].left, 16);
        assert_eq!(kids[2].top, 24);
        assert_eq!(kids[3].left, 16);
        assert_eq!(kids[3].top, 24);
        let mut cells = Vec::new();
        for k in kids {
            for r in k.top..k.top + k.height {
                for c in k.left..k.left + k.width {
                    cells.push((c, r));
                }
            }
        }
        cells.sort_unstable();
        let mut expected = Vec::new();
        for r in parent.top..parent.top + parent.height {
            for c in parent.left..parent.left + parent.width {
                expected.push((c, r));
            }
        }
        expected.sort_unstable();
        assert_eq!(cells, expected);
    }

    #[test]
    fn child_order_fixed() {
        let kids = PixelRect {
            left: 0,
            top: 0,
            width: 4,
            height: 4,
        }
        .split()
        .unwrap();
        assert_eq!(
            kids.map(|k| (k.left, k.top)),
            [(0, 0), (2, 0), (0, 2), (2, 2)]
        );
    }

    #[test]
    fn stencil_includes_corners_center_and_child_centers() {
        let r = PixelRect {
            left: 0,
            top: 0,
            width: 4,
            height: 4,
        };
        let pts = stencil_local_points(&r);
        assert!(pts.contains(&(0, 0)));
        assert!(pts.contains(&(3, 0)));
        assert!(pts.contains(&(0, 3)));
        assert!(pts.contains(&(3, 3)));
        assert!(pts.contains(&(1, 1))); // floor((0+3)/2)
                                        // child centers for 2x2 children
        assert!(pts.contains(&(0, 0))); // TL child center of [0,2)x[0,2) is (0,0) after floor
        assert!(pts.len() >= 5);
    }

    #[test]
    fn stencil_deduplicates_small_cells() {
        let r = PixelRect {
            left: 0,
            top: 0,
            width: 1,
            height: 1,
        };
        let pts = stencil_local_points(&r);
        assert_eq!(pts, vec![(0, 0)]);
    }

    #[test]
    fn stencil_indices_sort_row_major() {
        let mapping = DomainMapping {
            source_width: 128,
            source_height: 128,
            domain: PixelRect {
                left: 0,
                top: 0,
                width: 128,
                height: 128,
            },
        };
        let idxs = stencil_source_indices(
            &mapping,
            &PixelRect {
                left: 0,
                top: 0,
                width: 4,
                height: 4,
            },
        );
        let mut sorted = idxs.clone();
        sorted.sort_unstable();
        assert_eq!(idxs, sorted);
    }

    #[test]
    fn crop_local_source_mapping_exact() {
        let mapping = DomainMapping {
            source_width: 128,
            source_height: 128,
            domain: PixelRect {
                left: 24,
                top: 56,
                width: 64,
                height: 64,
            },
        };
        let sp = mapping.local_to_source(0, 0);
        assert_eq!((sp.source_col, sp.source_row), (24, 56));
        assert_eq!(mapping.source_to_local(24, 56), Some((0, 0)));
        assert_eq!(mapping.source_to_local(87, 119), Some((63, 63)));
        assert_eq!(mapping.source_to_local(0, 0), None);
    }

    #[test]
    fn uniform_unique_ray_counts_match_observed_ladders() {
        let source = DomainMapping {
            source_width: 128,
            source_height: 128,
            domain: PixelRect {
                left: 0,
                top: 0,
                width: 128,
                height: 128,
            },
        };
        assert_eq!(uniform_unique_ray_count(&source, 32), 144);
        assert_eq!(uniform_unique_ray_count(&source, 16), 576);
        assert_eq!(uniform_unique_ray_count(&source, 8), 2304);
        assert_eq!(uniform_unique_ray_count(&source, 4), 8192);
        assert_eq!(uniform_unique_ray_count(&source, 2), 16384);

        let crop = DomainMapping {
            source_width: 128,
            source_height: 128,
            domain: PixelRect {
                left: 0,
                top: 0,
                width: 64,
                height: 64,
            },
        };
        assert_eq!(uniform_unique_ray_count(&crop, 16), 144);
        assert_eq!(uniform_unique_ray_count(&crop, 8), 576);
        assert_eq!(uniform_unique_ray_count(&crop, 4), 2048);
        assert_eq!(uniform_unique_ray_count(&crop, 2), 4096);
        assert_eq!(uniform_unique_ray_count(&crop, 1), 4096);
    }
}
