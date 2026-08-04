//! Thin equatorial disk: geometric surface `f = z` plus explicit annulus filter.

use relativity_core::{evaluate_oblate_radius, KerrParams};
use relativity_integrate::{
    CrossingDirection, EventId, EventMetadata, EventSurface, IntegrationError, LocalizedSurfaceHit,
};

/// Zero-thickness equatorial annulus in Cartesian Kerr–Schild coordinates.
///
/// Radii are geometric scene parameters only — not an ISCO model.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThinDiskGeometry {
    pub r_inner: f64,
    pub r_outer: f64,
}

impl ThinDiskGeometry {
    pub fn new(r_inner: f64, r_outer: f64) -> Self {
        Self { r_inner, r_outer }
    }

    pub fn validate(&self, params: &KerrParams) -> Result<(), IntegrationError> {
        if !self.r_inner.is_finite() {
            return Err(IntegrationError::InvalidConfig {
                field: "disk.r_inner",
            });
        }
        if !self.r_outer.is_finite() {
            return Err(IntegrationError::InvalidConfig {
                field: "disk.r_outer",
            });
        }
        let r_plus = params.outer_horizon_radius();
        if !(self.r_inner > r_plus) {
            return Err(IntegrationError::InvalidConfig {
                field: "disk.r_inner",
            });
        }
        if !(self.r_outer > self.r_inner) {
            return Err(IntegrationError::InvalidConfig {
                field: "disk.r_outer",
            });
        }
        Ok(())
    }

    #[inline]
    pub fn contains_oblate_radius(&self, r: f64) -> bool {
        r.is_finite() && r >= self.r_inner && r <= self.r_outer
    }
}

/// Event surface `f = z` with annulus classification.
#[derive(Debug, Clone)]
pub struct ThinDisk {
    pub params: KerrParams,
    pub geometry: ThinDiskGeometry,
}

impl ThinDisk {
    pub fn new(params: KerrParams, geometry: ThinDiskGeometry) -> Result<Self, IntegrationError> {
        geometry.validate(&params)?;
        Ok(Self { params, geometry })
    }
}

impl EventSurface for ThinDisk {
    fn id(&self) -> EventId {
        EventId::ThinDisk
    }

    fn value(
        &self,
        _lambda: relativity_integrate::AffineParameter,
        state: &relativity_integrate::GeodesicState,
    ) -> Result<f64, IntegrationError> {
        let z = state.position.z;
        if !z.is_finite() {
            return Err(IntegrationError::EventDomain {
                event_id: EventId::ThinDisk,
                detail: "non-finite z".into(),
            });
        }
        Ok(z)
    }

    fn crossing(&self) -> CrossingDirection {
        CrossingDirection::Any
    }

    fn classify_localized_hit(
        &self,
        hit: &LocalizedSurfaceHit,
    ) -> Result<Option<EventMetadata>, IntegrationError> {
        let r = evaluate_oblate_radius(&self.params, &hit.state.position)
            .map_err(|source| IntegrationError::EventDomain {
                event_id: EventId::ThinDisk,
                detail: format!("{source}"),
            })?
            .r;
        if !self.geometry.contains_oblate_radius(r) {
            return Ok(None);
        }
        Ok(Some(EventMetadata::ThinDisk {
            oblate_radius: r,
            crossing_side: hit.disk_crossing_side(),
        }))
    }
}
