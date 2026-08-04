//! Relativity core: Kerr–Schild geometry, tetrads, and null-ray initialization.
//!
//! No filesystem, TOML, image, GUI, GPU, or async dependencies.
//! Gate 1A scope only — no production geodesic integrator.

#![forbid(unsafe_code)]

pub mod coords;
pub mod corpus;
pub mod error;
pub mod hamiltonian;
pub mod kerr;
pub mod metric;
pub mod observer;
pub mod radius;
pub mod ray_init;
pub mod types;

pub use coords::{
    bl_metric, bl_to_ks_position, cartesian_from_spherical_ks, covector_bl_to_ks,
    covector_ks_to_bl, jacobian_cartesian_ks_from_bl, ks_to_bl_position,
    spherical_ks_direction_from_cartesian, spherical_ks_from_cartesian, vector_bl_to_ks,
    vector_ks_to_bl, PositionSphericalKs, SphericalKsAzimuthStatus, SphericalKsDirection,
    AXIS_SIN_FLOOR,
};
pub use corpus::{stratified_corpus, CorpusPoint, CorpusTag, ExpectedOutcome, CORPUS_SEED};
pub use error::{CoreError, DomainReason, EvalStatus};
pub use hamiltonian::{evaluate_hamiltonian, HamiltonianEval};
pub use kerr::KerrParams;
pub use metric::{
    evaluate_kerr_schild, inverse_metric_spatial_derivatives, lower_vector, matrix_inverse_oracle,
    raise_covector, InverseMetricDerivatives, KerrSchildQuantities, MatrixInverseOracle,
    MinkowskiMetric, SpatialDerivativeIndex,
};
pub use observer::{check_tetrad, minkowski_static_observer, zamo_observer, Observer, Tetrad};
pub use radius::{evaluate_oblate_radius, OblateRadius};
pub use ray_init::{initialize_rectilinear_ray, CameraParams, InitialRay, SensorCoord};
pub use types::{
    identity_residual, Covector, LocalComponents, MetricTensor, PositionBl, PositionKs, RawMatrix4,
    Vector,
};
