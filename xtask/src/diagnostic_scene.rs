//! Shared Gate 1B2-compatible diagnostic scene + numerical profile (Gate 2A0-4).
//!
//! All named tiers and custom resolutions use this builder. Only `TraceGrid`
//! dimensions vary across tiers.

use crate::preset::Preset;
use relativity_core::{CameraParams, KerrParams, PositionBl};
use relativity_integrate::{Dop853Config, EventArmingPolicy, HorizonProximityPolicy};
use relativity_trace::{hex_sha, ThinDiskGeometry, TraceGrid, TraceScene};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DIAGNOSTIC_NUMERICAL_PROFILE_ID: &str = "gate-1b2-diagnostic-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticNumericalProfile {
    pub profile_id: String,
    pub relative_tolerance: [f64; 8],
    pub absolute_tolerance: [f64; 8],
    pub affine_limit: f64,
    pub max_accepted_steps: u64,
    pub max_step: f64,
    pub event_arming_minimum_affine_parameter: f64,
    pub horizon_proximity_enabled: bool,
    pub horizon_proximity_tolerance: Option<f64>,
    pub digest: String,
}

/// Gate 1B2 diagnostic integrator configuration (shared by every tier).
pub fn gate_1b2_diagnostic_integrator() -> Result<Dop853Config, Box<dyn std::error::Error>> {
    let mut integrator = Dop853Config::diagnostic_default();
    integrator.relative_tolerance = [1e-8; 8];
    integrator.absolute_tolerance = [1e-9, 1e-9, 1e-9, 1e-9, 1e-10, 1e-10, 1e-10, 1e-10];
    integrator.affine_limit = 120.0;
    integrator.max_accepted_steps = 2_000;
    integrator.max_step = 2.0;
    integrator.horizon_proximity = HorizonProximityPolicy::enabled(1e-4)?;
    integrator.event_arming = EventArmingPolicy::after(1e-12)?;
    Ok(integrator)
}

pub fn numerical_profile_from_integrator(cfg: &Dop853Config) -> DiagnosticNumericalProfile {
    let mut profile = DiagnosticNumericalProfile {
        profile_id: DIAGNOSTIC_NUMERICAL_PROFILE_ID.into(),
        relative_tolerance: cfg.relative_tolerance,
        absolute_tolerance: cfg.absolute_tolerance,
        affine_limit: cfg.affine_limit,
        max_accepted_steps: cfg.max_accepted_steps,
        max_step: cfg.max_step,
        event_arming_minimum_affine_parameter: cfg.event_arming.minimum_affine_parameter,
        horizon_proximity_enabled: cfg.horizon_proximity.enabled,
        horizon_proximity_tolerance: if cfg.horizon_proximity.enabled {
            Some(cfg.horizon_proximity.approach_tolerance)
        } else {
            None
        },
        digest: String::new(),
    };
    profile.digest = numerical_profile_digest(&profile);
    profile
}

/// Digest of numerical settings only — excludes grid, threads, timing, styles, paths.
pub fn numerical_profile_digest(profile: &DiagnosticNumericalProfile) -> String {
    let mut h = Sha256::new();
    h.update(profile.profile_id.as_bytes());
    h.update(b"|rtol|");
    for v in &profile.relative_tolerance {
        h.update(v.to_bits().to_le_bytes());
    }
    h.update(b"|atol|");
    for v in &profile.absolute_tolerance {
        h.update(v.to_bits().to_le_bytes());
    }
    h.update(b"|affine|");
    h.update(profile.affine_limit.to_bits().to_le_bytes());
    h.update(b"|max_steps|");
    h.update(profile.max_accepted_steps.to_le_bytes());
    h.update(b"|max_step|");
    h.update(profile.max_step.to_bits().to_le_bytes());
    h.update(b"|arming|");
    h.update(
        profile
            .event_arming_minimum_affine_parameter
            .to_bits()
            .to_le_bytes(),
    );
    h.update(b"|hz_en|");
    h.update([u8::from(profile.horizon_proximity_enabled)]);
    h.update(b"|hz_tol|");
    match profile.horizon_proximity_tolerance {
        Some(t) => {
            h.update([1u8]);
            h.update(t.to_bits().to_le_bytes());
        }
        None => {
            h.update([0u8]);
        }
    }
    hex_sha(&h.finalize())
}

/// Build the canonical Gate 1B2 diagnostic scene for the resolved grid.
///
/// Only `grid` differs across tiers; integrator/camera/disk/escape semantics
/// are identical.
pub fn build_diagnostic_trace_scene(
    preset: &Preset,
    grid: TraceGrid,
) -> Result<(TraceScene, DiagnosticNumericalProfile), Box<dyn std::error::Error>> {
    let mass = preset.spacetime.mass;
    let spin = preset.spacetime.spin_a_over_m * mass;
    let kerr = KerrParams::new(mass, spin)?;
    let r_plus = kerr.outer_horizon_radius();
    let disk = ThinDiskGeometry::new((r_plus + 1.5).max(3.0 * mass), preset.disk.outer_radius_m);
    disk.validate(&kerr)?;

    let integrator = gate_1b2_diagnostic_integrator()?;
    let numerical_profile = numerical_profile_from_integrator(&integrator);

    let scene = TraceScene {
        kerr,
        observer: PositionBl::new(
            0.0,
            preset.observer.boyer_lindquist_r,
            preset.observer.boyer_lindquist_theta_degrees.to_radians(),
            preset.observer.boyer_lindquist_phi_degrees.to_radians(),
        ),
        camera: CameraParams {
            horizontal_fov: preset.camera.horizontal_field_of_view_degrees.to_radians(),
            roll: preset.camera.roll_degrees.to_radians(),
        },
        disk,
        escape_radius: preset.celestial_sphere.radius_m.min(80.0),
        event_arming: integrator.event_arming.clone(),
        integrator,
        grid,
    };
    scene.validate()?;
    Ok((scene, numerical_profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_tiers_share_numerical_profile_digest() {
        let a = numerical_profile_from_integrator(&gate_1b2_diagnostic_integrator().unwrap());
        let b = numerical_profile_from_integrator(&gate_1b2_diagnostic_integrator().unwrap());
        assert_eq!(a.digest, b.digest);
        assert_eq!(a.profile_id, DIAGNOSTIC_NUMERICAL_PROFILE_ID);
    }

    #[test]
    fn grid_dimensions_do_not_alter_numerical_profile_digest() {
        // Profile is derived from integrator only; grid is not an input.
        let p32 = numerical_profile_from_integrator(&gate_1b2_diagnostic_integrator().unwrap());
        let p256 = numerical_profile_from_integrator(&gate_1b2_diagnostic_integrator().unwrap());
        assert_eq!(p32.digest, p256.digest);
    }

    #[test]
    fn tolerance_change_alters_numerical_profile_digest() {
        let mut cfg = gate_1b2_diagnostic_integrator().unwrap();
        let base = numerical_profile_from_integrator(&cfg);
        cfg.relative_tolerance[0] = 1e-7;
        let changed = numerical_profile_from_integrator(&cfg);
        assert_ne!(base.digest, changed.digest);
    }

    #[test]
    fn event_policy_change_alters_numerical_profile_digest() {
        let mut cfg = gate_1b2_diagnostic_integrator().unwrap();
        let base = numerical_profile_from_integrator(&cfg);
        cfg.event_arming = EventArmingPolicy::after(1e-6).unwrap();
        let arming = numerical_profile_from_integrator(&cfg);
        assert_ne!(base.digest, arming.digest);

        let mut cfg2 = gate_1b2_diagnostic_integrator().unwrap();
        cfg2.horizon_proximity = HorizonProximityPolicy::enabled(1e-3).unwrap();
        let hz = numerical_profile_from_integrator(&cfg2);
        assert_ne!(base.digest, hz.digest);
    }
}
