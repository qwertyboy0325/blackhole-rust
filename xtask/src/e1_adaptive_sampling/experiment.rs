//! E1 experiment runner: reference prep, schedules, matrix, artifacts.

use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::diagnostic_scene::build_diagnostic_trace_scene;
use crate::e1_adaptive_sampling::config::{
    E1Config, APPROVED_BASE, REQUIRED_BASELINE_ORACLE_DIGEST, REQUIRED_LOCK_DIGEST,
};
use crate::e1_adaptive_sampling::metrics::{
    compare_reconstruction_rgb, compare_reconstruction_to_oracle, encode_outcome_disagreement_pgm,
    verify_selected_sample_parity,
};
use crate::e1_adaptive_sampling::quadtree::{
    stencil_source_indices, DomainMapping, PixelRect, QuadCell,
};
use crate::e1_adaptive_sampling::reconstruct::{
    encode_leaf_depth_pgm, encode_reconstruction_ppm, encode_sample_mask_pgm, reconstruct,
};
use crate::e1_adaptive_sampling::report::{
    build_worst_pixel_records, classify_hypothesis, write_experiment_reports, CurvePoint,
    E1CaseReport, E1ExperimentReport, MatchedComparison, MethodCurve, ScheduleEvent,
};
use crate::e1_adaptive_sampling::sample::{SampleCache, TraceContext};
use crate::e1_adaptive_sampling::score::{
    priority_cmp, score_cell, FeatureVector, MethodId, PriorityKey,
};
use crate::oracle_benchmark;
use crate::preset::load_preset;
use crate::trace_outcome_map::{resolve_execution, CliExecution};
use relativity_oracle::{OracleChannelSet, OracleFrame, PixelCrop};
use relativity_trace::{hex_sha, TraceSurfaceSet};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct ExperimentFilters {
    pub case: Option<String>,
    pub method: Option<MethodId>,
    pub maximum_budget_level: Option<usize>,
    pub skip_ablations: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusManifest {
    #[allow(dead_code)]
    schema_version: u32,
    #[allow(dead_code)]
    corpus_id: String,
    #[allow(dead_code)]
    reference_renderer_base_commit: String,
    #[allow(dead_code)]
    base_preset: String,
    width: u32,
    height: u32,
    source_cases: Vec<ManifestSourceCase>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestSourceCase {
    id: String,
    spin_a_over_m: f64,
    observer_r: f64,
    observer_theta_degrees: f64,
    observer_phi_degrees: f64,
    horizontal_fov_degrees: f64,
    surface_set: TraceSurfaceSet,
    channel_set: OracleChannelSet,
}

#[derive(Debug, Clone, Deserialize)]
struct CorpusLock {
    source_cases: Vec<LockedSource>,
    crop_cases: Vec<LockedCrop>,
}

#[derive(Debug, Clone, Deserialize)]
struct LockedSource {
    definition: ManifestSourceCase,
    oracle_scientific_digest: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LockedCrop {
    id: String,
    source: String,
    crop: PixelCrop,
    #[allow(dead_code)]
    transition_score: u64,
    oracle_scientific_digest: String,
}

#[derive(Clone)]
struct CaseSpec {
    // clone for crop derivation from source cases
    id: String,
    #[allow(dead_code)]
    source_id: String,
    definition: ManifestSourceCase,
    mapping: DomainMapping,
    is_crop: bool,
    oracle: OracleFrame,
    reference_ppm: Vec<u8>,
}

pub fn run(
    config_path: &str,
    output_dir: &str,
    execution: CliExecution,
    threads: Option<usize>,
    require_release: bool,
    filters: ExperimentFilters,
) -> Result<(), Box<dyn Error>> {
    let t0 = Instant::now();
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }
    let root = workspace_root()?;
    let cfg = E1Config::load(&resolve(&root, config_path))?;
    let out = resolve(&root, output_dir);
    std::fs::create_dir_all(&out)?;

    let committed_lock = std::fs::read(resolve(&root, &cfg.oracle_lock))?;
    let lock_digest = hex_sha(&committed_lock);
    if lock_digest != REQUIRED_LOCK_DIGEST {
        return Err(format!(
            "oracle lock digest mismatch: {lock_digest} != {REQUIRED_LOCK_DIGEST}"
        )
        .into());
    }

    // Regenerate E0 reference with skip lock update.
    let ref_dir = out.join("reference");
    let _ = std::fs::remove_dir_all(&ref_dir);
    std::fs::create_dir_all(&ref_dir)?;
    let t_oracle = Instant::now();
    oracle_benchmark::run(
        &cfg.oracle_manifest,
        ref_dir.to_str().ok_or("ref dir utf8")?,
        execution,
        threads,
        require_release,
        false, // do not update committed lock
    )?;
    let oracle_wall = t_oracle.elapsed().as_secs_f64();
    let regen_lock = std::fs::read(ref_dir.join("corpus-lock-v1.json"))?;
    if regen_lock != committed_lock {
        return Err("regenerated lock bytes != committed lock".into());
    }

    let manifest: CorpusManifest = toml::from_str(&std::fs::read_to_string(resolve(
        &root,
        &cfg.oracle_manifest,
    ))?)?;
    let lock: CorpusLock = serde_json::from_slice(&committed_lock)?;
    let cases = load_cases(&root, &ref_dir, &manifest, &lock)?;
    let baseline = cases
        .iter()
        .find(|c| c.id == "kerr0999-edge-opaque")
        .ok_or("missing baseline case")?;
    if baseline.oracle.scientific_digest != REQUIRED_BASELINE_ORACLE_DIGEST {
        return Err("baseline oracle digest mismatch".into());
    }

    let parallel_threads = match execution {
        CliExecution::Serial => None,
        CliExecution::Parallel => Some(resolve_execution(execution, threads)?.thread_count()),
    };

    let mut case_reports = Vec::new();
    let mut ablation_reports = Vec::new();
    let canonical = filters.case.is_none()
        && filters.method.is_none()
        && filters.maximum_budget_level.is_none()
        && !filters.skip_ablations;

    for case in &cases {
        if filters.case.as_ref().is_some_and(|f| f != &case.id) {
            continue;
        }
        let leaf_sizes = if case.is_crop {
            cfg.crop_leaf_sizes.clone()
        } else {
            cfg.source_leaf_sizes.clone()
        };
        let max_level = filters
            .maximum_budget_level
            .unwrap_or(leaf_sizes.len())
            .min(leaf_sizes.len());

        let mut method_curves = Vec::new();
        for method in MethodId::primary_methods() {
            if filters.method.is_some_and(|m| m != method) {
                continue;
            }
            let curve = run_method_ladder(
                &cfg,
                case,
                method,
                &leaf_sizes[..max_level],
                parallel_threads,
                &out.join("cases").join(&case.id).join(method_dir(method)),
            )?;
            method_curves.push(curve);
        }
        fill_matched_comparisons(&mut method_curves);
        case_reports.push(E1CaseReport {
            case_id: case.id.clone(),
            is_crop: case.is_crop,
            methods: method_curves,
        });
    }

    if !filters.skip_ablations {
        let ablation_cases = [
            "kerr0999-edge-opaque",
            "kerr0999-edge-opaque-boundary-crop",
            "kerr0999-edge-sky-boundary-crop",
        ];
        for case in &cases {
            if !ablation_cases.contains(&case.id.as_str()) {
                continue;
            }
            if filters.case.as_ref().is_some_and(|f| f != &case.id) {
                continue;
            }
            let leaf_sizes = if case.is_crop {
                cfg.crop_leaf_sizes.clone()
            } else {
                cfg.source_leaf_sizes.clone()
            };
            let max_level = filters
                .maximum_budget_level
                .unwrap_or(leaf_sizes.len())
                .min(leaf_sizes.len());
            for method in MethodId::ablation_methods() {
                if filters.method.is_some_and(|m| m != method) {
                    continue;
                }
                let curve = run_method_ladder(
                    &cfg,
                    case,
                    method,
                    &leaf_sizes[..max_level],
                    parallel_threads,
                    &out.join("ablations").join(&case.id).join(method.as_str()),
                )?;
                ablation_reports.push(E1CaseReport {
                    case_id: case.id.clone(),
                    is_crop: case.is_crop,
                    methods: vec![curve],
                });
            }
        }
    }

    let hypothesis = classify_hypothesis(&case_reports);
    let commit = git_head(&root).unwrap_or_else(|| "unknown".into());
    let dirty = porcelain_dirty(&root);
    let mut report = E1ExperimentReport {
        schema_version: 1,
        experiment_id: cfg.experiment_id.clone(),
        base_commit: APPROVED_BASE.into(),
        evaluated_commit: commit,
        dirty,
        oracle_lock_digest: lock_digest,
        oracle_baseline_digest: REQUIRED_BASELINE_ORACLE_DIGEST.into(),
        configuration_digest: cfg.digest()?,
        evidence_class: "experimental-reproducible".into(),
        cases: case_reports,
        ablations: ablation_reports,
        hypothesis_classification: hypothesis.clone(),
        recommendation: recommendation_for(&hypothesis),
        deterministic_content_digest: String::new(),
        total_wall_clock_seconds: t0.elapsed().as_secs_f64(),
        oracle_reference_wall_clock_seconds: oracle_wall,
        canonical,
        filters: format!("{filters:?}"),
    };
    let digest = content_digest(&report)?;
    report.deterministic_content_digest = digest;
    write_experiment_reports(&out, &report)?;
    println!(
        "E1 experiment complete: hypothesis={} digest={}",
        report.hypothesis_classification, report.deterministic_content_digest
    );
    Ok(())
}

fn fill_matched_comparisons(methods: &mut [MethodCurve]) {
    let uniform = methods
        .iter()
        .find(|m| m.method_id.contains("uniform"))
        .cloned();
    let intensity = methods
        .iter()
        .find(|m| m.method_id.contains("intensity"))
        .cloned();
    for method in methods.iter_mut() {
        if method.method_id.contains("uniform") {
            continue;
        }
        let mut matched = Vec::new();
        for p in &method.points {
            if let Some(u) = &uniform {
                if let Some(b) = u
                    .points
                    .iter()
                    .filter(|bp| bp.unique_traced_rays <= p.unique_traced_rays)
                    .max_by_key(|bp| bp.unique_traced_rays)
                {
                    matched.push(dominance_row(
                        &method.method_id,
                        "uniform-quadtree-v1",
                        p,
                        b,
                    ));
                }
            }
            if method.method_id.contains("physics") {
                if let Some(i) = &intensity {
                    if let Some(b) = i
                        .points
                        .iter()
                        .filter(|bp| bp.unique_traced_rays <= p.unique_traced_rays)
                        .max_by_key(|bp| bp.unique_traced_rays)
                    {
                        matched.push(dominance_row(
                            &method.method_id,
                            "intensity-only-adaptive-v1",
                            p,
                            b,
                        ));
                    }
                }
            }
        }
        method.matched = matched;
    }
}

fn dominance_row(
    candidate_method: &str,
    baseline_method: &str,
    cand: &CurvePoint,
    base: &CurvePoint,
) -> MatchedComparison {
    fn dom(c: f64, b: f64) -> String {
        if c == 0.0 && b == 0.0 {
            "equal".into()
        } else if c == 0.0 && b > 0.0 {
            "better".into()
        } else if c > 0.0 && b == 0.0 {
            "worse".into()
        } else if c < b {
            "better".into()
        } else if c > b {
            "worse".into()
        } else {
            "equal".into()
        }
    }
    fn opt_dom(c: Option<f64>, b: Option<f64>) -> String {
        match (c, b) {
            (Some(cv), Some(bv)) => dom(cv, bv),
            (None, None) => "n/a".into(),
            _ => "n/a".into(),
        }
    }
    MatchedComparison {
        candidate_method: candidate_method.into(),
        baseline_method: baseline_method.into(),
        candidate_rays: cand.unique_traced_rays,
        matched_baseline_rays: base.unique_traced_rays,
        ray_count_difference: cand.unique_traced_rays as i64 - base.unique_traced_rays as i64,
        outcome_rate_dominance: dom(
            cand.scientific.outcome_disagreement_rate,
            base.scientific.outcome_disagreement_rate,
        ),
        rgb_mse_dominance: dom(cand.rgb.channel_mse, base.rgb.channel_mse),
        angular_rmse_dominance: opt_dom(
            cand.scientific
                .celestial_angular_error_radians
                .as_ref()
                .map(|m| m.rmse),
            base.scientific
                .celestial_angular_error_radians
                .as_ref()
                .map(|m| m.rmse),
        ),
        log2_iobs_rmse_dominance: opt_dom(
            cand.scientific.log2_observed_error.as_ref().map(|m| m.rmse),
            base.scientific.log2_observed_error.as_ref().map(|m| m.rmse),
        ),
    }
}

fn method_dir(m: MethodId) -> &'static str {
    match m {
        MethodId::UniformQuadtreeV1 => "uniform",
        MethodId::IntensityOnlyAdaptiveV1 => "intensity-only",
        MethodId::PhysicsAwareAdaptiveV1 => "physics-aware",
        other => other.as_str(),
    }
}

fn recommendation_for(h: &str) -> String {
    match h {
        "SUPPORTED_ON_E0_CORPUS" => "PROCEED_TO_E1_HARDENING".into(),
        "MIXED_ON_E0_CORPUS" => "ITERATE_E1_ESTIMATOR".into(),
        _ => "PAUSE_RESEARCH_WEDGE".into(),
    }
}

fn run_method_ladder(
    cfg: &E1Config,
    case: &CaseSpec,
    method: MethodId,
    leaf_sizes: &[u32],
    parallel_threads: Option<usize>,
    out_root: &Path,
) -> Result<MethodCurve, Box<dyn Error>> {
    std::fs::create_dir_all(out_root)?;
    let root = workspace_root()?;
    let base = load_preset(&root.join("presets/gargantua-baseline.toml"))?;
    let preset = apply_case(&base, &case.definition);
    let grid = relativity_trace::TraceGrid {
        width: case.mapping.source_width,
        height: case.mapping.source_height,
    };
    let (scene, _) = build_diagnostic_trace_scene(&preset, grid)?;
    let ctx = TraceContext {
        scene: &scene,
        surface_set: case.definition.surface_set,
        channel_set: case.definition.channel_set,
        mapping: case.mapping,
    };

    // Uniform budgets first when adaptive.
    let uniform_budgets = if method == MethodId::UniformQuadtreeV1 {
        Vec::new()
    } else {
        let mut budgets = Vec::new();
        for &leaf in leaf_sizes {
            let mut cache = SampleCache::new();
            let leaves = build_uniform_leaves(case.mapping.local_width(), leaf);
            ensure_leaves_stencils(&mut cache, &ctx, &leaves, parallel_threads)?;
            budgets.push(cache.unique_traced_rays());
        }
        budgets
    };

    let mut points = Vec::new();
    let mut prev_uniform_rays = Vec::new();

    for (level, &leaf) in leaf_sizes.iter().enumerate() {
        let t_case = Instant::now();
        let mut cache = SampleCache::new();
        let mut schedule = Vec::new();
        let leaves = if method == MethodId::UniformQuadtreeV1 {
            let leaves = build_uniform_leaves(case.mapping.local_width(), leaf);
            let newly = ensure_leaves_stencils(&mut cache, &ctx, &leaves, parallel_threads)?;
            schedule.push(ScheduleEvent {
                step: 0,
                requested_target: cache.unique_traced_rays(),
                actual_unique_rays: cache.unique_traced_rays(),
                overshoot: 0,
                selected: None,
                score: None,
                features: None,
                newly_traced: newly,
                leaf_count: leaves.len() as u64,
                max_depth: leaves.iter().map(|l| l.depth).max().unwrap_or(0),
            });
            prev_uniform_rays.push(cache.unique_traced_rays());
            leaves
        } else {
            let target = uniform_budgets[level];
            run_adaptive(
                cfg,
                method,
                &ctx,
                &mut cache,
                target,
                parallel_threads,
                &mut schedule,
            )?
        };

        let t_recon = Instant::now();
        let mut recon = reconstruct(
            case.mapping.local_width(),
            case.mapping.local_height(),
            &leaves,
            cache.samples(),
        )?;
        // Patch target source coordinates for each local pixel.
        for p in &mut recon.pixels {
            let sp = case.mapping.local_to_source(p.local_col, p.local_row);
            p.source_col = sp.source_col;
            p.source_row = sp.source_row;
            p.source_index = sp.source_index(case.mapping.source_width);
        }
        let recon_s = t_recon.elapsed().as_secs_f64();

        let sample_refs: Vec<&_> = cache.samples().values().collect();
        let parity =
            verify_selected_sample_parity(&sample_refs, &case.oracle, &case.reference_ppm)?;
        if parity.selected_sample_mismatch_count != 0 {
            return Err(format!(
                "selected sample mismatch on {}/{}: {:?}",
                case.id,
                method.as_str(),
                parity
            )
            .into());
        }

        let t_metric = Instant::now();
        let sci = compare_reconstruction_to_oracle(&case.oracle, &recon)?;
        let ppm = encode_reconstruction_ppm(&recon);
        let rgb = compare_reconstruction_rgb(&case.reference_ppm, &ppm)?;
        let worst_pixels = build_worst_pixel_records(
            &case.oracle,
            &recon,
            &leaves,
            &schedule,
            &sci,
            &case.reference_ppm,
        );
        let metric_s = t_metric.elapsed().as_secs_f64();

        // Full-domain coverage finals: source leaf=2 stencils cover every pixel;
        // crop leaf=1 traces every pixel. Intermediate filtered ladders must not
        // be held to exact reconstruction.
        let is_full_coverage_final = leaf == 1 || (!case.is_crop && leaf == 2);
        if is_full_coverage_final && (!rgb.exact_match || sci.outcome_disagreement_count != 0) {
            return Err(format!(
                "final full-ray entry not exact for {}/{} leaf={leaf} rays={} mse={} outcome_dis={}",
                case.id,
                method.as_str(),
                cache.unique_traced_rays(),
                rgb.channel_mse,
                sci.outcome_disagreement_count
            )
            .into());
        }

        let budget_id = format!("leaf-{leaf}");
        let dir = out_root.join(&budget_id);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("reconstruction.ppm"), &ppm)?;
        let traced_locals: Vec<_> = cache
            .samples()
            .values()
            .map(|s| (s.local_col, s.local_row))
            .collect();
        std::fs::write(
            dir.join("sample-mask.pgm"),
            encode_sample_mask_pgm(recon.width, recon.height, &traced_locals),
        )?;
        std::fs::write(
            dir.join("leaf-depth.pgm"),
            encode_leaf_depth_pgm(recon.width, recon.height, &leaves),
        )?;
        std::fs::write(
            dir.join("outcome-disagreement.pgm"),
            encode_outcome_disagreement_pgm(&case.oracle, &recon),
        )?;
        let sci_json = serde_json::to_vec_pretty(&sci)?;
        std::fs::write(dir.join("scientific-error-summary.json"), sci_json)?;
        std::fs::write(
            dir.join("schedule-summary.json"),
            serde_json::to_vec_pretty(&schedule)?,
        )?;

        let domain_rays = u64::from(recon.width) * u64::from(recon.height);
        points.push(CurvePoint {
            budget_id,
            leaf_size: leaf,
            unique_traced_rays: cache.unique_traced_rays(),
            ray_fraction: cache.unique_traced_rays() as f64 / domain_rays as f64,
            total_rhs_evaluations: cache.total_rhs_evaluations(),
            mean_rhs_per_ray: if cache.unique_traced_rays() == 0 {
                0.0
            } else {
                cache.total_rhs_evaluations() as f64 / cache.unique_traced_rays() as f64
            },
            maximum_rhs: cache.maximum_rhs(),
            scientific: sci,
            rgb,
            sample_parity: parity,
            schedule,
            worst_pixels,
            wall_clock_seconds: t_case.elapsed().as_secs_f64(),
            reconstruction_wall_clock_seconds: recon_s,
            metric_wall_clock_seconds: metric_s,
        });
    }

    Ok(MethodCurve {
        method_id: method.as_str().into(),
        points,
        matched: Vec::new(),
    })
}

fn run_adaptive(
    cfg: &E1Config,
    method: MethodId,
    ctx: &TraceContext<'_>,
    cache: &mut SampleCache,
    target_rays: u64,
    parallel_threads: Option<usize>,
    schedule: &mut Vec<ScheduleEvent>,
) -> Result<Vec<QuadCell>, Box<dyn Error>> {
    let root = QuadCell {
        rect: PixelRect {
            left: 0,
            top: 0,
            width: ctx.mapping.local_width(),
            height: ctx.mapping.local_height(),
        },
        depth: 0,
    };
    let mut leaves = vec![root];
    let newly = ensure_leaves_stencils(cache, ctx, &leaves, parallel_threads)?;
    schedule.push(ScheduleEvent {
        step: 0,
        requested_target: target_rays,
        actual_unique_rays: cache.unique_traced_rays(),
        overshoot: cache.unique_traced_rays().saturating_sub(target_rays),
        selected: None,
        score: None,
        features: None,
        newly_traced: newly,
        leaf_count: 1,
        max_depth: 0,
    });

    let mut step = 1u64;
    while cache.unique_traced_rays() < target_rays {
        // Score leaves
        let mut best: Option<(usize, f64, PriorityKey, FeatureVector)> = None;
        for (i, leaf) in leaves.iter().enumerate() {
            if !leaf.rect.is_splittable() {
                continue;
            }
            let idxs = stencil_source_indices(&ctx.mapping, &leaf.rect);
            let probes: Vec<&_> = idxs.iter().filter_map(|idx| cache.get(*idx)).collect();
            if probes.len() != idxs.len() {
                return Err("incomplete stencil before scoring".into());
            }
            let fv = score_cell(cfg, method, &ctx.mapping, &leaf.rect, &probes);
            let key = PriorityKey {
                area: leaf.rect.area(),
                depth: leaf.depth,
                top: leaf.rect.top,
                left: leaf.rect.left,
            };
            let cand = (i, fv.score, key, fv);
            let replace = match &best {
                None => true,
                Some(b) => {
                    priority_cmp((cand.1, cand.2), (b.1, b.2)) == std::cmp::Ordering::Greater
                }
            };
            if replace {
                best = Some(cand);
            }
        }
        let Some((idx, score, _, features)) = best else {
            break;
        };
        let parent = leaves.remove(idx);
        let children = parent.rect.split()?;
        let mut new_cells = Vec::new();
        for child in children {
            new_cells.push(QuadCell {
                rect: child,
                depth: parent.depth + 1,
            });
        }
        let newly = ensure_leaves_stencils(cache, ctx, &new_cells, parallel_threads)?;
        leaves.extend(new_cells);
        schedule.push(ScheduleEvent {
            step,
            requested_target: target_rays,
            actual_unique_rays: cache.unique_traced_rays(),
            overshoot: cache.unique_traced_rays().saturating_sub(target_rays),
            selected: Some(parent.rect),
            score: Some(score),
            features: Some(features),
            newly_traced: newly,
            leaf_count: leaves.len() as u64,
            max_depth: leaves.iter().map(|l| l.depth).max().unwrap_or(0),
        });
        step += 1;
    }
    Ok(leaves)
}

fn build_uniform_leaves(domain_size: u32, leaf_size: u32) -> Vec<QuadCell> {
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

fn ensure_leaves_stencils(
    cache: &mut SampleCache,
    ctx: &TraceContext<'_>,
    leaves: &[QuadCell],
    parallel_threads: Option<usize>,
) -> Result<Vec<u64>, Box<dyn Error>> {
    let mut all = BTreeSet::new();
    for leaf in leaves {
        for idx in stencil_source_indices(&ctx.mapping, &leaf.rect) {
            all.insert(idx);
        }
    }
    let idxs: Vec<u64> = all.into_iter().collect();
    cache.ensure_traced(ctx, &idxs, parallel_threads)
}

fn load_cases(
    _root: &Path,
    ref_dir: &Path,
    manifest: &CorpusManifest,
    lock: &CorpusLock,
) -> Result<Vec<CaseSpec>, Box<dyn Error>> {
    let mut out = Vec::new();
    for src in &lock.source_cases {
        let def = manifest
            .source_cases
            .iter()
            .find(|c| c.id == src.definition.id)
            .ok_or("manifest/lock source mismatch")?
            .clone();
        let dir = ref_dir.join("cases").join(&def.id);
        let oracle: OracleFrame =
            serde_json::from_slice(&std::fs::read(dir.join("oracle-frame.json"))?)?;
        oracle.validate().map_err(|e| e.to_string())?;
        if oracle.scientific_digest != src.oracle_scientific_digest {
            return Err(format!("digest mismatch for {}", def.id).into());
        }
        let ppm = std::fs::read(dir.join("reference.ppm"))?;
        out.push(CaseSpec {
            id: def.id.clone(),
            source_id: def.id.clone(),
            definition: def,
            mapping: DomainMapping {
                source_width: manifest.width,
                source_height: manifest.height,
                domain: PixelRect {
                    left: 0,
                    top: 0,
                    width: manifest.width,
                    height: manifest.height,
                },
            },
            is_crop: false,
            oracle,
            reference_ppm: ppm,
        });
    }
    for crop in &lock.crop_cases {
        let source = out
            .iter()
            .find(|c| c.id == crop.source)
            .ok_or("crop source missing")?
            .clone();
        let dir = ref_dir.join("crops").join(&crop.id);
        let oracle: OracleFrame =
            serde_json::from_slice(&std::fs::read(dir.join("oracle-frame.json"))?)?;
        oracle.validate().map_err(|e| e.to_string())?;
        if oracle.scientific_digest != crop.oracle_scientific_digest {
            return Err(format!("crop digest mismatch {}", crop.id).into());
        }
        let ppm = std::fs::read(dir.join("reference.ppm"))?;
        crop.crop.validate_against_source()?;
        out.push(CaseSpec {
            id: crop.id.clone(),
            source_id: crop.source.clone(),
            definition: source.definition,
            mapping: DomainMapping {
                source_width: manifest.width,
                source_height: manifest.height,
                domain: PixelRect {
                    left: crop.crop.left,
                    top: crop.crop.top,
                    width: crop.crop.width,
                    height: crop.crop.height,
                },
            },
            is_crop: true,
            oracle,
            reference_ppm: ppm,
        });
    }
    Ok(out)
}

fn apply_case(base: &crate::preset::Preset, case: &ManifestSourceCase) -> crate::preset::Preset {
    let mut preset = base.clone();
    preset.spacetime.spin_a_over_m = case.spin_a_over_m;
    preset.observer.boyer_lindquist_r = case.observer_r;
    preset.observer.boyer_lindquist_theta_degrees = case.observer_theta_degrees;
    preset.observer.boyer_lindquist_phi_degrees = case.observer_phi_degrees;
    preset.camera.horizontal_field_of_view_degrees = case.horizontal_fov_degrees;
    preset
}

trait CropValidate {
    fn validate_against_source(&self) -> Result<(), Box<dyn Error>>;
}
impl CropValidate for PixelCrop {
    fn validate_against_source(&self) -> Result<(), Box<dyn Error>> {
        let r = PixelRect {
            left: self.left,
            top: self.top,
            width: self.width,
            height: self.height,
        };
        r.validate_domain(128, 128).map_err(|e| e.into())
    }
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}

fn resolve(root: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

fn git_head(root: &Path) -> Option<String> {
    let o = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn porcelain_dirty(root: &Path) -> bool {
    std::process::Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(true)
}

fn content_digest(report: &E1ExperimentReport) -> Result<String, Box<dyn Error>> {
    let mut v = serde_json::to_value(report)?;
    if let Some(obj) = v.as_object_mut() {
        obj.remove("deterministic_content_digest");
        obj.remove("total_wall_clock_seconds");
        obj.remove("oracle_reference_wall_clock_seconds");
        // strip per-point timings
        if let Some(cases) = obj.get_mut("cases").and_then(|c| c.as_array_mut()) {
            strip_timings(cases);
        }
        if let Some(cases) = obj.get_mut("ablations").and_then(|c| c.as_array_mut()) {
            strip_timings(cases);
        }
    }
    Ok(hex_sha(&Sha256::digest(serde_json::to_vec(&v)?)))
}

fn strip_timings(cases: &mut [serde_json::Value]) {
    for case in cases {
        if let Some(methods) = case.get_mut("methods").and_then(|m| m.as_array_mut()) {
            for method in methods {
                if let Some(points) = method.get_mut("points").and_then(|p| p.as_array_mut()) {
                    for p in points {
                        if let Some(o) = p.as_object_mut() {
                            o.remove("wall_clock_seconds");
                            o.remove("reconstruction_wall_clock_seconds");
                            o.remove("metric_wall_clock_seconds");
                        }
                    }
                }
            }
        }
    }
}

// Silence unused import warning for MatchedComparison until report fills it.
#[allow(dead_code)]
fn _matched_ty(_: MatchedComparison) {}
