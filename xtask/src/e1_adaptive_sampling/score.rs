//! Intensity-only and physics-aware scoring (candidate probes only).

use crate::e1_adaptive_sampling::config::E1Config;
use crate::e1_adaptive_sampling::quadtree::{screen_diagonal_source, DomainMapping, PixelRect};
use crate::e1_adaptive_sampling::sample::AdaptiveRaySample;
use relativity_trace::OutcomeClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MethodId {
    UniformQuadtreeV1,
    IntensityOnlyAdaptiveV1,
    PhysicsAwareAdaptiveV1,
    PhysicsNoOutcome,
    PhysicsNoLensMap,
    PhysicsNoG,
    PhysicsNoRadiance,
    PhysicsNoTraceCost,
}

impl MethodId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UniformQuadtreeV1 => "uniform-quadtree-v1",
            Self::IntensityOnlyAdaptiveV1 => "intensity-only-adaptive-v1",
            Self::PhysicsAwareAdaptiveV1 => "physics-aware-adaptive-v1",
            Self::PhysicsNoOutcome => "physics-no-outcome",
            Self::PhysicsNoLensMap => "physics-no-lens-map",
            Self::PhysicsNoG => "physics-no-g",
            Self::PhysicsNoRadiance => "physics-no-radiance",
            Self::PhysicsNoTraceCost => "physics-no-trace-cost",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "uniform" | "uniform-quadtree-v1" => Self::UniformQuadtreeV1,
            "intensity-only" | "intensity-only-adaptive-v1" => Self::IntensityOnlyAdaptiveV1,
            "physics-aware" | "physics-aware-adaptive-v1" => Self::PhysicsAwareAdaptiveV1,
            "physics-no-outcome" => Self::PhysicsNoOutcome,
            "physics-no-lens-map" => Self::PhysicsNoLensMap,
            "physics-no-g" => Self::PhysicsNoG,
            "physics-no-radiance" => Self::PhysicsNoRadiance,
            "physics-no-trace-cost" => Self::PhysicsNoTraceCost,
            _ => return None,
        })
    }

    pub fn primary_methods() -> [Self; 3] {
        [
            Self::UniformQuadtreeV1,
            Self::IntensityOnlyAdaptiveV1,
            Self::PhysicsAwareAdaptiveV1,
        ]
    }

    pub fn ablation_methods() -> [Self; 5] {
        [
            Self::PhysicsNoOutcome,
            Self::PhysicsNoLensMap,
            Self::PhysicsNoG,
            Self::PhysicsNoRadiance,
            Self::PhysicsNoTraceCost,
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeatureVector {
    pub luma_span_stops: f64,
    pub luma_component: f64,
    pub outcome_raw: f64,
    pub outcome_component: f64,
    pub angular_spread: f64,
    pub angular_deformation: f64,
    pub angular_component: f64,
    pub uv_spread: f64,
    pub uv_deformation: f64,
    pub uv_component: f64,
    pub g_span: f64,
    pub g_component: f64,
    pub radiance_span: f64,
    pub radiance_component: f64,
    pub cost_span: f64,
    pub cost_component: f64,
    pub score: f64,
}

pub fn luma_y8(rgb: [u8; 3]) -> u8 {
    let y =
        (54u32 * u32::from(rgb[0]) + 183 * u32::from(rgb[1]) + 19 * u32::from(rgb[2]) + 128) >> 8;
    y as u8
}

pub fn log_intensity_from_y8(y8: u8) -> f64 {
    ((f64::from(y8) + 1.0) / 256.0).log2()
}

fn compute_angular_spread(dirs: &[[f64; 3]]) -> f64 {
    let mut max = 0.0;
    for i in 0..dirs.len() {
        for j in (i + 1)..dirs.len() {
            let a = dirs[i];
            let b = dirs[j];
            let ang = if a[0].to_bits() == b[0].to_bits()
                && a[1].to_bits() == b[1].to_bits()
                && a[2].to_bits() == b[2].to_bits()
            {
                0.0
            } else {
                let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
                dot.acos()
            };
            if ang > max {
                max = ang;
            }
        }
    }
    max
}

fn compute_uv_spread(uvs: &[(f64, f64)]) -> f64 {
    let mut max = 0.0;
    for i in 0..uvs.len() {
        for j in (i + 1)..uvs.len() {
            let (u0, v0) = uvs[i];
            let (u1, v1) = uvs[j];
            let du_raw = (u0 - u1).abs();
            let du = du_raw.min(1.0 - du_raw);
            let dv = (v0 - v1).abs();
            let s = du.hypot(dv);
            if s > max {
                max = s;
            }
        }
    }
    max
}

pub fn score_cell(
    cfg: &E1Config,
    method: MethodId,
    mapping: &DomainMapping,
    local_rect: &PixelRect,
    probes: &[&AdaptiveRaySample],
) -> FeatureVector {
    let mut ls = Vec::with_capacity(probes.len());
    for p in probes {
        ls.push(log_intensity_from_y8(luma_y8(p.rgb)));
    }
    let luma_span_stops = if ls.is_empty() {
        0.0
    } else {
        ls.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - ls.iter().cloned().fold(f64::INFINITY, f64::min)
    };
    let luma_component = luma_span_stops / cfg.luma_stop_scale;

    let mut classes = BTreeSet::new();
    for p in probes {
        classes.insert(format!("{:?}", p.outcome_class));
    }
    let outcome_raw = if classes.len() > 1 {
        cfg.outcome_priority
    } else {
        0.0
    };
    let mut outcome_component = outcome_raw;

    let dirs: Vec<[f64; 3]> = probes
        .iter()
        .filter(|p| p.outcome_class == OutcomeClass::Escaped)
        .filter_map(|p| p.celestial.as_ref().map(|c| c.direction))
        .collect();
    let uvs: Vec<(f64, f64)> = probes
        .iter()
        .filter(|p| p.outcome_class == OutcomeClass::Escaped)
        .filter_map(|p| p.celestial.as_ref().map(|c| (c.u, c.v)))
        .collect();
    let diag = screen_diagonal_source(mapping, local_rect);
    let angular_spread = if dirs.len() >= 2 {
        compute_angular_spread(&dirs)
    } else {
        0.0
    };
    let angular_deformation = angular_spread / diag;
    let mut angular_component = angular_deformation.min(cfg.component_cap);

    let uv_spread = if uvs.len() >= 2 {
        compute_uv_spread(&uvs)
    } else {
        0.0
    };
    let uv_deformation = uv_spread / diag;
    let mut uv_component = uv_deformation.min(cfg.component_cap);

    let log2gs: Vec<f64> = probes
        .iter()
        .filter_map(|p| p.disk.as_ref().map(|d| d.log2_g))
        .collect();
    let g_span = if log2gs.len() >= 2 {
        log2gs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - log2gs.iter().cloned().fold(f64::INFINITY, f64::min)
    } else {
        0.0
    };
    let mut g_component = (g_span / cfg.g_stop_scale).min(cfg.component_cap);

    let log2i: Vec<f64> = probes
        .iter()
        .filter_map(|p| {
            p.disk
                .as_ref()
                .map(|d| d.observed_bolometric_intensity.log2())
        })
        .filter(|v| v.is_finite())
        .collect();
    let radiance_span = if log2i.len() >= 2 {
        log2i.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - log2i.iter().cloned().fold(f64::INFINITY, f64::min)
    } else {
        0.0
    };
    let mut radiance_component = (radiance_span / cfg.radiance_stop_scale).min(cfg.component_cap);

    let costs: Vec<f64> = probes
        .iter()
        .map(|p| (p.rhs_evaluations as f64 + 1.0).log2())
        .collect();
    let cost_span = if costs.len() >= 2 {
        costs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - costs.iter().cloned().fold(f64::INFINITY, f64::min)
    } else {
        0.0
    };
    let mut cost_component = (cost_span / cfg.cost_stop_scale).min(cfg.component_cap);

    match method {
        MethodId::IntensityOnlyAdaptiveV1 => {
            outcome_component = 0.0;
            angular_component = 0.0;
            uv_component = 0.0;
            g_component = 0.0;
            radiance_component = 0.0;
            cost_component = 0.0;
        }
        MethodId::PhysicsNoOutcome => outcome_component = 0.0,
        MethodId::PhysicsNoLensMap => {
            angular_component = 0.0;
            uv_component = 0.0;
        }
        MethodId::PhysicsNoG => g_component = 0.0,
        MethodId::PhysicsNoRadiance => radiance_component = 0.0,
        MethodId::PhysicsNoTraceCost => cost_component = 0.0,
        MethodId::UniformQuadtreeV1 | MethodId::PhysicsAwareAdaptiveV1 => {}
    }

    let score = match method {
        MethodId::IntensityOnlyAdaptiveV1 => luma_component,
        MethodId::UniformQuadtreeV1 => 0.0,
        _ => luma_component
            .max(outcome_component)
            .max(angular_component)
            .max(uv_component)
            .max(g_component)
            .max(radiance_component)
            .max(cost_component),
    };

    FeatureVector {
        luma_span_stops,
        luma_component,
        outcome_raw,
        outcome_component,
        angular_spread,
        angular_deformation,
        angular_component,
        uv_spread,
        uv_deformation,
        uv_component,
        g_span,
        g_component,
        radiance_span,
        radiance_component,
        cost_span,
        cost_component,
        score,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityKey {
    pub area: u64,
    pub depth: u32,
    pub top: u32,
    pub left: u32,
}

/// Compare (score, key) for adaptive selection: highest score, then area desc,
/// depth asc, top asc, left asc.
/// Returns `Greater` when `a` should be selected over `b`.
pub fn priority_cmp(a: (f64, PriorityKey), b: (f64, PriorityKey)) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match a.0.total_cmp(&b.0) {
        Ordering::Equal => {}
        o => return o,
    }
    // area descending
    match a.1.area.cmp(&b.1.area) {
        Ordering::Equal => {}
        o => return o,
    }
    // depth ascending → invert
    match b.1.depth.cmp(&a.1.depth) {
        Ordering::Equal => {}
        o => return o,
    }
    // top ascending → invert
    match b.1.top.cmp(&a.1.top) {
        Ordering::Equal => {}
        o => return o,
    }
    // left ascending → invert
    b.1.left.cmp(&a.1.left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e1_adaptive_sampling::sample::{AdaptiveCelestialSample, AdaptiveDiskSample};

    fn cfg() -> E1Config {
        E1Config {
            schema_version: 1,
            experiment_id: "e1-physics-aware-adaptive-quadtree-v1".into(),
            oracle_manifest: "m".into(),
            oracle_lock: "l".into(),
            source_leaf_sizes: vec![32, 16, 8, 4, 2],
            crop_leaf_sizes: vec![16, 8, 4, 2, 1],
            luma_stop_scale: 0.5,
            outcome_priority: 8.0,
            g_stop_scale: 0.125,
            radiance_stop_scale: 0.5,
            cost_stop_scale: 0.5,
            component_cap: 8.0,
            reconstruction: "leaf-local-nearest-v1".into(),
            probe_stencil: "corners-center-child-centers-v1".into(),
        }
    }

    fn probe(
        rgb: [u8; 3],
        class: OutcomeClass,
        rhs: u64,
        celestial: Option<AdaptiveCelestialSample>,
        disk: Option<AdaptiveDiskSample>,
    ) -> AdaptiveRaySample {
        AdaptiveRaySample {
            local_col: 0,
            local_row: 0,
            source_index: 0,
            source_col: 0,
            source_row: 0,
            outcome_class: class,
            rhs_evaluations: rhs,
            celestial,
            disk,
            rgb,
        }
    }

    #[test]
    fn intensity_score_uses_only_luma() {
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
        let rect = PixelRect {
            left: 0,
            top: 0,
            width: 4,
            height: 4,
        };
        let a = probe([0, 0, 0], OutcomeClass::Escaped, 10, None, None);
        let b = probe(
            [255, 255, 255],
            OutcomeClass::DiskHit,
            999,
            None,
            Some(AdaptiveDiskSample {
                radius: 1.0,
                g_factor: 2.0,
                log2_g: 1.0,
                emitted_bolometric_intensity: 4.0,
                observed_bolometric_intensity: 8.0,
            }),
        );
        let fv = score_cell(
            &cfg(),
            MethodId::IntensityOnlyAdaptiveV1,
            &mapping,
            &rect,
            &[&a, &b],
        );
        assert_eq!(fv.outcome_component, 0.0);
        assert_eq!(fv.g_component, 0.0);
        assert_eq!(fv.score, fv.luma_component);
        assert!(fv.luma_component > 0.0);
    }

    #[test]
    fn intensity_unaffected_by_scientific_mutation_with_fixed_rgb() {
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
        let rect = PixelRect {
            left: 0,
            top: 0,
            width: 2,
            height: 2,
        };
        let a = probe([10, 20, 30], OutcomeClass::Escaped, 1, None, None);
        let mut b = probe([40, 50, 60], OutcomeClass::Escaped, 1, None, None);
        let s1 = score_cell(
            &cfg(),
            MethodId::IntensityOnlyAdaptiveV1,
            &mapping,
            &rect,
            &[&a, &b],
        )
        .score;
        b.rhs_evaluations = 1_000_000;
        b.outcome_class = OutcomeClass::DiskHit;
        let s2 = score_cell(
            &cfg(),
            MethodId::IntensityOnlyAdaptiveV1,
            &mapping,
            &rect,
            &[&a, &b],
        )
        .score;
        assert_eq!(s1.to_bits(), s2.to_bits());
    }

    #[test]
    fn outcome_feature_activates_on_mixed_classes() {
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
        let rect = PixelRect {
            left: 0,
            top: 0,
            width: 2,
            height: 2,
        };
        let a = probe([0, 0, 0], OutcomeClass::Escaped, 1, None, None);
        let b = probe([0, 0, 0], OutcomeClass::DiskHit, 1, None, None);
        let fv = score_cell(
            &cfg(),
            MethodId::PhysicsAwareAdaptiveV1,
            &mapping,
            &rect,
            &[&a, &b],
        );
        assert_eq!(fv.outcome_component, 8.0);
    }

    #[test]
    fn identical_directions_yield_exact_zero_angular_spread() {
        let d = [0.0, 0.0, 1.0];
        assert_eq!(compute_angular_spread(&[d, d]), 0.0);
    }

    #[test]
    fn seam_aware_u_spread_handles_01_seam() {
        let s = compute_uv_spread(&[(0.01, 0.5), (0.99, 0.5)]);
        assert!(s < 0.1);
    }

    #[test]
    fn ablations_disable_only_target_feature() {
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
        let rect = PixelRect {
            left: 0,
            top: 0,
            width: 4,
            height: 4,
        };
        let a = probe(
            [0, 0, 0],
            OutcomeClass::Escaped,
            1,
            Some(AdaptiveCelestialSample {
                theta: 1.0,
                psi: 0.0,
                direction: [1.0, 0.0, 0.0],
                u: 0.0,
                v: 0.2,
            }),
            None,
        );
        let b = probe(
            [255, 0, 0],
            OutcomeClass::DiskHit,
            100,
            None,
            Some(AdaptiveDiskSample {
                radius: 3.0,
                g_factor: 2.0,
                log2_g: 1.0,
                emitted_bolometric_intensity: 2.0,
                observed_bolometric_intensity: 4.0,
            }),
        );
        let full = score_cell(
            &cfg(),
            MethodId::PhysicsAwareAdaptiveV1,
            &mapping,
            &rect,
            &[&a, &b],
        );
        let no_out = score_cell(
            &cfg(),
            MethodId::PhysicsNoOutcome,
            &mapping,
            &rect,
            &[&a, &b],
        );
        assert_eq!(no_out.outcome_component, 0.0);
        assert_eq!(no_out.g_component.to_bits(), full.g_component.to_bits());
        let no_g = score_cell(&cfg(), MethodId::PhysicsNoG, &mapping, &rect, &[&a, &b]);
        assert_eq!(no_g.g_component, 0.0);
        assert_eq!(
            no_g.outcome_component.to_bits(),
            full.outcome_component.to_bits()
        );
    }

    #[test]
    fn priority_ties_use_area_depth_top_left() {
        let a = (
            1.0,
            PriorityKey {
                area: 16,
                depth: 1,
                top: 0,
                left: 0,
            },
        );
        let b = (
            1.0,
            PriorityKey {
                area: 64,
                depth: 0,
                top: 0,
                left: 0,
            },
        );
        // larger area preferred
        assert_eq!(priority_cmp(a, b), std::cmp::Ordering::Less);
        let c = (
            1.0,
            PriorityKey {
                area: 16,
                depth: 0,
                top: 0,
                left: 0,
            },
        );
        let d = (
            1.0,
            PriorityKey {
                area: 16,
                depth: 1,
                top: 0,
                left: 0,
            },
        );
        // shallower depth preferred
        assert_eq!(priority_cmp(c, d), std::cmp::Ordering::Greater);
    }
}
