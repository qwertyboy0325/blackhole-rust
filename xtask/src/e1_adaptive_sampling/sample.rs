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
    build_celestial_coordinate_frame, celestial_sample_from_escape,
    trace_ray_pixel_with_surface_set, OutcomeClass, RayOutcome, TraceBundle, TraceGrid, TraceScene,
    TraceSurfaceSet,
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
            trace_batch_parallel(ctx, &missing, threads)?
        } else {
            trace_batch_serial(ctx, &missing)?
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

fn trace_batch_serial(
    ctx: &TraceContext<'_>,
    missing: &[u64],
) -> Result<Vec<(u64, AdaptiveRaySample)>, Box<dyn Error>> {
    let mut out = Vec::with_capacity(missing.len());
    for &idx in missing {
        out.push((idx, trace_one(ctx, idx)?));
    }
    Ok(out)
}

fn trace_batch_parallel(
    ctx: &TraceContext<'_>,
    missing: &[u64],
    threads: usize,
) -> Result<Vec<(u64, AdaptiveRaySample)>, Box<dyn Error>> {
    use rayon::prelude::*;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| format!("rayon pool: {e}"))?;
    let mut pairs = pool.install(|| {
        missing
            .par_iter()
            .map(|&idx| {
                trace_one(ctx, idx)
                    .map(|sample| (idx, sample))
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    pairs.sort_by_key(|(idx, _)| *idx);
    Ok(pairs)
}

fn source_index_to_col_row(source_index: u64, source_width: u32) -> (u32, u32) {
    let w = u64::from(source_width);
    let row = (source_index / w) as u32;
    let col = (source_index % w) as u32;
    (col, row)
}

fn trace_one(
    ctx: &TraceContext<'_>,
    source_index: u64,
) -> Result<AdaptiveRaySample, Box<dyn Error>> {
    let (source_col, source_row) = source_index_to_col_row(source_index, ctx.mapping.source_width);
    let (local_col, local_row) = ctx
        .mapping
        .source_to_local(source_col, source_row)
        .ok_or("source pixel outside experiment domain")?;
    let outcome =
        trace_ray_pixel_with_surface_set(ctx.scene, source_col, source_row, ctx.surface_set)?;
    adapt_outcome(
        ctx,
        local_col,
        local_row,
        source_index,
        source_col,
        source_row,
        outcome,
    )
}

/// Convert one traced outcome via accepted 1×1 builders. No retracing.
pub fn adapt_outcome(
    ctx: &TraceContext<'_>,
    local_col: u32,
    local_row: u32,
    source_index: u64,
    source_col: u32,
    source_row: u32,
    outcome: RayOutcome,
) -> Result<AdaptiveRaySample, Box<dyn Error>> {
    let outcome_class = outcome.class();
    let rhs_evaluations = outcome.rhs_evaluations();
    let grid = TraceGrid {
        width: 1,
        height: 1,
    };
    let bundle = TraceBundle {
        grid,
        outcomes: vec![outcome],
    };
    let celestial_frame = build_celestial_coordinate_frame(&ctx.scene.kerr, &bundle)
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;

    let celestial = match &bundle.outcomes[0] {
        RayOutcome::Escaped(escape) => {
            let sample = celestial_sample_from_escape(&ctx.scene.kerr, escape)
                .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
            Some(AdaptiveCelestialSample {
                theta: sample.theta,
                psi: sample.psi,
                direction: sample.unit_coordinate_direction,
                u: sample.uv.u,
                v: sample.uv.v,
            })
        }
        _ => None,
    };

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

    let disk = match (
        frequency.as_ref().and_then(|f| f.pixels().first()),
        bolometric.as_ref().and_then(|b| b.pixels().first()),
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

    let texture = procedural_coordinate_grid_v1();
    let rgb = if let Some(bolo) = &bolometric {
        let frame = render_bolometric_celestial_composite(
            &celestial_frame,
            bolo,
            &texture,
            &bolometric_debug_display_v1(),
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        frame.pixels()[0]
    } else {
        let lensed = render_lensed_celestial(
            &celestial_frame,
            &texture,
            LensedCelestialMode::DiskOmittedDiagnostic,
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        lensed.frame.pixels()[0]
    };

    Ok(AdaptiveRaySample {
        local_col,
        local_row,
        source_index,
        source_col,
        source_row,
        outcome_class,
        rhs_evaluations,
        celestial,
        disk,
        rgb,
    })
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
