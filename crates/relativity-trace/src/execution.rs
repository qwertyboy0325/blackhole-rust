//! Project-owned camera-grid execution modes (serial / bounded parallel).

use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;

/// High-level execution mode label recorded in worker metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceExecutionMode {
    Serial,
    Parallel,
}

impl TraceExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Serial => "serial",
            Self::Parallel => "parallel",
        }
    }
}

/// Requested execution configuration for camera-grid tracing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceExecution {
    Serial,
    Parallel { threads: NonZeroUsize },
}

impl TraceExecution {
    pub fn serial() -> Self {
        Self::Serial
    }

    pub fn parallel(threads: NonZeroUsize) -> Self {
        Self::Parallel { threads }
    }

    pub fn mode(self) -> TraceExecutionMode {
        match self {
            Self::Serial => TraceExecutionMode::Serial,
            Self::Parallel { .. } => TraceExecutionMode::Parallel,
        }
    }

    pub fn thread_count(self) -> usize {
        match self {
            Self::Serial => 1,
            Self::Parallel { threads } => threads.get(),
        }
    }

    pub fn scheduler(self) -> &'static str {
        match self {
            Self::Serial => "serial-row-major",
            Self::Parallel { .. } => "rayon-indexed-work-stealing",
        }
    }

    pub fn metadata(self) -> TraceExecutionMetadata {
        TraceExecutionMetadata {
            mode: self.mode(),
            thread_count: self.thread_count(),
            scheduler: self.scheduler().into(),
        }
    }
}

/// Worker-emitted execution facts for the map that was actually traced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceExecutionMetadata {
    pub mode: TraceExecutionMode,
    pub thread_count: usize,
    pub scheduler: String,
}

impl TraceExecutionMetadata {
    pub fn serial() -> Self {
        TraceExecution::Serial.metadata()
    }
}
