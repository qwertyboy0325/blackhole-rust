//! Verified E0 reference corpus session for E1 (read-only after materialize).
//!
//! Sampler/`TraceContext` never receives this type — only experiment loaders
//! and post-reconstruction metrics use oracle frames loaded from disk.

use crate::e1_adaptive_sampling::config::{E1Config, REQUIRED_LOCK_DIGEST};
use crate::oracle_benchmark;
use crate::trace_outcome_map::CliExecution;
use relativity_oracle::OracleFrame;
use relativity_trace::hex_sha;
use serde::Deserialize;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MARKER: &str = "e1-verified-reference-session.v1";

#[derive(Debug, Clone)]
pub struct VerifiedReferenceSession {
    pub root: PathBuf,
    pub lock_digest: String,
    pub materialize_wall_seconds: f64,
}

#[derive(Debug, Deserialize)]
struct SessionLock {
    source_cases: Vec<LockedSourceEntry>,
    crop_cases: Vec<LockedCropEntry>,
}

#[derive(Debug, Deserialize)]
struct LockedSourceEntry {
    definition: LockedDefinition,
    oracle_scientific_digest: String,
    reference_image_digest: String,
}

#[derive(Debug, Deserialize)]
struct LockedDefinition {
    id: String,
}

#[derive(Debug, Deserialize)]
struct LockedCropEntry {
    id: String,
    oracle_scientific_digest: String,
    reference_image_digest: String,
}

/// Generate (or re-validate) a reference corpus under `session_root`.
pub fn materialize(
    workspace: &Path,
    cfg: &E1Config,
    session_root: &Path,
    execution: CliExecution,
    threads: Option<usize>,
    require_release: bool,
) -> Result<VerifiedReferenceSession, Box<dyn Error>> {
    let committed_lock = std::fs::read(workspace.join(&cfg.oracle_lock))?;
    let lock_digest = hex_sha(&committed_lock);
    if lock_digest != REQUIRED_LOCK_DIGEST {
        return Err(format!(
            "oracle lock digest mismatch: {lock_digest} != {REQUIRED_LOCK_DIGEST}"
        )
        .into());
    }

    if session_root.join(MARKER).is_file() {
        return validate_existing(session_root, &committed_lock);
    }

    let _ = std::fs::remove_dir_all(session_root);
    std::fs::create_dir_all(session_root)?;
    let t0 = Instant::now();
    oracle_benchmark::run(
        &cfg.oracle_manifest,
        session_root.to_str().ok_or("session root utf8")?,
        execution,
        threads,
        require_release,
        false,
    )?;
    let wall = t0.elapsed().as_secs_f64();
    let regen_lock = std::fs::read(session_root.join("corpus-lock-v1.json"))?;
    if regen_lock != committed_lock {
        let _ = std::fs::remove_dir_all(session_root);
        return Err("regenerated lock bytes != committed lock".into());
    }
    // Write marker last so partial trees are rejected.
    std::fs::write(session_root.join(MARKER), expected_marker(&lock_digest))?;
    validate_existing(session_root, &committed_lock).map(|mut s| {
        s.materialize_wall_seconds = wall;
        s
    })
}

pub fn expected_marker(lock_digest: &str) -> String {
    format!("lock_digest={lock_digest}\n")
}

/// Validate an existing session directory against committed lock bytes.
///
/// Production path: requires the repository's pinned E0 lock digest, then
/// validates every source/crop OracleFrame and reference PPM. Unit tests use
/// [`validate_session_tree`] with a synthetic mini-lock instead of materializing
/// the full corpus.
pub fn validate_existing(
    session_root: &Path,
    committed_lock: &[u8],
) -> Result<VerifiedReferenceSession, Box<dyn Error>> {
    let lock_digest = hex_sha(committed_lock);
    if lock_digest != REQUIRED_LOCK_DIGEST {
        return Err(format!("committed lock digest mismatch: {lock_digest}").into());
    }
    validate_session_tree(session_root, committed_lock)
}

/// Lock-relative session validation (marker, lock bytes, every source/crop).
///
/// Does not enforce [`REQUIRED_LOCK_DIGEST`] so hermetic unit tests can exercise
/// tamper rejection against a tiny synthetic lock. Production callers must go
/// through [`validate_existing`].
pub fn validate_session_tree(
    session_root: &Path,
    lock_bytes: &[u8],
) -> Result<VerifiedReferenceSession, Box<dyn Error>> {
    let lock_digest = hex_sha(lock_bytes);

    let marker_path = session_root.join(MARKER);
    if !marker_path.is_file() {
        return Err(format!(
            "reference session missing marker at {}",
            session_root.display()
        )
        .into());
    }
    let marker = std::fs::read_to_string(&marker_path)?;
    let want = expected_marker(&lock_digest);
    if marker != want {
        return Err(format!(
            "reference session marker content mismatch: got {marker:?} want {want:?}"
        )
        .into());
    }

    let regen_lock = std::fs::read(session_root.join("corpus-lock-v1.json"))?;
    if regen_lock != lock_bytes {
        return Err("reference session lock bytes != committed lock".into());
    }

    let lock: SessionLock = serde_json::from_slice(lock_bytes)?;
    for src in &lock.source_cases {
        validate_case_artifacts(
            session_root,
            &format!("cases/{}", src.definition.id),
            &src.oracle_scientific_digest,
            &src.reference_image_digest,
        )?;
    }
    for crop in &lock.crop_cases {
        validate_case_artifacts(
            session_root,
            &format!("crops/{}", crop.id),
            &crop.oracle_scientific_digest,
            &crop.reference_image_digest,
        )?;
    }

    Ok(VerifiedReferenceSession {
        root: session_root.to_path_buf(),
        lock_digest,
        materialize_wall_seconds: 0.0,
    })
}

/// Read-only validation of one source/crop artifact directory against lock digests.
pub fn validate_case_artifacts(
    session_root: &Path,
    relative_dir: &str,
    oracle_scientific_digest: &str,
    reference_image_digest: &str,
) -> Result<(), Box<dyn Error>> {
    let dir = session_root.join(relative_dir);
    let frame_path = dir.join("oracle-frame.json");
    if !frame_path.is_file() {
        return Err(format!(
            "reference session incomplete: missing {relative_dir}/oracle-frame.json"
        )
        .into());
    }
    let frame: OracleFrame = serde_json::from_slice(&std::fs::read(&frame_path)?)?;
    frame
        .validate()
        .map_err(|e| format!("reference session oracle invalid at {relative_dir}: {e}"))?;
    if frame.scientific_digest != oracle_scientific_digest {
        return Err(format!(
            "reference session oracle scientific digest mismatch at {relative_dir}: {} != {oracle_scientific_digest}",
            frame.scientific_digest
        )
        .into());
    }

    let ppm_path = dir.join("reference.ppm");
    if !ppm_path.is_file() {
        return Err(
            format!("reference session incomplete: missing {relative_dir}/reference.ppm").into(),
        );
    }
    let ppm = std::fs::read(&ppm_path)?;
    let got = hex_sha(&ppm);
    if got != reference_image_digest {
        return Err(format!(
            "reference session reference_image_digest mismatch at {relative_dir}: {got} != {reference_image_digest}"
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_oracle::{
        oracle_scientific_digest, OracleCelestialSample, OracleDiskSample, OracleFrame,
        OraclePixel, OracleScientificClaim, OracleSourceDigests, SensorWindow, ORACLE_ID_V1,
        ORACLE_SCHEMA_VERSION,
    };
    use relativity_oracle::{OracleChannelSet, PixelCrop};
    use relativity_trace::{OutcomeClass, TraceSurfaceSet};
    use std::time::Instant;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn committed_lock_bytes() -> Vec<u8> {
        std::fs::read(workspace_root().join("experiments/oracle-benchmark/corpus-lock-v1.json"))
            .unwrap()
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "e1-ref-session-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tiny_ppm(rgb: [u8; 3]) -> Vec<u8> {
        let mut out = b"P6\n2 2\n255\n".to_vec();
        for _ in 0..4 {
            out.extend_from_slice(&rgb);
        }
        out
    }

    fn synthetic_source_frame() -> OracleFrame {
        let mut frame = OracleFrame {
            schema_version: ORACLE_SCHEMA_VERSION,
            oracle_id: ORACLE_ID_V1.into(),
            width: 2,
            height: 2,
            sensor_window: SensorWindow::full_frame(),
            surface_set: TraceSurfaceSet::OpaqueDiskHorizonEscape,
            channel_set: OracleChannelSet::FullBolometricDisk,
            scientific_claim: OracleScientificClaim::v1(),
            source_digests: OracleSourceDigests {
                numerical_profile_digest: "n".into(),
                trace_data_digest: "t".into(),
                outcome_class_digest: "o".into(),
                celestial_coordinate_digest: "c".into(),
                frequency_shift_digest: Some("f".into()),
                bolometric_digest: Some("b".into()),
            },
            pixels: vec![
                OraclePixel {
                    local_index: 0,
                    col: 0,
                    row: 0,
                    source_index: 0,
                    source_col: 0,
                    source_row: 0,
                    sensor_x: -0.5,
                    sensor_y: 0.5,
                    outcome_class: OutcomeClass::Escaped,
                    rhs_evaluations: 10,
                    failure_class: None,
                    celestial: Some(OracleCelestialSample {
                        boundary_oblate_radius: 80.0,
                        theta: 1.0,
                        psi: 0.0,
                        unit_coordinate_direction: [1.0, 0.0, 0.0],
                        u: 0.99,
                        v: 0.25,
                        escape_event_value: 0.0,
                    }),
                    disk: None,
                },
                OraclePixel {
                    local_index: 1,
                    col: 1,
                    row: 0,
                    source_index: 1,
                    source_col: 1,
                    source_row: 0,
                    sensor_x: 0.5,
                    sensor_y: 0.5,
                    outcome_class: OutcomeClass::DiskHit,
                    rhs_evaluations: 20,
                    failure_class: None,
                    celestial: None,
                    disk: Some(OracleDiskSample {
                        radius: 4.0,
                        azimuth: 0.25,
                        g_factor: 2.0,
                        log2_g: 1.0,
                        g_fourth: 16.0,
                        emitted_bolometric_intensity: 3.0,
                        observed_bolometric_intensity: 48.0,
                        disk_event_value: 0.0,
                    }),
                },
                OraclePixel {
                    local_index: 2,
                    col: 0,
                    row: 1,
                    source_index: 2,
                    source_col: 0,
                    source_row: 1,
                    sensor_x: -0.5,
                    sensor_y: -0.5,
                    outcome_class: OutcomeClass::HorizonEvent,
                    rhs_evaluations: 30,
                    failure_class: None,
                    celestial: None,
                    disk: None,
                },
                OraclePixel {
                    local_index: 3,
                    col: 1,
                    row: 1,
                    source_index: 3,
                    source_col: 1,
                    source_row: 1,
                    sensor_x: 0.5,
                    sensor_y: -0.5,
                    outcome_class: OutcomeClass::Escaped,
                    rhs_evaluations: 40,
                    failure_class: None,
                    celestial: Some(OracleCelestialSample {
                        boundary_oblate_radius: 80.0,
                        theta: 2.0,
                        psi: 1.0,
                        unit_coordinate_direction: [0.0, 1.0, 0.0],
                        u: 0.25,
                        v: 0.75,
                        escape_event_value: 0.0,
                    }),
                    disk: None,
                },
            ],
            scientific_digest: String::new(),
        };
        frame.scientific_digest = oracle_scientific_digest(&frame);
        frame.validate().unwrap();
        frame
    }

    fn synthetic_crop_frame(source: &OracleFrame) -> OracleFrame {
        relativity_oracle::crop_oracle_frame(
            source,
            PixelCrop {
                left: 1,
                top: 0,
                width: 1,
                height: 2,
            },
        )
        .unwrap()
    }

    fn write_case(dir: &Path, frame: &OracleFrame, ppm: &[u8]) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("oracle-frame.json"),
            serde_json::to_vec_pretty(frame).unwrap(),
        )
        .unwrap();
        std::fs::write(dir.join("reference.ppm"), ppm).unwrap();
    }

    /// Hermetic 1-source + 1-crop session. Never calls `materialize` / `oracle_benchmark`.
    fn synthetic_valid_session(tag: &str) -> (PathBuf, Vec<u8>) {
        let source = synthetic_source_frame();
        let crop = synthetic_crop_frame(&source);
        let source_ppm = tiny_ppm([10, 20, 30]);
        let crop_ppm = tiny_ppm([40, 50, 60]);
        let source_sci = source.scientific_digest.clone();
        let crop_sci = crop.scientific_digest.clone();
        let source_img = hex_sha(&source_ppm);
        let crop_img = hex_sha(&crop_ppm);

        let lock = serde_json::json!({
            "source_cases": [{
                "definition": { "id": "synth-source" },
                "oracle_scientific_digest": source_sci,
                "reference_image_digest": source_img,
            }],
            "crop_cases": [{
                "id": "synth-crop",
                "oracle_scientific_digest": crop_sci,
                "reference_image_digest": crop_img,
            }],
        });
        let lock_bytes = serde_json::to_vec_pretty(&lock).unwrap();
        let lock_digest = hex_sha(&lock_bytes);

        let root = temp_dir(tag);
        std::fs::write(root.join("corpus-lock-v1.json"), &lock_bytes).unwrap();
        std::fs::write(root.join(MARKER), expected_marker(&lock_digest)).unwrap();
        write_case(&root.join("cases/synth-source"), &source, &source_ppm);
        write_case(&root.join("crops/synth-crop"), &crop, &crop_ppm);
        validate_session_tree(&root, &lock_bytes).unwrap();
        (root, lock_bytes)
    }

    #[test]
    fn synthetic_fixture_is_fast_and_avoids_corpus_materialize() {
        // Guard: hermetic fixture must not perform production E0 tracing.
        // Bound is generous for slow CI hosts but far below corpus materialize.
        let t0 = Instant::now();
        let (root, lock) = synthetic_valid_session("guard");
        let elapsed = t0.elapsed().as_secs_f64();
        assert!(
            elapsed < 5.0,
            "synthetic fixture took {elapsed:.3}s; must not materialize E0 corpus"
        );
        assert!(validate_session_tree(&root, &lock).is_ok());
        // Production gate still rejects synthetic locks.
        let err = validate_existing(&root, &lock).unwrap_err();
        assert!(err.to_string().contains("committed lock digest mismatch"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn validate_existing_rejects_missing_marker() {
        let dir = temp_dir("missing");
        let err = validate_existing(&dir, &committed_lock_bytes()).unwrap_err();
        assert!(err.to_string().contains("missing marker"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_session_tree_rejects_tampered_marker_digest() {
        let (dir, lock) = synthetic_valid_session("marker");
        std::fs::write(dir.join(MARKER), "lock_digest=deadbeef\n").unwrap();
        let err = validate_session_tree(&dir, &lock).unwrap_err();
        assert!(err.to_string().contains("marker content mismatch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_session_tree_rejects_missing_case_artifact() {
        let (dir, lock) = synthetic_valid_session("missing-case");
        let _ = std::fs::remove_dir_all(dir.join("cases/synth-source"));
        let err = validate_session_tree(&dir, &lock).unwrap_err();
        assert!(err.to_string().contains("incomplete"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_session_tree_rejects_tampered_oracle_frame() {
        let (dir, lock) = synthetic_valid_session("oracle");
        let path = dir.join("cases/synth-source/oracle-frame.json");
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if let Some(d) = v.get_mut("scientific_digest") {
            *d = serde_json::json!("00".repeat(32));
        }
        std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let err = validate_session_tree(&dir, &lock).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("oracle") || msg.contains("digest") || msg.contains("invalid"),
            "{msg}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_session_tree_rejects_tampered_source_ppm() {
        let (dir, lock) = synthetic_valid_session("source-ppm");
        let path = dir.join("cases/synth-source/reference.ppm");
        let mut ppm = std::fs::read(&path).unwrap();
        if let Some(b) = ppm.last_mut() {
            *b ^= 0xff;
        }
        std::fs::write(&path, &ppm).unwrap();
        let err = validate_session_tree(&dir, &lock).unwrap_err();
        assert!(err.to_string().contains("reference_image_digest mismatch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_session_tree_rejects_tampered_crop_ppm() {
        let (dir, lock) = synthetic_valid_session("crop-ppm");
        let path = dir.join("crops/synth-crop/reference.ppm");
        let mut ppm = std::fs::read(&path).unwrap();
        if let Some(b) = ppm.last_mut() {
            *b ^= 0xff;
        }
        std::fs::write(&path, &ppm).unwrap();
        let err = validate_session_tree(&dir, &lock).unwrap_err();
        assert!(err.to_string().contains("reference_image_digest mismatch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn expected_marker_is_exact() {
        assert_eq!(expected_marker("abc"), "lock_digest=abc\n");
    }
}
