//! Composition metrics for Gate 2D3A camera search (from scene + presentation).

use crate::camera_search::CompositionMetrics;
use relativity_render::{
    LinearRgb, PresentationFrame, SceneAppearanceFrame, REC709_LUMA_WB, REC709_LUMA_WG,
    REC709_LUMA_WR,
};
use relativity_trace::OutcomeClass;

fn luma(rgb: LinearRgb) -> f64 {
    REC709_LUMA_WR * rgb.r + REC709_LUMA_WG * rgb.g + REC709_LUMA_WB * rgb.b
}

fn edge_margin_frac(col: u32, row: u32, w: u32, h: u32) -> f64 {
    let left = col as f64;
    let right = (w.saturating_sub(1).saturating_sub(col)) as f64;
    let top = row as f64;
    let bottom = (h.saturating_sub(1).saturating_sub(row)) as f64;
    let m = left.min(right).min(top).min(bottom);
    let denom = f64::from(w.min(h)).max(1.0);
    m / denom
}

/// Build composition metrics from a rendered scene appearance + presented frame digests.
pub fn composition_metrics_from_scene(
    scene: &SceneAppearanceFrame,
    presented: &PresentationFrame,
    source_physical_color_digest: &str,
    camera_spec_digest: &str,
    authority_label: &'static str,
) -> CompositionMetrics {
    let w = scene.grid.width;
    let h = scene.grid.height;
    let n = scene.pixels.len() as f64;

    let mut shadow_cols = Vec::new();
    let mut shadow_rows = Vec::new();
    let mut best_luma = f64::NEG_INFINITY;
    let mut best_col = None;
    let mut best_row = None;

    for (i, p) in scene.pixels.iter().enumerate() {
        let col = (i as u32) % w;
        let row = (i as u32) / w;
        match p.outcome_class {
            OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => {
                shadow_cols.push(col);
                shadow_rows.push(row);
            }
            OutcomeClass::DiskHit => {
                let y = luma(p.rgb);
                if y > best_luma {
                    best_luma = y;
                    best_col = Some(col);
                    best_row = Some(row);
                }
            }
            _ => {}
        }
    }

    let shadow_bbox = if shadow_cols.is_empty() {
        None
    } else {
        let min_c = *shadow_cols.iter().min().unwrap();
        let max_c = *shadow_cols.iter().max().unwrap();
        let min_r = *shadow_rows.iter().min().unwrap();
        let max_r = *shadow_rows.iter().max().unwrap();
        Some([min_c, min_r, max_c, max_r])
    };
    let shadow_centroid = if shadow_cols.is_empty() {
        None
    } else {
        let cx = shadow_cols.iter().map(|c| f64::from(*c)).sum::<f64>() / shadow_cols.len() as f64;
        let cy = shadow_rows.iter().map(|r| f64::from(*r)).sum::<f64>() / shadow_rows.len() as f64;
        Some([cx, cy])
    };
    let shadow_edge_margin_frac = match shadow_bbox {
        Some([min_c, min_r, max_c, max_r]) => edge_margin_frac(min_c, min_r, w, h)
            .min(edge_margin_frac(max_c, min_r, w, h))
            .min(edge_margin_frac(min_c, max_r, w, h))
            .min(edge_margin_frac(max_c, max_r, w, h)),
        None => 0.0,
    };
    let highlight_edge_margin_frac = match (best_col, best_row) {
        (Some(c), Some(r)) => edge_margin_frac(c, r, w, h),
        _ => 0.0,
    };

    CompositionMetrics {
        width: w,
        height: h,
        disk_hit_count: scene.disk_hit_count,
        escaped_count: scene.escaped_count,
        horizon_count: scene.horizon_count,
        affine_limit_count: scene.affine_limit_count,
        failed_count: scene.failed_count,
        disk_hit_fraction: scene.disk_hit_count as f64 / n,
        escaped_fraction: scene.escaped_count as f64 / n,
        horizon_fraction: scene.horizon_count as f64 / n,
        shadow_bbox,
        shadow_centroid,
        shadow_edge_margin_frac,
        highlight_col: best_col,
        highlight_row: best_row,
        highlight_edge_margin_frac,
        presentation_frame_digest: presented.presentation_frame_digest.clone(),
        scene_appearance_digest: scene.scene_appearance_digest.clone(),
        source_physical_color_digest: source_physical_color_digest.to_string(),
        camera_spec_digest: camera_spec_digest.to_string(),
        authority_label,
    }
}
