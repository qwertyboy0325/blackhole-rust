//! Strict TOML preset schema for Gate 1A (unknown fields rejected).
//!
//! Fields irrelevant to Gate 1A are still parsed and retained so the schema
//! rejects unknown keys without implementing deferred renderer features.

#![allow(dead_code)]

use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("failed to read preset: {0}")]
    Io(#[from] std::io::Error),
    #[error("preset parse/schema error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub schema_version: u32,
    pub name: String,
    pub description: String,
    pub provenance: Provenance,
    pub spacetime: Spacetime,
    pub geodesics: Geodesics,
    pub observer: ObserverSection,
    pub camera: CameraSection,
    pub disk: DiskSection,
    pub celestial_sphere: CelestialSphere,
    pub spectrum: Spectrum,
    pub diagnostic_render: DiagnosticRender,
    pub artifacts: Artifacts,
    pub gpu: Gpu,
    /// Gate 2C0 physical radiometry knobs (absent on diagnostic presets).
    #[serde(default)]
    pub physical: Option<PhysicalSection>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub physical_reference: String,
    pub published_spin_context: String,
    pub artistic_disclaimer: String,
    pub unknowns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spacetime {
    pub model: String,
    pub units: String,
    pub mass: f64,
    pub spin_a_over_m: f64,
    pub integration_coordinates: String,
    pub reporting_coordinates: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geodesics {
    pub formulation: String,
    pub integrator: String,
    pub scalar: String,
    pub relative_tolerance: f64,
    pub absolute_tolerance_position: f64,
    pub absolute_tolerance_momentum: f64,
    pub maximum_steps: u64,
    pub maximum_backward_affine_parameter: f64,
    pub event_localization_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverSection {
    pub motion: String,
    pub boyer_lindquist_r: f64,
    pub boyer_lindquist_theta_degrees: f64,
    pub boyer_lindquist_phi_degrees: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraSection {
    pub projection: String,
    pub horizontal_field_of_view_degrees: f64,
    pub look_at: String,
    pub roll_degrees: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskSection {
    pub geometry: String,
    pub inner_edge: String,
    pub outer_radius_m: f64,
    pub optical_model: String,
    pub velocity_model: String,
    pub emission_model: String,
    pub emission_claim: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CelestialSphere {
    pub radius_m: f64,
    pub texture: String,
    pub seam: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spectrum {
    pub representation: String,
    pub wavelength_min_nm: f64,
    pub wavelength_max_nm: f64,
    pub wavelength_step_nm: f64,
    pub sampling_status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticRender {
    pub width: u32,
    pub height: u32,
    pub samples_per_pixel: u32,
    pub sampling: String,
    pub seed: u64,
    pub thread_assembly: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifacts {
    pub image_format: String,
    pub scene_linear_rgb: String,
    pub report_format: String,
    pub digest: String,
    pub presentation_transform: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gpu {
    pub enabled: bool,
    pub status: String,
}

/// Gate 2C0 physical scale / accretion / emission provenance.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalSection {
    pub mass_solar: f64,
    pub mdot_kg_s: f64,
    pub mass_claim: String,
    pub flux_model: String,
    pub temperature_model: String,
    pub emission_model: String,
    pub emission_claim: String,
}

pub fn load_preset(path: &Path) -> Result<Preset, PresetError> {
    let text = std::fs::read_to_string(path)?;
    let preset: Preset = toml::from_str(&text).map_err(|e| PresetError::Parse(e.to_string()))?;
    if preset.schema_version != 1 {
        return Err(PresetError::Parse(format!(
            "unsupported schema_version {}",
            preset.schema_version
        )));
    }
    Ok(preset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_baseline_and_rejects_unknown() {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../presets/gargantua-baseline.toml");
        let p = load_preset(&root).unwrap();
        assert_eq!(p.name, "gargantua-baseline");
        let bad = "schema_version=1\nname=\"x\"\nextra=1\n";
        assert!(toml::from_str::<Preset>(bad).is_err());
    }
}
