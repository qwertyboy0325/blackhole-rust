//! Project-owned build-execution metadata for Gate 2A0 release authority.

use serde::{Deserialize, Serialize};

/// Compile-time / runtime build profile facts for an xtask binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildExecutionMetadata {
    pub cargo_profile: String,
    pub opt_level: String,
    pub debug_assertions: bool,
    pub target: String,
    pub toolchain: String,
}

impl BuildExecutionMetadata {
    /// Metadata for the currently executing xtask binary.
    pub fn current() -> Self {
        Self {
            cargo_profile: env!("BH_CARGO_PROFILE").to_string(),
            opt_level: env!("BH_OPT_LEVEL").to_string(),
            debug_assertions: cfg!(debug_assertions),
            target: env!("BH_TARGET").to_string(),
            toolchain: env!("BH_TOOLCHAIN").to_string(),
        }
    }

    /// Gate 2A0 authoritative release predicate (standard `release` profile only).
    pub fn is_optimized_release_execution(&self) -> bool {
        self.cargo_profile == "release" && !self.debug_assertions && self.opt_level != "0"
    }

    /// Human-readable profile description for guard errors.
    pub fn describe(&self) -> String {
        format!(
            "cargo_profile={} opt_level={} debug_assertions={} target={}",
            self.cargo_profile, self.opt_level, self.debug_assertions, self.target
        )
    }
}

/// Public predicate matching the Gate 2A0 authoritative condition.
pub fn is_optimized_release_execution() -> bool {
    BuildExecutionMetadata::current().is_optimized_release_execution()
}

/// Fail before any tracing when `--require-release` is set and this binary is not release.
pub fn require_release_execution(
    meta: &BuildExecutionMetadata,
) -> Result<(), Box<dyn std::error::Error>> {
    if meta.is_optimized_release_execution() {
        return Ok(());
    }
    Err(format!(
        "--require-release rejected non-release xtask execution ({})",
        meta.describe()
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_release() -> BuildExecutionMetadata {
        BuildExecutionMetadata {
            cargo_profile: "release".into(),
            opt_level: "3".into(),
            debug_assertions: false,
            target: "aarch64-apple-darwin".into(),
            toolchain: "rustc 1.96.0".into(),
        }
    }

    fn sample_dev() -> BuildExecutionMetadata {
        BuildExecutionMetadata {
            cargo_profile: "dev".into(),
            opt_level: "0".into(),
            debug_assertions: true,
            target: "aarch64-apple-darwin".into(),
            toolchain: "rustc 1.96.0".into(),
        }
    }

    #[test]
    fn build_metadata_fields_non_empty() {
        let m = BuildExecutionMetadata::current();
        assert!(!m.cargo_profile.is_empty());
        assert!(!m.opt_level.is_empty());
        assert!(!m.target.is_empty());
        assert!(!m.toolchain.is_empty());
    }

    #[test]
    fn debug_assertions_matches_cfg() {
        let m = BuildExecutionMetadata::current();
        assert_eq!(m.debug_assertions, cfg!(debug_assertions));
    }

    #[test]
    fn release_guard_accepts_release_metadata() {
        assert!(sample_release().is_optimized_release_execution());
        assert!(require_release_execution(&sample_release()).is_ok());
    }

    #[test]
    fn release_guard_rejects_dev_metadata() {
        assert!(!sample_dev().is_optimized_release_execution());
        let err = require_release_execution(&sample_dev())
            .unwrap_err()
            .to_string();
        assert!(err.contains("--require-release"));
        assert!(err.contains("cargo_profile=dev"));
    }

    #[test]
    fn custom_profile_not_authoritative_even_if_optimized() {
        let mut m = sample_release();
        m.cargo_profile = "release-lto".into();
        assert!(!m.is_optimized_release_execution());
    }

    #[test]
    fn opt_level_zero_not_authoritative() {
        let mut m = sample_release();
        m.opt_level = "0".into();
        assert!(!m.is_optimized_release_execution());
    }
}
