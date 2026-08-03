//! Escape sphere: `f = r_oblate - r_escape`, increasing.

use relativity_core::{evaluate_oblate_radius, KerrParams};

use crate::error::IntegrationError;
use crate::state::{AffineParameter, GeodesicState};

use super::surface::{CrossingDirection, EventId, EventSurface};

#[derive(Debug, Clone)]
pub struct EscapeSphere {
    pub params: KerrParams,
    pub r_escape: f64,
}

impl EscapeSphere {
    pub fn new(params: KerrParams, r_escape: f64) -> Result<Self, IntegrationError> {
        if !r_escape.is_finite() || r_escape <= 0.0 {
            return Err(IntegrationError::InvalidConfig { field: "r_escape" });
        }
        Ok(Self { params, r_escape })
    }
}

impl EventSurface for EscapeSphere {
    fn id(&self) -> EventId {
        EventId::EscapeSphere
    }

    fn value(
        &self,
        _lambda: AffineParameter,
        state: &GeodesicState,
    ) -> Result<f64, IntegrationError> {
        let r = evaluate_oblate_radius(&self.params, &state.position)
            .map_err(|source| IntegrationError::EventDomain {
                event_id: EventId::EscapeSphere,
                detail: format!("{source}"),
            })?
            .r;
        Ok(r - self.r_escape)
    }

    fn crossing(&self) -> CrossingDirection {
        CrossingDirection::Increasing
    }
}
