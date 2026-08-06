use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::preset::{load_preset, Preset};
use crate::reference_pipeline::{compute_reference_scientific_frames, ReferenceScientificFrames};
use crate::trace_outcome_map::{resolve_execution, CliExecution};
use relativity_oracle::{
    build_oracle_frame, compare_oracle_frames, crop_oracle_frame, OracleChannelSet, OracleFrame,
    OracleFrameInputs, PixelCrop, SensorWindow, ORACLE_ID_V1, ORACLE_SCHEMA_VERSION,
};
use relativity_render::{
    bolometric_debug_display_v1, procedural_coordinate_grid_v1,
    render_bolometric_celestial_composite, render_lensed_celestial, LensedCelestialMode,
};
use relativity_trace::{encode_ppm, hex_sha, pixel_index, TraceGrid, TraceSurfaceSet};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusManifest {
    schema_version: u32,
    corpus_id: String,
    reference_renderer_base_commit: String,
    base_preset: String,
    width: u32,
    height: u32,
    source_cases: Vec<ManifestSourceCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CorpusLock {
    schema_version: u32,
    corpus_id: String,
    reference_renderer_base_commit: String,
    oracle_schema_id: String,
    source_cases: Vec<LockedSourceCase>,
    crop_cases: Vec<LockedCropCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockedSourceCase {
    definition: ManifestSourceCase,
    oracle_scientific_digest: String,
    reference_image_digest: String,
    trace_invocations: u32,
    celestial_coordinate_passes: u32,
    oracle_assembly_passes: u32,
    observer_frequency_verification_passes: u32,
    frequency_shift_passes: u32,
    bolometric_emission_passes: u32,
    bolometric_transport_passes: u32,
    ray_count: u64,
    outcome_counts: relativity_trace::OutcomeCounts,
    channel_coverage: ChannelCoverage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockedCropCase {
    id: String,
    source: String,
    crop: PixelCrop,
    transition_score: u64,
    oracle_scientific_digest: String,
    reference_image_digest: String,
    trace_invocations: u32,
    ray_count: u64,
    outcome_counts: relativity_trace::OutcomeCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChannelCoverage {
    celestial_samples: u64,
    disk_frequency_samples: u64,
    disk_bolometric_samples: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExperimentalPerformanceReport {
    ray_count: u64,
    trace_wall_clock_seconds: f64,
    channel_wall_clock_seconds: f64,
    total_wall_clock_seconds: f64,
    rays_per_second: f64,
    serialized_oracle_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observed_resident_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbComparisonMetrics {
    pub pixel_count: u64,
    pub channel_mse: f64,
    pub maximum_absolute_channel_error: u8,
    pub exact_match: bool,
    pub psnr_db: Option<f64>,
}

pub fn run(
    manifest_path: &str,
    output_dir: &str,
    execution: CliExecution,
    threads: Option<usize>,
    require_release: bool,
    update_committed_lock: bool,
) -> Result<(), Box<dyn Error>> {
    let build = BuildExecutionMetadata::current();
    if require_release {
        require_release_execution(&build)?;
    }
    let root = workspace_root()?;
    let manifest_full = resolve_path(&root, manifest_path);
    let manifest_bytes = std::fs::read(&manifest_full)?;
    let manifest_digest = hex_sha(&Sha256::digest(&manifest_bytes));
    let manifest: CorpusManifest = toml::from_str(&String::from_utf8(manifest_bytes.clone())?)?;
    validate_manifest(&manifest)?;
    let out_dir = resolve_path(&root, output_dir);
    std::fs::create_dir_all(out_dir.join("cases"))?;
    std::fs::create_dir_all(out_dir.join("crops"))?;

    let base_preset = load_preset(&resolve_path(&root, &manifest.base_preset))?;
    let trace_execution = resolve_execution(execution, threads)?;
    let grid = TraceGrid {
        width: manifest.width,
        height: manifest.height,
    };

    let mut lock_sources = Vec::new();
    let mut source_frames: BTreeMap<String, (OracleFrame, Vec<u8>, ManifestSourceCase)> =
        BTreeMap::new();
    for case in &manifest.source_cases {
        let preset = apply_case(&base_preset, case);
        let frames = compute_reference_scientific_frames(
            &preset,
            grid,
            case.surface_set,
            case.channel_set,
            trace_execution,
        )?;
        let oracle = build_oracle_frame(OracleFrameInputs {
            trace: &frames.trace,
            celestial: &frames.celestial,
            frequency: frames.frequency.as_ref(),
            bolometric: frames.bolometric.as_ref(),
            sensor_window: SensorWindow::full_frame(),
            surface_set: case.surface_set,
            channel_set: case.channel_set,
            source_digests: frames.source_digests.clone(),
        })?;
        let ppm = render_reference_ppm(case.channel_set, &frames)?;
        let image_self_metrics = compare_rgb(&ppm, &ppm)?;
        if !image_self_metrics.exact_match {
            return Err("RGB self-comparison failed".into());
        }
        write_case_artifacts(
            &out_dir.join("cases").join(&case.id),
            &oracle,
            &ppm,
            &frames,
        )?;
        let serialized_oracle_bytes = serde_json::to_vec_pretty(&oracle)?.len() as u64;
        write_performance(
            &out_dir.join("cases").join(&case.id),
            &frames,
            serialized_oracle_bytes,
        )?;
        let self_metrics = compare_oracle_frames(&oracle, &oracle)?;
        if self_metrics.outcome_disagreement_count != 0 {
            return Err("oracle self-comparison failed".into());
        }
        lock_sources.push(LockedSourceCase {
            definition: case.clone(),
            oracle_scientific_digest: oracle.scientific_digest.clone(),
            reference_image_digest: hex_sha(&ppm),
            trace_invocations: 1,
            celestial_coordinate_passes: 1,
            oracle_assembly_passes: 1,
            observer_frequency_verification_passes: u32::from(
                case.channel_set == OracleChannelSet::FullBolometricDisk,
            ),
            frequency_shift_passes: u32::from(
                case.channel_set == OracleChannelSet::FullBolometricDisk,
            ),
            bolometric_emission_passes: u32::from(
                case.channel_set == OracleChannelSet::FullBolometricDisk,
            ),
            bolometric_transport_passes: u32::from(
                case.channel_set == OracleChannelSet::FullBolometricDisk,
            ),
            ray_count: u64::from(manifest.width) * u64::from(manifest.height),
            outcome_counts: frames.outcome_counts.clone(),
            channel_coverage: coverage(&oracle),
        });
        source_frames.insert(case.id.clone(), (oracle, ppm, case.clone()));
    }

    let crop_specs = [
        ("kerr0999-edge-opaque-boundary-crop", "kerr0999-edge-opaque"),
        ("kerr0999-edge-sky-boundary-crop", "kerr0999-edge-sky"),
    ];
    let mut lock_crops = Vec::new();
    for (crop_id, source_id) in crop_specs {
        let (source, source_ppm, _) = source_frames
            .get(source_id)
            .ok_or("crop source missing from manifest")?;
        let (crop, score) = select_boundary_crop(source, 64, 64, 8)?;
        let cropped_oracle = crop_oracle_frame(source, crop)?;
        let cropped_ppm = crop_ppm(source_ppm, source.width, crop);
        let crop_dir = out_dir.join("crops").join(crop_id);
        std::fs::create_dir_all(&crop_dir)?;
        write_oracle_and_ppm(&crop_dir, &cropped_oracle, &cropped_ppm)?;
        lock_crops.push(LockedCropCase {
            id: crop_id.into(),
            source: source_id.into(),
            crop,
            transition_score: score,
            oracle_scientific_digest: cropped_oracle.scientific_digest.clone(),
            reference_image_digest: hex_sha(&cropped_ppm),
            trace_invocations: 0,
            ray_count: u64::from(crop.width) * u64::from(crop.height),
            outcome_counts: outcome_counts(&cropped_oracle),
        });
    }

    let lock = CorpusLock {
        schema_version: 1,
        corpus_id: manifest.corpus_id.clone(),
        reference_renderer_base_commit: manifest.reference_renderer_base_commit.clone(),
        oracle_schema_id: format!("{ORACLE_ID_V1}-schema-{ORACLE_SCHEMA_VERSION}"),
        source_cases: lock_sources,
        crop_cases: lock_crops,
    };
    let lock_bytes = serde_json::to_vec_pretty(&lock)?;
    std::fs::write(out_dir.join("corpus-lock-v1.json"), &lock_bytes)?;
    if update_committed_lock {
        std::fs::write(
            root.join("experiments/oracle-benchmark/corpus-lock-v1.json"),
            &lock_bytes,
        )?;
    }

    let summary = serde_json::json!({
        "corpus_id": manifest.corpus_id,
        "manifest_path": manifest_path,
        "manifest_digest": manifest_digest,
        "oracle_schema_id": format!("{ORACLE_ID_V1}-schema-{ORACLE_SCHEMA_VERSION}"),
        "source_case_count": lock.source_cases.len(),
        "crop_case_count": lock.crop_cases.len(),
        "total_source_ray_count": u64::from(manifest.width) * u64::from(manifest.height) * lock.source_cases.len() as u64,
        "lock_digest": hex_sha(&lock_bytes),
        "known_limitations": [
            "E0 contains owner-reviewable experimental baselines only",
            "crop windows are outcome-boundary-rich candidates, not proven critical curves",
            "no adaptive sampling or formal error guarantee is implemented"
        ]
    });
    std::fs::write(
        out_dir.join("benchmark-summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    std::fs::write(
        out_dir.join("benchmark-summary.md"),
        format!(
            "# E0 Oracle Benchmark Corpus\n\n- Corpus: `{}`\n- Lock digest: `{}`\n- Sources: {}\n- Crops: {}\n",
            summary["corpus_id"].as_str().unwrap_or(""),
            summary["lock_digest"].as_str().unwrap_or(""),
            lock.source_cases.len(),
            lock.crop_cases.len()
        ),
    )?;
    Ok(())
}

fn validate_manifest(manifest: &CorpusManifest) -> Result<(), Box<dyn Error>> {
    if manifest.schema_version != 1 {
        return Err("unsupported corpus manifest schema_version".into());
    }
    if manifest.source_cases.len() != 6 {
        return Err("canonical E0 manifest must contain exactly six source cases".into());
    }
    if manifest.width != 128 || manifest.height != 128 {
        return Err("canonical E0 source cases must be 128x128".into());
    }
    let mut ids = BTreeSet::new();
    for case in &manifest.source_cases {
        if !ids.insert(case.id.as_str()) {
            return Err(format!("duplicate source case id `{}`", case.id).into());
        }
        if case.spin_a_over_m < 0.999 && case.channel_set == OracleChannelSet::FullBolometricDisk {
            return Err("lower-spin opaque full-bolometric cases are absent from E0".into());
        }
    }
    Ok(())
}

fn apply_case(base: &Preset, case: &ManifestSourceCase) -> Preset {
    let mut preset = base.clone();
    preset.spacetime.spin_a_over_m = case.spin_a_over_m;
    preset.observer.boyer_lindquist_r = case.observer_r;
    preset.observer.boyer_lindquist_theta_degrees = case.observer_theta_degrees;
    preset.observer.boyer_lindquist_phi_degrees = case.observer_phi_degrees;
    preset.camera.horizontal_field_of_view_degrees = case.horizontal_fov_degrees;
    preset
}

fn render_reference_ppm(
    channel_set: OracleChannelSet,
    frames: &ReferenceScientificFrames,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let texture_spec = procedural_coordinate_grid_v1();
    let rgb = if channel_set == OracleChannelSet::FullBolometricDisk {
        let bolometric = frames
            .bolometric
            .as_ref()
            .ok_or("full mode missing bolometric frame")?;
        let display = bolometric_debug_display_v1();
        render_bolometric_celestial_composite(
            &frames.celestial,
            bolometric,
            &texture_spec,
            &display,
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?
    } else {
        render_lensed_celestial(
            &frames.celestial,
            &texture_spec,
            LensedCelestialMode::DiskOmittedDiagnostic,
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?
        .frame
    };
    Ok(encode_ppm(&rgb))
}

fn write_case_artifacts(
    dir: &Path,
    oracle: &OracleFrame,
    ppm: &[u8],
    frames: &ReferenceScientificFrames,
) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(dir)?;
    write_oracle_and_ppm(dir, oracle, ppm)?;
    let summary = serde_json::json!({
        "oracle_scientific_digest": oracle.scientific_digest,
        "source_digests": oracle.source_digests,
        "outcome_counts": frames.outcome_counts,
        "numerical_profile": frames.numerical_profile,
        "kerr_spin_a": frames.scene.kerr.spin(),
    });
    std::fs::write(
        dir.join("scientific-summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(())
}

fn write_oracle_and_ppm(
    dir: &Path,
    oracle: &OracleFrame,
    ppm: &[u8],
) -> Result<(), Box<dyn Error>> {
    std::fs::write(
        dir.join("oracle-frame.json"),
        serde_json::to_vec_pretty(oracle)?,
    )?;
    std::fs::write(dir.join("reference.ppm"), ppm)?;
    Ok(())
}

fn write_performance(
    dir: &Path,
    frames: &ReferenceScientificFrames,
    serialized_oracle_bytes: u64,
) -> Result<(), Box<dyn Error>> {
    let total = frames.trace_wall_clock_seconds + frames.channel_wall_clock_seconds;
    let ray_count = frames.trace.outcomes.len() as u64;
    let report = ExperimentalPerformanceReport {
        ray_count,
        trace_wall_clock_seconds: frames.trace_wall_clock_seconds,
        channel_wall_clock_seconds: frames.channel_wall_clock_seconds,
        total_wall_clock_seconds: total,
        rays_per_second: if frames.trace_wall_clock_seconds > 0.0 {
            ray_count as f64 / frames.trace_wall_clock_seconds
        } else {
            0.0
        },
        serialized_oracle_bytes,
        observed_resident_memory_bytes: None,
    };
    std::fs::write(
        dir.join("performance.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}

fn coverage(frame: &OracleFrame) -> ChannelCoverage {
    ChannelCoverage {
        celestial_samples: frame
            .pixels
            .iter()
            .filter(|p| p.celestial.is_some())
            .count() as u64,
        disk_frequency_samples: frame.pixels.iter().filter(|p| p.disk.is_some()).count() as u64,
        disk_bolometric_samples: frame.pixels.iter().filter(|p| p.disk.is_some()).count() as u64,
    }
}

fn outcome_counts(frame: &OracleFrame) -> relativity_trace::OutcomeCounts {
    let mut counts = relativity_trace::OutcomeCounts {
        disk_hit: 0,
        escaped: 0,
        horizon_event: 0,
        horizon_approach: 0,
        affine_limit: 0,
        failed: 0,
    };
    for pixel in &frame.pixels {
        match pixel.outcome_class {
            relativity_trace::OutcomeClass::DiskHit => counts.disk_hit += 1,
            relativity_trace::OutcomeClass::Escaped => counts.escaped += 1,
            relativity_trace::OutcomeClass::HorizonEvent => counts.horizon_event += 1,
            relativity_trace::OutcomeClass::HorizonApproach => counts.horizon_approach += 1,
            relativity_trace::OutcomeClass::AffineLimit => counts.affine_limit += 1,
            relativity_trace::OutcomeClass::Failed => counts.failed += 1,
        }
    }
    counts
}

fn select_boundary_crop(
    frame: &OracleFrame,
    crop_width: u32,
    crop_height: u32,
    stride: u32,
) -> Result<(PixelCrop, u64), Box<dyn Error>> {
    let mut best: Option<(u64, u32, u32)> = None;
    let max_top = frame
        .height
        .checked_sub(crop_height)
        .ok_or("crop taller than frame")?;
    let max_left = frame
        .width
        .checked_sub(crop_width)
        .ok_or("crop wider than frame")?;
    for top in (0..=max_top).step_by(stride as usize) {
        for left in (0..=max_left).step_by(stride as usize) {
            let score = transition_score(frame, left, top, crop_width, crop_height);
            let candidate = (score, top, left);
            if best.is_none_or(|b| {
                candidate.0 > b.0 || (candidate.0 == b.0 && (candidate.1, candidate.2) < (b.1, b.2))
            }) {
                best = Some(candidate);
            }
        }
    }
    let (score, top, left) = best.ok_or("no crop candidates")?;
    if score == 0 {
        return Err("selected crop transition score must be greater than zero".into());
    }
    Ok((
        PixelCrop {
            left,
            top,
            width: crop_width,
            height: crop_height,
        },
        score,
    ))
}

fn transition_score(frame: &OracleFrame, left: u32, top: u32, width: u32, height: u32) -> u64 {
    let grid = TraceGrid {
        width: frame.width,
        height: frame.height,
    };
    let mut count = 0;
    for row in top..top + height {
        for col in left..left + width {
            let class = frame.pixels[pixel_index(grid, col, row)].outcome_class;
            if col + 1 < left + width
                && frame.pixels[pixel_index(grid, col + 1, row)].outcome_class != class
            {
                count += 1;
            }
            if row + 1 < top + height
                && frame.pixels[pixel_index(grid, col, row + 1)].outcome_class != class
            {
                count += 1;
            }
        }
    }
    count
}

fn crop_ppm(source_ppm: &[u8], source_width: u32, crop: PixelCrop) -> Vec<u8> {
    let header = format!("P6\n{} {}\n255\n", crop.width, crop.height);
    let source_header_end = find_ppm_payload(source_ppm);
    let source = &source_ppm[source_header_end..];
    let mut out = header.into_bytes();
    for row in crop.top..crop.top + crop.height {
        let start = ((row * source_width + crop.left) * 3) as usize;
        let end = start + (crop.width * 3) as usize;
        out.extend_from_slice(&source[start..end]);
    }
    out
}

fn find_ppm_payload(ppm: &[u8]) -> usize {
    let mut newlines = 0;
    for (idx, byte) in ppm.iter().enumerate() {
        if *byte == b'\n' {
            newlines += 1;
            if newlines == 3 {
                return idx + 1;
            }
        }
    }
    0
}

pub fn compare_rgb(
    reference: &[u8],
    candidate: &[u8],
) -> Result<RgbComparisonMetrics, Box<dyn Error>> {
    if reference.len() != candidate.len() {
        return Err("RGB buffers differ in length".into());
    }
    let mut sum_sq = 0f64;
    let mut max = 0u8;
    for (a, b) in reference.iter().zip(candidate) {
        let d = a.abs_diff(*b);
        max = max.max(d);
        sum_sq += f64::from(d) * f64::from(d);
    }
    let channel_mse = if reference.is_empty() {
        0.0
    } else {
        sum_sq / reference.len() as f64
    };
    Ok(RgbComparisonMetrics {
        pixel_count: (reference.len() / 3) as u64,
        channel_mse,
        maximum_absolute_channel_error: max,
        exact_match: channel_mse == 0.0,
        psnr_db: (channel_mse != 0.0).then_some(10.0 * (255.0f64 * 255.0 / channel_mse).log10()),
    })
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask manifest has no parent")?
        .to_path_buf())
}

fn resolve_path(root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_exact_match_uses_psnr_sentinel() {
        let rgb = [1, 2, 3, 4, 5, 6];
        let metrics = compare_rgb(&rgb, &rgb).unwrap();
        assert!(metrics.exact_match);
        assert_eq!(metrics.channel_mse, 0.0);
        assert_eq!(metrics.psnr_db, None);
    }

    #[test]
    fn manifest_rejects_unknown_fields() {
        let bad = r#"
schema_version = 1
corpus_id = "x"
reference_renderer_base_commit = "b"
base_preset = "p"
width = 128
height = 128
extra = true
source_cases = []
"#;
        assert!(toml::from_str::<CorpusManifest>(bad).is_err());
    }
}
