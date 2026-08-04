//! Explicit event-surface registration sets for diagnostic camera traces (Gate 2A2).

use serde::{Deserialize, Serialize};

/// Which event surfaces are registered for a diagnostic grid trace.
///
/// `HorizonEscapeOnly` is a **disk-omitted celestial diagnostic**: the thin-disk
/// event is not registered. It is not a transparent physical disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TraceSurfaceSet {
    OpaqueDiskHorizonEscape,
    HorizonEscapeOnly,
}

impl TraceSurfaceSet {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpaqueDiskHorizonEscape => "opaque-disk-horizon-escape",
            Self::HorizonEscapeOnly => "horizon-escape-only",
        }
    }

    /// Stable project-owned digest tag (not Debug/Display/serde).
    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::OpaqueDiskHorizonEscape => "trace-surface-set:opaque-disk-horizon-escape",
            Self::HorizonEscapeOnly => "trace-surface-set:horizon-escape-only",
        }
    }

    pub const fn filename_stem(self) -> &'static str {
        self.as_str()
    }
}
