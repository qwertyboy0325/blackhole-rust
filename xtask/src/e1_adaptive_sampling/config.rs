//! Strict E1 experiment configuration.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::path::Path;

pub const REQUIRED_LOCK_DIGEST: &str =
    "647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb";
pub const REQUIRED_BASELINE_ORACLE_DIGEST: &str =
    "ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383";
pub const APPROVED_BASE: &str = "86dd63dc537d5e4f41f5e798f5f30a4e3694558e";

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct E1Config {
    pub schema_version: u32,
    pub experiment_id: String,
    pub oracle_manifest: String,
    pub oracle_lock: String,
    pub source_leaf_sizes: Vec<u32>,
    pub crop_leaf_sizes: Vec<u32>,
    pub luma_stop_scale: f64,
    pub outcome_priority: f64,
    pub g_stop_scale: f64,
    pub radiance_stop_scale: f64,
    pub cost_stop_scale: f64,
    pub component_cap: f64,
    pub reconstruction: String,
    pub probe_stencil: String,
}

impl E1Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let bytes = std::fs::read(path)?;
        let cfg: Self = toml::from_str(std::str::from_utf8(&bytes)?)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.schema_version != 1 {
            return Err("E1 config schema_version must be 1".into());
        }
        if self.experiment_id != "e1-physics-aware-adaptive-quadtree-v1" {
            return Err("unexpected experiment_id".into());
        }
        if self.reconstruction != "leaf-local-nearest-v1" {
            return Err("unsupported reconstruction policy".into());
        }
        if self.probe_stencil != "corners-center-child-centers-v1" {
            return Err("unsupported probe stencil".into());
        }
        if self.source_leaf_sizes != [32, 16, 8, 4, 2] {
            return Err("source_leaf_sizes must be [32,16,8,4,2]".into());
        }
        if self.crop_leaf_sizes != [16, 8, 4, 2, 1] {
            return Err("crop_leaf_sizes must be [16,8,4,2,1]".into());
        }
        for (name, v) in [
            ("luma_stop_scale", self.luma_stop_scale),
            ("outcome_priority", self.outcome_priority),
            ("g_stop_scale", self.g_stop_scale),
            ("radiance_stop_scale", self.radiance_stop_scale),
            ("cost_stop_scale", self.cost_stop_scale),
            ("component_cap", self.component_cap),
        ] {
            if !v.is_finite() || v <= 0.0 {
                return Err(format!("{name} must be finite and > 0").into());
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, Box<dyn Error>> {
        let json = serde_json::to_vec(self)?;
        Ok(hex::encode(Sha256::digest(&json)))
    }
}

// Serde for digest: need Serialize
impl serde::Serialize for E1Config {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("E1Config", 15)?;
        s.serialize_field("schema_version", &self.schema_version)?;
        s.serialize_field("experiment_id", &self.experiment_id)?;
        s.serialize_field("oracle_manifest", &self.oracle_manifest)?;
        s.serialize_field("oracle_lock", &self.oracle_lock)?;
        s.serialize_field("source_leaf_sizes", &self.source_leaf_sizes)?;
        s.serialize_field("crop_leaf_sizes", &self.crop_leaf_sizes)?;
        s.serialize_field("luma_stop_scale", &self.luma_stop_scale)?;
        s.serialize_field("outcome_priority", &self.outcome_priority)?;
        s.serialize_field("g_stop_scale", &self.g_stop_scale)?;
        s.serialize_field("radiance_stop_scale", &self.radiance_stop_scale)?;
        s.serialize_field("cost_stop_scale", &self.cost_stop_scale)?;
        s.serialize_field("component_cap", &self.component_cap)?;
        s.serialize_field("reconstruction", &self.reconstruction)?;
        s.serialize_field("probe_stencil", &self.probe_stencil)?;
        s.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_schema_exact() {
        let cfg = E1Config::load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../experiments/e1-adaptive-sampling/config-v1.toml"
        )))
        .unwrap();
        assert_eq!(cfg.schema_version, 1);
        assert_eq!(cfg.source_leaf_sizes, vec![32, 16, 8, 4, 2]);
    }

    #[test]
    fn unknown_fields_reject() {
        let bad = r#"
schema_version = 1
experiment_id = "e1-physics-aware-adaptive-quadtree-v1"
oracle_manifest = "m"
oracle_lock = "l"
source_leaf_sizes = [32, 16, 8, 4, 2]
crop_leaf_sizes = [16, 8, 4, 2, 1]
luma_stop_scale = 0.5
outcome_priority = 8.0
g_stop_scale = 0.125
radiance_stop_scale = 0.5
cost_stop_scale = 0.5
component_cap = 8.0
reconstruction = "leaf-local-nearest-v1"
probe_stencil = "corners-center-child-centers-v1"
extra = true
"#;
        assert!(toml::from_str::<E1Config>(bad).is_err());
    }
}
