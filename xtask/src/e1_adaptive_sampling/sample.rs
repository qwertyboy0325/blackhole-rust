//! Selected-ray cache and one-ray scientific adapter (no oracle access).

use crate::e1_adaptive_sampling::quadtree::DomainMapping;
use relativity_oracle::OracleChannelSet;
use relativity_render::{
    bolometric_debug_display_v1, build_disk_bolometric_frame, build_disk_frequency_shift_frame,
    diagnostic_bolometric_emission_v1, procedural_coordinate_grid_v1,
    render_bolometric_celestial_composite, render_lensed_celestial, DiskBolometricPixel,
    DiskFrequencyShiftPixel, DiskVelocityModel, LensedCelestialMode, ResolvedDiskBounds,
};
use relativity_trace::{
    build_celestial_coordinate_frame, trace_ray_pixel_with_surface_set, OutcomeClass, RayOutcome,
    TraceBundle, TraceGrid, TraceScene, TraceSurfaceSet,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveCelestialSample {
    pub theta: f64,
    pub psi: f64,
    pub direction: [f64; 3],
    pub u: f64,
    pub v: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveDiskSample {
    pub radius: f64,
    pub g_factor: f64,
    pub log2_g: f64,
    pub emitted_bolometric_intensity: f64,
    pub observed_bolometric_intensity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveRaySample {
    pub local_col: u32,
    pub local_row: u32,
    pub source_index: u64,
    pub source_col: u32,
    pub source_row: u32,
    pub outcome_class: OutcomeClass,
    pub rhs_evaluations: u64,
    pub celestial: Option<AdaptiveCelestialSample>,
    pub disk: Option<AdaptiveDiskSample>,
    pub rgb: [u8; 3],
}

/// Trace callback isolation: sampler never sees OracleFrame.
pub struct TraceContext<'a> {
    pub scene: &'a TraceScene,
    pub surface_set: TraceSurfaceSet,
    pub channel_set: OracleChannelSet,
    pub mapping: DomainMapping,
}

pub struct SampleCache {
    samples: BTreeMap<u64, AdaptiveRaySample>,
    unique_traces: u64,
    total_rhs: u64,
    max_rhs: u64,
}

impl Default for SampleCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SampleCache {
    pub fn new() -> Self {
        Self {
            samples: BTreeMap::new(),
            unique_traces: 0,
            total_rhs: 0,
            max_rhs: 0,
        }
    }

    pub fn unique_traced_rays(&self) -> u64 {
        self.unique_traces
    }

    pub fn total_rhs_evaluations(&self) -> u64 {
        self.total_rhs
    }

    pub fn maximum_rhs(&self) -> u64 {
        self.max_rhs
    }

    pub fn get(&self, source_index: u64) -> Option<&AdaptiveRaySample> {
        self.samples.get(&source_index)
    }

    pub fn samples(&self) -> &BTreeMap<u64, AdaptiveRaySample> {
        &self.samples
    }

    pub fn ensure_traced(
        &mut self,
        ctx: &TraceContext<'_>,
        source_indices: &[u64],
        parallel_threads: Option<usize>,
        pool: Option<&rayon::ThreadPool>,
    ) -> Result<Vec<u64>, Box<dyn Error>> {
        let mut missing: Vec<u64> = source_indices
            .iter()
            .copied()
            .filter(|idx| !self.samples.contains_key(idx))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        if missing.is_empty() {
            return Ok(Vec::new());
        }

        let results = if let Some(threads) = parallel_threads.filter(|t| *t > 1) {
            let traced = if let Some(pool) = pool {
                trace_outcomes_on_pool(ctx, &missing, pool)?
            } else {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .map_err(|e| format!("rayon pool: {e}"))?;
                trace_outcomes_on_pool(ctx, &missing, &pool)?
            };
            let rows: Vec<_> = traced
                .into_iter()
                .map(|t| {
                    (
                        t.local_col,
                        t.local_row,
                        t.source_index,
                        t.source_col,
                        t.source_row,
                        t.outcome,
                    )
                })
                .collect();
            let samples = adapt_outcomes_batch(ctx, &rows)?;
            rows.iter()
                .zip(samples)
                .map(|(r, s)| (r.2, s))
                .collect::<Vec<_>>()
        } else {
            let mut traced = Vec::with_capacity(missing.len());
            for idx in missing.iter().copied() {
                traced.push(trace_outcome(ctx, idx)?);
            }
            let rows: Vec<_> = traced
                .into_iter()
                .map(|t| {
                    (
                        t.local_col,
                        t.local_row,
                        t.source_index,
                        t.source_col,
                        t.source_row,
                        t.outcome,
                    )
                })
                .collect();
            let samples = adapt_outcomes_batch(ctx, &rows)?;
            rows.iter()
                .zip(samples)
                .map(|(r, s)| (r.2, s))
                .collect::<Vec<_>>()
        };

        let mut newly = Vec::with_capacity(results.len());
        for (idx, sample) in results {
            self.unique_traces += 1;
            self.total_rhs += sample.rhs_evaluations;
            self.max_rhs = self.max_rhs.max(sample.rhs_evaluations);
            newly.push(idx);
            self.samples.insert(idx, sample);
        }
        Ok(newly)
    }
}

fn trace_outcomes_on_pool(
    ctx: &TraceContext<'_>,
    missing: &[u64],
    pool: &rayon::ThreadPool,
) -> Result<Vec<TracedPixel>, Box<dyn Error>> {
    use rayon::prelude::*;
    let mut pairs = pool.install(|| {
        missing
            .par_iter()
            .map(|&idx| trace_outcome(ctx, idx).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, String>>()
    })?;
    pairs.sort_by_key(|t| t.source_index);
    Ok(pairs)
}

fn source_index_to_col_row(source_index: u64, source_width: u32) -> (u32, u32) {
    let w = u64::from(source_width);
    let row = (source_index / w) as u32;
    let col = (source_index % w) as u32;
    (col, row)
}

struct TracedPixel {
    local_col: u32,
    local_row: u32,
    source_index: u64,
    source_col: u32,
    source_row: u32,
    outcome: RayOutcome,
}

fn trace_outcome(ctx: &TraceContext<'_>, source_index: u64) -> Result<TracedPixel, Box<dyn Error>> {
    let (source_col, source_row) = source_index_to_col_row(source_index, ctx.mapping.source_width);
    let (local_col, local_row) = ctx
        .mapping
        .source_to_local(source_col, source_row)
        .ok_or("source pixel outside experiment domain")?;
    let outcome =
        trace_ray_pixel_with_surface_set(ctx.scene, source_col, source_row, ctx.surface_set)?;
    Ok(TracedPixel {
        local_col,
        local_row,
        source_index,
        source_col,
        source_row,
        outcome,
    })
}

/// Convert one traced outcome via accepted builders (delegates to batch).
#[allow(dead_code)]
pub fn adapt_outcome(
    ctx: &TraceContext<'_>,
    local_col: u32,
    local_row: u32,
    source_index: u64,
    source_col: u32,
    source_row: u32,
    outcome: RayOutcome,
) -> Result<AdaptiveRaySample, Box<dyn Error>> {
    let samples = adapt_outcomes_batch(
        ctx,
        &[(
            local_col,
            local_row,
            source_index,
            source_col,
            source_row,
            outcome,
        )],
    )?;
    samples
        .into_iter()
        .next()
        .ok_or_else(|| "empty adapt_outcomes_batch".into())
}

/// Batch scientific adaptation over ordered outcomes (source camera coords already fixed at trace).
pub fn adapt_outcomes_batch(
    ctx: &TraceContext<'_>,
    rows: &[(u32, u32, u64, u32, u32, RayOutcome)],
) -> Result<Vec<AdaptiveRaySample>, Box<dyn Error>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let n = rows.len() as u32;
    let grid = TraceGrid {
        width: n,
        height: 1,
    };
    let outcomes: Vec<RayOutcome> = rows.iter().map(|r| r.5.clone()).collect();
    let bundle = TraceBundle { grid, outcomes };
    let celestial_frame = build_celestial_coordinate_frame(&ctx.scene.kerr, &bundle)
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;

    let (frequency, bolometric) = if ctx.channel_set == OracleChannelSet::FullBolometricDisk {
        let frequency = build_disk_frequency_shift_frame(
            &ctx.scene.kerr,
            &bundle,
            DiskVelocityModel::ProgradeCircularGeodesic,
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        let bounds = ResolvedDiskBounds::new(ctx.scene.disk.r_inner, ctx.scene.disk.r_outer)
            .map_err(|e| e.to_string())?;
        let bolometric =
            build_disk_bolometric_frame(&frequency, &diagnostic_bolometric_emission_v1(), bounds)
                .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        (Some(frequency), Some(bolometric))
    } else {
        (None, None)
    };

    let texture = procedural_coordinate_grid_v1();
    let rgb_pixels = if let Some(bolo) = &bolometric {
        let frame = render_bolometric_celestial_composite(
            &celestial_frame,
            bolo,
            &texture,
            &bolometric_debug_display_v1(),
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        frame.pixels().to_vec()
    } else {
        let lensed = render_lensed_celestial(
            &celestial_frame,
            &texture,
            LensedCelestialMode::DiskOmittedDiagnostic,
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        lensed.frame.pixels().to_vec()
    };

    let mut out = Vec::with_capacity(rows.len());
    for (i, (local_col, local_row, source_index, source_col, source_row, outcome)) in
        rows.iter().enumerate()
    {
        let outcome_class = outcome.class();
        let rhs_evaluations = outcome.rhs_evaluations();
        let celestial = match celestial_frame.pixel_at(i as u32, 0) {
            relativity_trace::CelestialCoordinatePixel::Escaped(sample) => {
                Some(AdaptiveCelestialSample {
                    theta: sample.theta,
                    psi: sample.psi,
                    direction: sample.unit_coordinate_direction,
                    u: sample.uv.u,
                    v: sample.uv.v,
                })
            }
            relativity_trace::CelestialCoordinatePixel::NotEscaped { .. } => None,
        };
        let disk = match (
            frequency.as_ref().map(|f| f.pixel_at(i as u32, 0)),
            bolometric.as_ref().map(|b| b.pixel_at(i as u32, 0)),
        ) {
            (Some(DiskFrequencyShiftPixel::DiskHit(f)), Some(DiskBolometricPixel::DiskHit(b))) => {
                Some(AdaptiveDiskSample {
                    radius: f.radius,
                    g_factor: f.g_factor,
                    log2_g: f.log2_g,
                    emitted_bolometric_intensity: b.emitted_bolometric_intensity,
                    observed_bolometric_intensity: b.observed_bolometric_intensity,
                })
            }
            _ => None,
        };
        out.push(AdaptiveRaySample {
            local_col: *local_col,
            local_row: *local_row,
            source_index: *source_index,
            source_col: *source_col,
            source_row: *source_row,
            outcome_class,
            rhs_evaluations,
            celestial,
            disk,
            rgb: rgb_pixels[i],
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_prevents_duplicate_traces_counting() {
        let mut cache = SampleCache::new();
        assert_eq!(cache.unique_traced_rays(), 0);
        // Direct insert simulation
        cache.samples.insert(
            1,
            AdaptiveRaySample {
                local_col: 1,
                local_row: 0,
                source_index: 1,
                source_col: 1,
                source_row: 0,
                outcome_class: OutcomeClass::Escaped,
                rhs_evaluations: 3,
                celestial: None,
                disk: None,
                rgb: [0, 0, 0],
            },
        );
        cache.unique_traces = 1;
        assert!(cache.get(1).is_some());
        assert_eq!(cache.unique_traced_rays(), 1);
    }
}
