use crate::error::IntegrationError;
use crate::state::{AffineParameter, GeodesicState};

use super::metadata::{EventMetadata, LocalizedSurfaceHit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventId {
    OuterHorizon,
    EscapeSphere,
    ThinDisk,
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

    /// Post-localization filter. `Ok(None)` rejects the root (continue integration).
    ///
    /// Default: accept with [`EventMetadata::None`].
    /// `ThinDisk` overrides to enforce the oblate-radius annulus.
    fn classify_localized_hit(
        &self,
        _hit: &LocalizedSurfaceHit,
    ) -> Result<Option<EventMetadata>, IntegrationError> {
        Ok(Some(EventMetadata::None))
    }
}

/// Floating-point comparison policy for an exact endpoint root: `f == 0.0`
/// under IEEE-754 exact equality (no absolute/relative tolerance).
#[inline]
pub fn is_exact_root(f: f64) -> bool {
    f == 0.0
}

/// True if `(f0, f1)` is an eligible **exact** event crossing for `dir`.
///
/// Eligible only when:
/// - strict sign change (`f0 * f1 < 0`) in the requested direction; or
/// - exact endpoint root (`f1 == 0.0`) approached from the correct side
///   (`f0` has the incoming sign), or both endpoints exactly zero.
///
/// `event_value_tolerance` is **not** used here — it is a localization
/// convergence tolerance only.
///
/// Not supported: identical-sign proximity, tangent contact, discontinuous
/// event functions.
pub fn is_eligible_crossing(f0: f64, f1: f64, dir: CrossingDirection) -> bool {
    if !f0.is_finite() || !f1.is_finite() {
        return false;
    }
    let sign_change = f0 * f1 < 0.0;
    let exact_end = is_exact_root(f1);
    let exact_both = is_exact_root(f0) && is_exact_root(f1);
    if !sign_change && !exact_end && !exact_both {
        return false;
    }
    match dir {
        CrossingDirection::Any => true,
        CrossingDirection::Increasing => {
            if sign_change {
                f0 < 0.0 && f1 > 0.0
            } else if exact_end {
                f0 < 0.0 || exact_both
            } else {
                false
            }
        }
        CrossingDirection::Decreasing => {
            if sign_change {
                f0 > 0.0 && f1 < 0.0
            } else if exact_end {
                f0 > 0.0 || exact_both
            } else {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_identical_sign_proximity() {
        assert!(!is_eligible_crossing(1e-16, 1e-16, CrossingDirection::Any));
        assert!(!is_eligible_crossing(
            1.0,
            1e-20,
            CrossingDirection::Decreasing
        ));
        assert!(!is_eligible_crossing(
            -1.0,
            -1e-20,
            CrossingDirection::Increasing
        ));
    }

    #[test]
    fn accepts_strict_sign_change() {
        assert!(is_eligible_crossing(
            -1.0,
            1.0,
            CrossingDirection::Increasing
        ));
        assert!(is_eligible_crossing(
            1.0,
            -1.0,
            CrossingDirection::Decreasing
        ));
        assert!(!is_eligible_crossing(
            -1.0,
            1.0,
            CrossingDirection::Decreasing
        ));
    }

    #[test]
    fn accepts_exact_endpoint_zero() {
        assert!(is_eligible_crossing(
            1.0,
            0.0,
            CrossingDirection::Decreasing
        ));
        assert!(is_eligible_crossing(
            -1.0,
            0.0,
            CrossingDirection::Increasing
        ));
        assert!(!is_eligible_crossing(
            1.0,
            0.0,
            CrossingDirection::Increasing
        ));
    }
}
