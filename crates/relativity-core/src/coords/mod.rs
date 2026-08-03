//! Checked coordinate maps: Boyer–Lindquist ↔ spherical KS ↔ Cartesian KS.
//!
//! Convention: **ingoing Kerr–Schild** compatible with the project's
//! `ℓ_μ = (1, (rx+ay)/(r²+a²), (ry−ax)/(r²+a²), z/r)` null covector.

mod boyer_lindquist;
mod kerr_schild_spherical;

pub use boyer_lindquist::{
    bl_metric, bl_to_ks_position, covector_bl_to_ks, covector_ks_to_bl,
    jacobian_cartesian_ks_from_bl, ks_to_bl_position, vector_bl_to_ks, vector_ks_to_bl,
};
pub use kerr_schild_spherical::{
    cartesian_from_spherical_ks, jacobian_cartesian_from_spherical_ks, spherical_ks_from_cartesian,
    PositionSphericalKs,
};
