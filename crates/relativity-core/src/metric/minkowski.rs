//! Explicit Minkowski metric test implementation (signature `(-,+,+,+)`).

use crate::types::{MetricTensor, PositionKs, Vector};

/// Flat spacetime `η_μν = diag(-1, 1, 1, 1)`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MinkowskiMetric;

impl MinkowskiMetric {
    #[must_use]
    pub fn metric(&self, _pos: &PositionKs) -> MetricTensor {
        MetricTensor::minkowski()
    }

    #[must_use]
    pub fn inverse_metric(&self, _pos: &PositionKs) -> MetricTensor {
        MetricTensor::minkowski()
    }

    #[must_use]
    pub fn lower(&self, v: &Vector) -> crate::types::Covector {
        self.metric(&PositionKs::spatial(0.0, 0.0, 0.0)).mul_vec(v)
    }
}
