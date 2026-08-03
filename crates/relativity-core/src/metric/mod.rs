//! Spacetime metrics for Gate 1A.

mod derivatives;
mod kerr_schild;
mod minkowski;

pub use derivatives::{
    inverse_metric_spatial_derivatives, InverseMetricDerivatives, SpatialDerivativeIndex,
};
pub use kerr_schild::{
    evaluate_kerr_schild, lower_vector, matrix_inverse_oracle, raise_covector, KerrSchildQuantities,
};
pub use minkowski::MinkowskiMetric;
