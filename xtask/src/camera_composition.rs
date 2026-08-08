//! Gate 2D3A camera/composition preset overlay (C2; D3A-A1/A2/A8).
//!
//! Strict allowlist: only `[observer]` + `[camera]` fields from a camera preset
//! may override the physical preset. Physical TOML is never mutated on disk.

use crate::preset::{CameraSection, ObserverSection, Preset};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use thiserror::Error;

pub const CAMERA_PRESET_SCHEMA_VERSION: u32 = 1;
pub const ALLOWED_MOTION: &str = "zamo";
pub const ALLOWED_PROJECTION: &str = "rectilinear";
pub const ALLOWED_LOOK_AT: &str = "black_hole_origin";

#[derive(Debug, Error)]
pub enum CameraCompositionError {
    #[error("failed to read camera preset: {0}")]
    Io(#[from] std::io::Error),
    #[error("camera preset parse/schema error: {0}")]
    Parse(String),
    #[error("camera preset validation failed: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::enum_variant_names)]
pub enum CameraRole {
    BaselineCamera,
    HeroCamera,
    CandidateCamera,
}

impl CameraRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BaselineCamera => "BASELINE_CAMERA",
            Self::HeroCamera => "HERO_CAMERA",
            Self::CandidateCamera => "CANDIDATE_CAMERA",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CameraCompositionPreset {
    pub schema_version: u32,
    pub camera_preset_id: String,
    pub role: CameraRole,
    #[serde(default)]
    pub description: Option<String>,
    pub observer: ObserverSection,
    pub camera: CameraSection,
}

impl CameraCompositionPreset {
    pub fn validate_v1(&self) -> Result<(), CameraCompositionError> {
        if self.schema_version != CAMERA_PRESET_SCHEMA_VERSION {
            return Err(CameraCompositionError::Invalid(format!(
                "unsupported schema_version {}",
                self.schema_version
            )));
        }
        if self.observer.motion != ALLOWED_MOTION {
            return Err(CameraCompositionError::Invalid(format!(
                "observer.motion must be `{ALLOWED_MOTION}` in Gate 2D3A v1 (D3A-A8)"
            )));
        }
        if self.camera.projection != ALLOWED_PROJECTION {
            return Err(CameraCompositionError::Invalid(format!(
                "camera.projection must be `{ALLOWED_PROJECTION}`"
            )));
        }
        if self.camera.look_at != ALLOWED_LOOK_AT {
            return Err(CameraCompositionError::Invalid(format!(
                "camera.look_at must be `{ALLOWED_LOOK_AT}` in Gate 2D3A v1 (D3A-A8)"
            )));
        }
        if self.camera.roll_degrees != 0.0 {
            return Err(CameraCompositionError::Invalid(
                "camera.roll_degrees must be 0.0 in Gate 2D3A v1 (D3A-A8)".into(),
            ));
        }
        for (name, v) in [
            ("boyer_lindquist_r", self.observer.boyer_lindquist_r),
            (
                "boyer_lindquist_theta_degrees",
                self.observer.boyer_lindquist_theta_degrees,
            ),
            (
                "boyer_lindquist_phi_degrees",
                self.observer.boyer_lindquist_phi_degrees,
            ),
            (
                "horizontal_field_of_view_degrees",
                self.camera.horizontal_field_of_view_degrees,
            ),
        ] {
            if !v.is_finite() {
                return Err(CameraCompositionError::Invalid(format!(
                    "{name} must be finite"
                )));
            }
        }
        if !(self.observer.boyer_lindquist_r > 0.0) {
            return Err(CameraCompositionError::Invalid(
                "boyer_lindquist_r must be > 0".into(),
            ));
        }
        if !(self.camera.horizontal_field_of_view_degrees > 0.0
            && self.camera.horizontal_field_of_view_degrees < 180.0)
        {
            return Err(CameraCompositionError::Invalid(
                "horizontal_field_of_view_degrees must be in (0, 180)".into(),
            ));
        }
        Ok(())
    }
}

pub fn load_camera_composition_preset(
    path: &Path,
) -> Result<CameraCompositionPreset, CameraCompositionError> {
    let text = std::fs::read_to_string(path)?;
    let preset: CameraCompositionPreset =
        toml::from_str(&text).map_err(|e| CameraCompositionError::Parse(e.to_string()))?;
    preset.validate_v1()?;
    Ok(preset)
}

/// D3A-A1: overlay only allowlisted observer/camera fields onto a cloned preset.
pub fn apply_camera_overlay(
    physical: &Preset,
    camera: &CameraCompositionPreset,
) -> Result<Preset, CameraCompositionError> {
    camera.validate_v1()?;
    let mut out = physical.clone();
    out.observer = camera.observer.clone();
    out.camera = camera.camera.clone();
    Ok(out)
}

pub fn camera_spec_digest(camera: &CameraCompositionPreset) -> String {
    let mut h = Sha256::new();
    h.update(b"gate-2d3a-camera-spec-v1|");
    h.update(camera.schema_version.to_le_bytes());
    h.update(camera.camera_preset_id.as_bytes());
    h.update(b"|");
    h.update(camera.role.as_str().as_bytes());
    h.update(b"|obs|");
    h.update(camera.observer.motion.as_bytes());
    h.update(camera.observer.boyer_lindquist_r.to_bits().to_le_bytes());
    h.update(
        camera
            .observer
            .boyer_lindquist_theta_degrees
            .to_bits()
            .to_le_bytes(),
    );
    h.update(
        camera
            .observer
            .boyer_lindquist_phi_degrees
            .to_bits()
            .to_le_bytes(),
    );
    h.update(b"|cam|");
    h.update(camera.camera.projection.as_bytes());
    h.update(
        camera
            .camera
            .horizontal_field_of_view_degrees
            .to_bits()
            .to_le_bytes(),
    );
    h.update(camera.camera.look_at.as_bytes());
    h.update(camera.camera.roll_degrees.to_bits().to_le_bytes());
    hex::encode(h.finalize())
}

pub fn candidate_camera_preset(
    candidate_id: &str,
    r: f64,
    theta_deg: f64,
    phi_deg: f64,
    hfov_deg: f64,
) -> CameraCompositionPreset {
    CameraCompositionPreset {
        schema_version: CAMERA_PRESET_SCHEMA_VERSION,
        camera_preset_id: candidate_id.to_string(),
        role: CameraRole::CandidateCamera,
        description: Some(format!(
            "search candidate {candidate_id}: r={r} θ={theta_deg} φ={phi_deg} hfov={hfov_deg}"
        )),
        observer: ObserverSection {
            motion: ALLOWED_MOTION.into(),
            boyer_lindquist_r: r,
            boyer_lindquist_theta_degrees: theta_deg,
            boyer_lindquist_phi_degrees: phi_deg,
        },
        camera: CameraSection {
            projection: ALLOWED_PROJECTION.into(),
            horizontal_field_of_view_degrees: hfov_deg,
            look_at: ALLOWED_LOOK_AT.into(),
            roll_degrees: 0.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::load_preset;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    #[test]
    fn baseline_overlay_matches_physical_observer_camera() {
        let physical = load_preset(&root().join("presets/gargantua-physical-v1.toml")).unwrap();
        let cam = load_camera_composition_preset(
            &root().join("presets/camera/gargantua-baseline-v1.toml"),
        )
        .unwrap();
        assert_eq!(cam.role, CameraRole::BaselineCamera);
        let overlaid = apply_camera_overlay(&physical, &cam).unwrap();
        assert_eq!(
            overlaid.observer.boyer_lindquist_r,
            physical.observer.boyer_lindquist_r
        );
        assert_eq!(
            overlaid.observer.boyer_lindquist_theta_degrees,
            physical.observer.boyer_lindquist_theta_degrees
        );
        assert_eq!(
            overlaid.observer.boyer_lindquist_phi_degrees,
            physical.observer.boyer_lindquist_phi_degrees
        );
        assert_eq!(
            overlaid.camera.horizontal_field_of_view_degrees,
            physical.camera.horizontal_field_of_view_degrees
        );
        assert_eq!(overlaid.camera.roll_degrees, physical.camera.roll_degrees);
        assert_eq!(overlaid.camera.look_at, physical.camera.look_at);
    }

    #[test]
    fn rejects_non_zero_roll() {
        let mut cam = load_camera_composition_preset(
            &root().join("presets/camera/gargantua-baseline-v1.toml"),
        )
        .unwrap();
        cam.camera.roll_degrees = 1.0;
        assert!(cam.validate_v1().is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let bad = r#"
schema_version = 1
camera_preset_id = "x"
role = "BASELINE_CAMERA"
extra = 1
[observer]
motion = "zamo"
boyer_lindquist_r = 20.0
boyer_lindquist_theta_degrees = 85.0
boyer_lindquist_phi_degrees = 0.0
[camera]
projection = "rectilinear"
horizontal_field_of_view_degrees = 50.0
look_at = "black_hole_origin"
roll_degrees = 0.0
"#;
        assert!(toml::from_str::<CameraCompositionPreset>(bad).is_err());
    }
}
