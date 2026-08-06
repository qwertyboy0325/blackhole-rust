//! Verified E0 reference corpus session for E1 (read-only after materialize).
//!
//! Sampler/`TraceContext` never receives this type — only experiment loaders
//! and post-reconstruction metrics use oracle frames loaded from disk.

use crate::e1_adaptive_sampling::config::{E1Config, REQUIRED_LOCK_DIGEST};
use crate::oracle_benchmark;
use crate::trace_outcome_map::CliExecution;
use relativity_trace::hex_sha;
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
    std::fs::write(
        session_root.join(MARKER),
        format!("lock_digest={lock_digest}\n"),
    )?;
    validate_existing(session_root, &committed_lock).map(|mut s| {
        s.materialize_wall_seconds = wall;
        s
    })
}

/// Validate an existing session directory against committed lock bytes.
pub fn validate_existing(
    session_root: &Path,
    committed_lock: &[u8],
) -> Result<VerifiedReferenceSession, Box<dyn Error>> {
    if !session_root.join(MARKER).is_file() {
        return Err(format!(
            "reference session missing marker at {}",
            session_root.display()
        )
        .into());
    }
    let regen_lock = std::fs::read(session_root.join("corpus-lock-v1.json"))?;
    if regen_lock != committed_lock {
        return Err("reference session lock bytes != committed lock".into());
    }
    let lock_digest = hex_sha(&regen_lock);
    if lock_digest != REQUIRED_LOCK_DIGEST {
        return Err(format!("reference session lock digest mismatch: {lock_digest}").into());
    }
    // Require baseline case artifacts present (partial-tree reject).
    let baseline = session_root
        .join("cases")
        .join("kerr0999-edge-opaque")
        .join("oracle-frame.json");
    if !baseline.is_file() {
        return Err("reference session incomplete: missing baseline oracle-frame".into());
    }
    Ok(VerifiedReferenceSession {
        root: session_root.to_path_buf(),
        lock_digest,
        materialize_wall_seconds: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_existing_rejects_missing_marker() {
        let dir =
            std::env::temp_dir().join(format!("e1-ref-session-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let err = validate_existing(&dir, b"{}").unwrap_err();
        assert!(err.to_string().contains("missing marker"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
