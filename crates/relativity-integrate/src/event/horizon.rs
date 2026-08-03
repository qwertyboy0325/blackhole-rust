//! Outer horizon: `f = r_oblate - r_+`, decreasing.

use relativity_core::{evaluate_oblate_radius, KerrParams};

use crate::error::IntegrationError;
use crate::state::{AffineParameter, GeodesicState};

use super::surface::{CrossingDirection, EventId, EventSurface};

#[derive(Debug, Clone)]
pub struct OuterHorizon {
    pub params: KerrParams,
}

impl OuterHorizon {
    pub fn new(params: KerrParams) -> Self {
        Self { params }
    }

    pub fn r_plus(&self) -> f64 {
        self.params.outer_horizon_radius()
    }
}

impl EventSurface for OuterHorizon {
    fn id(&self) -> EventId {
        EventId::OuterHorizon
    }

    fn value(
        &self,
        _lambda: AffineParameter,
        state: &GeodesicState,
    ) -> Result<f64, IntegrationError> {
        let r = evaluate_oblate_radius(&self.params, &state.position)
            .map_err(|source| IntegrationError::EventDomain {
                event_id: EventId::OuterHorizon,
                detail: format!("{source}"),
            })?
            .r;
        Ok(r - self.r_plus())
    }

    fn crossing(&self) -> CrossingDirection {
        CrossingDirection::Decreasing
    }
}
