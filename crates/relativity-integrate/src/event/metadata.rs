//! Localized-hit classification metadata (typed; no free-form maps).

use crate::event::EventId;
use crate::state::{AffineParameter, GeodesicState};

use super::root::EventLocalizationStats;

/// Side of an equatorial thin-disk plane crossing (`f = z`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiskCrossingSide {
    UpperToLower,
    LowerToUpper,
    ExactEndpoint,
}

/// Structured metadata attached to an accepted localized event.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventMetadata {
    None,
    ThinDisk {
        oblate_radius: f64,
        crossing_side: DiskCrossingSide,
    },
}

/// Localized root candidate presented to [`super::EventSurface::classify_localized_hit`].
#[derive(Debug, Clone, PartialEq)]
pub struct LocalizedSurfaceHit {
    pub event_id: EventId,
    pub lambda: AffineParameter,
    pub state: GeodesicState,
    pub event_value: f64,
    pub localization: EventLocalizationStats,
    /// Endpoint samples that formed the accepted-step bracket.
    pub f0: f64,
    pub f1: f64,
}

impl LocalizedSurfaceHit {
    /// Infer thin-disk crossing side from endpoint event values (`f = z`).
    pub fn disk_crossing_side(&self) -> DiskCrossingSide {
        if self.f1 == 0.0 {
            DiskCrossingSide::ExactEndpoint
        } else if self.f0 > 0.0 && self.f1 < 0.0 {
            DiskCrossingSide::UpperToLower
        } else if self.f0 < 0.0 && self.f1 > 0.0 {
            DiskCrossingSide::LowerToUpper
        } else if self.event_value == 0.0 {
            DiskCrossingSide::ExactEndpoint
        } else if self.f0 > self.f1 {
            DiskCrossingSide::UpperToLower
        } else {
            DiskCrossingSide::LowerToUpper
        }
    }
}
