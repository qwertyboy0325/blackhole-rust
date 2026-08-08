//! Gate 2D3A deterministic camera search (D3A-A3/A4/A5/A6/A8).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraSearchSpecFile {
    pub schema_version: u32,
    pub search_spec_id: String,
    pub primary_family: String,
    pub neighbor_families: Vec<String>,
    pub smoke_width: u32,
    pub smoke_height: u32,
    pub gate_width: u32,
    pub gate_height: u32,
    pub shortlist_n: usize,
    pub max_candidates: usize,
    pub parameterization_note: String,
    pub hfov_degrees: Vec<f64>,
    pub r_over_m: Vec<f64>,
    pub theta_degrees: Vec<f64>,
    pub phi_degrees: Vec<f64>,
    pub search_guidance: SearchGuidance,
    pub smoke_hard_invalidity: SmokeHardInvalidity,
    pub gate_hard_invalidity: GateHardInvalidity,
    pub shortlist: ShortlistRule,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchGuidance {
    pub disk_hit_mid: f64,
    pub escaped_mid: f64,
    pub horizon_mid: f64,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SmokeHardInvalidity {
    pub max_affine_or_failed: u64,
    pub max_disk_hit_fraction: f64,
    pub min_escaped_fraction: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GateHardInvalidity {
    pub max_affine_or_failed: u64,
    pub max_disk_hit_fraction: f64,
    pub min_disk_hit_fraction: f64,
    pub min_escaped_fraction: f64,
    pub max_escaped_fraction: f64,
    pub min_horizon_fraction: f64,
    pub max_horizon_fraction: f64,
    pub min_disk_plus_horizon_fraction: f64,
    pub min_shadow_edge_margin_frac: f64,
    pub min_highlight_edge_margin_frac: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ShortlistRule {
    pub rule_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchCandidate {
    pub id: String,
    pub index: usize,
    pub r_over_m: f64,
    pub theta_degrees: f64,
    pub phi_degrees: f64,
    pub hfov_degrees: f64,
    pub family_hint: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompositionMetrics {
    pub width: u32,
    pub height: u32,
    pub disk_hit_count: u64,
    pub escaped_count: u64,
    pub horizon_count: u64,
    pub affine_limit_count: u64,
    pub failed_count: u64,
    pub disk_hit_fraction: f64,
    pub escaped_fraction: f64,
    pub horizon_fraction: f64,
    pub shadow_bbox: Option<[u32; 4]>,
    pub shadow_centroid: Option<[f64; 2]>,
    pub shadow_edge_margin_frac: f64,
    pub highlight_col: Option<u32>,
    pub highlight_row: Option<u32>,
    pub highlight_edge_margin_frac: f64,
    pub presentation_frame_digest: String,
    pub scene_appearance_digest: String,
    pub source_physical_color_digest: String,
    pub camera_spec_digest: String,
    pub authority_label: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateStageResult {
    pub candidate: SearchCandidate,
    pub smoke: Option<CompositionMetrics>,
    pub smoke_valid: bool,
    pub smoke_reject_reason: Option<String>,
    pub gate: Option<CompositionMetrics>,
    pub gate_valid: bool,
    pub gate_reject_reason: Option<String>,
    pub shortlist_key: Option<ShortlistKey>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ShortlistKey {
    pub abs_disk_from_mid: f64,
    pub abs_escaped_from_mid: f64,
    pub neg_shadow_margin: f64,
    pub candidate_id: String,
}

impl PartialOrd for ShortlistKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ShortlistKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.abs_disk_from_mid
            .total_cmp(&other.abs_disk_from_mid)
            .then_with(|| {
                self.abs_escaped_from_mid
                    .total_cmp(&other.abs_escaped_from_mid)
            })
            .then_with(|| self.neg_shadow_margin.total_cmp(&other.neg_shadow_margin))
            .then_with(|| self.candidate_id.cmp(&other.candidate_id))
    }
}

impl Eq for ShortlistKey {}

pub fn load_camera_search_spec(
    path: &Path,
) -> Result<CameraSearchSpecFile, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let spec: CameraSearchSpecFile = toml::from_str(&text)?;
    if spec.schema_version != 1 {
        return Err(format!("unsupported search schema_version {}", spec.schema_version).into());
    }
    if spec.search_guidance.label != "SEARCH_GUIDANCE_NOT_GATE_TRUTH" {
        return Err("search_guidance.label must be SEARCH_GUIDANCE_NOT_GATE_TRUTH (D3A-A6)".into());
    }
    Ok(spec)
}

pub fn expand_candidates(
    spec: &CameraSearchSpecFile,
) -> Result<Vec<SearchCandidate>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    let mut index = 0usize;
    for &hfov in &spec.hfov_degrees {
        for &r in &spec.r_over_m {
            for &theta in &spec.theta_degrees {
                for &phi in &spec.phi_degrees {
                    let id = format!("c{index:03}");
                    let family_hint = family_hint(r, theta, phi, hfov);
                    out.push(SearchCandidate {
                        id,
                        index,
                        r_over_m: r,
                        theta_degrees: theta,
                        phi_degrees: phi,
                        hfov_degrees: hfov,
                        family_hint,
                    });
                    index += 1;
                }
            }
        }
    }
    if out.len() > spec.max_candidates {
        return Err(format!(
            "expanded {} candidates > max_candidates {}",
            out.len(),
            spec.max_candidates
        )
        .into());
    }
    if out.len()
        != spec.hfov_degrees.len()
            * spec.r_over_m.len()
            * spec.theta_degrees.len()
            * spec.phi_degrees.len()
    {
        return Err("candidate expansion size mismatch".into());
    }
    Ok(out)
}

fn family_hint(r: f64, theta: f64, phi: f64, hfov: f64) -> &'static str {
    if (r - 20.0).abs() < 1e-12
        && (theta - 85.0).abs() < 1e-12
        && (phi - 0.0).abs() < 1e-12
        && (hfov - 50.0).abs() < 1e-12
    {
        return "F1_BASELINE";
    }
    if hfov >= 70.0 || r >= 36.0 {
        return "F3_WIDER_ENV";
    }
    if theta >= 84.0 && phi.abs() >= 45.0 {
        return "F4_EDGE_ON_DRAMATIC";
    }
    if hfov <= 58.0 && r <= 28.0 {
        return "F2_HERO_CLOSE";
    }
    "F5_BALANCED_POSTER"
}

pub fn camera_search_spec_digest(spec: &CameraSearchSpecFile) -> String {
    let mut h = Sha256::new();
    h.update(b"gate-2d3a-camera-search-spec-v1|");
    h.update(spec.schema_version.to_le_bytes());
    h.update(spec.search_spec_id.as_bytes());
    h.update(b"|");
    h.update(spec.primary_family.as_bytes());
    h.update(b"|neighbors|");
    for n in &spec.neighbor_families {
        h.update(n.as_bytes());
        h.update(b",");
    }
    h.update(b"|note|");
    h.update(spec.parameterization_note.as_bytes());
    h.update(b"|smoke|");
    h.update(spec.smoke_width.to_le_bytes());
    h.update(spec.smoke_height.to_le_bytes());
    h.update(b"|gate|");
    h.update(spec.gate_width.to_le_bytes());
    h.update(spec.gate_height.to_le_bytes());
    h.update(b"|shortlist_n|");
    h.update((spec.shortlist_n as u64).to_le_bytes());
    h.update(b"|max|");
    h.update((spec.max_candidates as u64).to_le_bytes());
    h.update(b"|hfov|");
    for v in &spec.hfov_degrees {
        h.update(v.to_bits().to_le_bytes());
    }
    h.update(b"|r|");
    for v in &spec.r_over_m {
        h.update(v.to_bits().to_le_bytes());
    }
    h.update(b"|theta|");
    for v in &spec.theta_degrees {
        h.update(v.to_bits().to_le_bytes());
    }
    h.update(b"|phi|");
    for v in &spec.phi_degrees {
        h.update(v.to_bits().to_le_bytes());
    }
    h.update(b"|guidance|");
    h.update(spec.search_guidance.disk_hit_mid.to_bits().to_le_bytes());
    h.update(spec.search_guidance.escaped_mid.to_bits().to_le_bytes());
    h.update(spec.search_guidance.horizon_mid.to_bits().to_le_bytes());
    h.update(spec.search_guidance.label.as_bytes());
    h.update(b"|smoke_inv|");
    h.update(
        spec.smoke_hard_invalidity
            .max_affine_or_failed
            .to_le_bytes(),
    );
    h.update(
        spec.smoke_hard_invalidity
            .max_disk_hit_fraction
            .to_bits()
            .to_le_bytes(),
    );
    h.update(
        spec.smoke_hard_invalidity
            .min_escaped_fraction
            .to_bits()
            .to_le_bytes(),
    );
    h.update(b"|gate_inv|");
    let g = &spec.gate_hard_invalidity;
    h.update(g.max_affine_or_failed.to_le_bytes());
    for v in [
        g.max_disk_hit_fraction,
        g.min_disk_hit_fraction,
        g.min_escaped_fraction,
        g.max_escaped_fraction,
        g.min_horizon_fraction,
        g.max_horizon_fraction,
        g.min_disk_plus_horizon_fraction,
        g.min_shadow_edge_margin_frac,
        g.min_highlight_edge_margin_frac,
    ] {
        h.update(v.to_bits().to_le_bytes());
    }
    h.update(b"|shortlist|");
    h.update(spec.shortlist.rule_id.as_bytes());
    // Include expanded candidate IDs for bit-stability of the search universe.
    if let Ok(cands) = expand_candidates(spec) {
        h.update(b"|candidates|");
        for c in &cands {
            h.update(c.id.as_bytes());
            h.update(c.r_over_m.to_bits().to_le_bytes());
            h.update(c.theta_degrees.to_bits().to_le_bytes());
            h.update(c.phi_degrees.to_bits().to_le_bytes());
            h.update(c.hfov_degrees.to_bits().to_le_bytes());
        }
    }
    hex::encode(h.finalize())
}

pub fn smoke_reject_reason(m: &CompositionMetrics, rules: &SmokeHardInvalidity) -> Option<String> {
    let bad = m.affine_limit_count + m.failed_count;
    if bad > rules.max_affine_or_failed {
        return Some(format!("affine_or_failed={bad}"));
    }
    if m.disk_hit_fraction > rules.max_disk_hit_fraction {
        return Some(format!("disk_hit_fraction={}", m.disk_hit_fraction));
    }
    if m.escaped_fraction < rules.min_escaped_fraction {
        return Some(format!("escaped_fraction={}", m.escaped_fraction));
    }
    None
}

pub fn gate_reject_reason(m: &CompositionMetrics, rules: &GateHardInvalidity) -> Option<String> {
    let bad = m.affine_limit_count + m.failed_count;
    if bad > rules.max_affine_or_failed {
        return Some(format!("affine_or_failed={bad}"));
    }
    if m.disk_hit_fraction > rules.max_disk_hit_fraction
        || m.disk_hit_fraction < rules.min_disk_hit_fraction
    {
        return Some(format!("disk_hit_fraction={}", m.disk_hit_fraction));
    }
    if m.escaped_fraction < rules.min_escaped_fraction
        || m.escaped_fraction > rules.max_escaped_fraction
    {
        return Some(format!("escaped_fraction={}", m.escaped_fraction));
    }
    if m.horizon_fraction < rules.min_horizon_fraction
        || m.horizon_fraction > rules.max_horizon_fraction
    {
        return Some(format!("horizon_fraction={}", m.horizon_fraction));
    }
    if m.disk_hit_fraction + m.horizon_fraction < rules.min_disk_plus_horizon_fraction {
        return Some(format!(
            "disk_plus_horizon={}",
            m.disk_hit_fraction + m.horizon_fraction
        ));
    }
    if m.shadow_edge_margin_frac < rules.min_shadow_edge_margin_frac {
        return Some(format!(
            "shadow_edge_margin_frac={}",
            m.shadow_edge_margin_frac
        ));
    }
    if m.highlight_edge_margin_frac < rules.min_highlight_edge_margin_frac {
        return Some(format!(
            "highlight_edge_margin_frac={}",
            m.highlight_edge_margin_frac
        ));
    }
    None
}

pub fn shortlist_key(m: &CompositionMetrics, guidance: &SearchGuidance, id: &str) -> ShortlistKey {
    ShortlistKey {
        abs_disk_from_mid: (m.disk_hit_fraction - guidance.disk_hit_mid).abs(),
        abs_escaped_from_mid: (m.escaped_fraction - guidance.escaped_mid).abs(),
        neg_shadow_margin: -m.shadow_edge_margin_frac,
        candidate_id: id.to_string(),
    }
}

pub fn select_shortlist(results: &[CandidateStageResult], n: usize) -> Vec<&CandidateStageResult> {
    let mut valid: Vec<&CandidateStageResult> = results
        .iter()
        .filter(|r| r.gate_valid && r.shortlist_key.is_some())
        .collect();
    valid.sort_by(|a, b| {
        a.shortlist_key
            .as_ref()
            .unwrap()
            .cmp(b.shortlist_key.as_ref().unwrap())
    });
    valid.into_iter().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_exactly_48_candidates() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let spec = load_camera_search_spec(&root.join("presets/camera/camera-search-spec-v1.toml"))
            .unwrap();
        let c = expand_candidates(&spec).unwrap();
        assert_eq!(c.len(), 48);
        assert_eq!(c[0].id, "c000");
        assert_eq!(c[0].family_hint, "F1_BASELINE");
        assert_eq!(c[47].id, "c047");
        let d1 = camera_search_spec_digest(&spec);
        let d2 = camera_search_spec_digest(&spec);
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }
}
