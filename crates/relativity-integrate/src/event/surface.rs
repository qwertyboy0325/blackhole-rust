use crate::error::IntegrationError;
use crate::state::{AffineParameter, GeodesicState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventId {
    OuterHorizon,
    EscapeSphere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CrossingDirection {
    Any,
    Increasing,
    Decreasing,
}

pub trait EventSurface {
    fn id(&self) -> EventId;

    fn value(
        &self,
        lambda: AffineParameter,
        state: &GeodesicState,
    ) -> Result<f64, IntegrationError>;

    fn crossing(&self) -> CrossingDirection;
}

/// True if `(f0, f1)` is an eligible crossing for `dir`.
///
/// Primary: strict sign change (`f0 * f1 < 0`) in the requested direction.
///
/// Also eligible: directional hit of the surface within `value_tol` at the
/// accepted endpoint (`|f1| <= value_tol`) when the step approached from the
/// correct side. This covers f64 horizon approach where `r → r₊⁺` without a
/// representable interior sample (not tangent contact: `|f0|` remains large).
///
/// Not supported: tangent contact, identical-sign endpoints with `|f1| > tol`,
/// discontinuous event functions.
pub fn is_eligible_crossing(f0: f64, f1: f64, dir: CrossingDirection) -> bool {
    is_eligible_crossing_tol(f0, f1, dir, 0.0)
}

pub fn is_eligible_crossing_tol(f0: f64, f1: f64, dir: CrossingDirection, value_tol: f64) -> bool {
    if !f0.is_finite() || !f1.is_finite() {
        return false;
    }
    let sign_change = f0 * f1 < 0.0;
    let endpoint_hit = value_tol > 0.0 && f1.abs() <= value_tol && f0.abs() > value_tol;
    if !sign_change && !endpoint_hit {
        return false;
    }
    match dir {
        CrossingDirection::Any => true,
        CrossingDirection::Increasing => f0 < f1 && (sign_change || (f0 < 0.0 && endpoint_hit)),
        CrossingDirection::Decreasing => f0 > f1 && (sign_change || (f0 > 0.0 && endpoint_hit)),
    }
}
