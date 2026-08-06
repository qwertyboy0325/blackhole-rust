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
pub fn validate_existing(
    session_root: &Path,
    committed_lock: &[u8],
) -> Result<VerifiedReferenceSession, Box<dyn Error>> {
    let lock_digest = hex_sha(committed_lock);
    if lock_digest != REQUIRED_LOCK_DIGEST {
        return Err(format!("committed lock digest mismatch: {lock_digest}").into());
    }

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
    if regen_lock != committed_lock {
        return Err("reference session lock bytes != committed lock".into());
    }

    let lock: SessionLock = serde_json::from_slice(committed_lock)?;
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
    use std::sync::OnceLock;

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

    fn copy_dir(src: &Path, dst: &Path) {
        std::fs::create_dir_all(dst).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let to = dst.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &to);
            } else {
                std::fs::copy(entry.path(), to).unwrap();
            }
        }
    }

    /// Valid verified session tree for tamper tests (materialize once per process).
    fn valid_session_template() -> &'static Path {
        static SESSION: OnceLock<PathBuf> = OnceLock::new();
        SESSION.get_or_init(|| {
            let root = workspace_root();
            let existing = root.join("artifacts/e1-adaptive-sampling/shared-reference");
            let lock = committed_lock_bytes();
            if existing.join(MARKER).is_file() && validate_existing(&existing, &lock).is_ok() {
                let dir = temp_dir("template-copy");
                copy_dir(&existing, &dir);
                return dir;
            }
            let dir = temp_dir("template-materialize");
            let cfg = E1Config::load(&root.join("experiments/e1-adaptive-sampling/config-v1.toml"))
                .unwrap();
            materialize(
                &root,
                &cfg,
                &dir,
                CliExecution::Parallel,
                Some(
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(2)
                        .min(4),
                ),
                false,
            )
            .unwrap();
            dir
        })
    }

    fn clone_valid_session(tag: &str) -> PathBuf {
        let dir = temp_dir(tag);
        copy_dir(valid_session_template(), &dir);
        dir
    }

    #[test]
    fn validate_existing_rejects_missing_marker() {
        let dir = temp_dir("missing");
        let err = validate_existing(&dir, &committed_lock_bytes()).unwrap_err();
        assert!(err.to_string().contains("missing marker"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_existing_rejects_tampered_marker_digest() {
        let dir = clone_valid_session("marker");
        std::fs::write(dir.join(MARKER), "lock_digest=deadbeef\n").unwrap();
        let err = validate_existing(&dir, &committed_lock_bytes()).unwrap_err();
        assert!(err.to_string().contains("marker content mismatch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_existing_rejects_missing_case_artifact() {
        let dir = clone_valid_session("missing-case");
        let _ = std::fs::remove_dir_all(dir.join("cases/kerr0999-edge-opaque"));
        let err = validate_existing(&dir, &committed_lock_bytes()).unwrap_err();
        assert!(err.to_string().contains("incomplete"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_existing_rejects_tampered_oracle_frame() {
        let dir = clone_valid_session("oracle");
        let path = dir.join("cases/kerr0999-edge-opaque/oracle-frame.json");
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        if let Some(d) = v.get_mut("scientific_digest") {
            *d = serde_json::json!("00".repeat(32));
        }
        std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let err = validate_existing(&dir, &committed_lock_bytes()).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("oracle") || msg.contains("digest") || msg.contains("invalid"),
            "{msg}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_existing_rejects_tampered_source_ppm() {
        let dir = clone_valid_session("source-ppm");
        let path = dir.join("cases/kerr0999-edge-opaque/reference.ppm");
        let mut ppm = std::fs::read(&path).unwrap();
        if let Some(b) = ppm.last_mut() {
            *b ^= 0xff;
        }
        std::fs::write(&path, &ppm).unwrap();
        let err = validate_existing(&dir, &committed_lock_bytes()).unwrap_err();
        assert!(err.to_string().contains("reference_image_digest mismatch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_existing_rejects_tampered_crop_ppm() {
        let dir = clone_valid_session("crop-ppm");
        let path = dir.join("crops/kerr0999-edge-sky-boundary-crop/reference.ppm");
        let mut ppm = std::fs::read(&path).unwrap();
        if let Some(b) = ppm.last_mut() {
            *b ^= 0xff;
        }
        std::fs::write(&path, &ppm).unwrap();
        let err = validate_existing(&dir, &committed_lock_bytes()).unwrap_err();
        assert!(err.to_string().contains("reference_image_digest mismatch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn expected_marker_is_exact() {
        assert_eq!(expected_marker("abc"), "lock_digest=abc\n");
    }
}
