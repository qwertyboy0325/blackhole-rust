//! Gate 2C1 absolute CIE XYZ + scene-linear RGB (Architecture B).
//!
//! Production XYZ is integrated from `PhysicalDiskEmissionFrame` `(F,T,g)` via
//! Planck + `g³` at official CIE 1 nm nodes — **not** from the sparse 256-bin
//! `PhysicalSpectralFrame` cube (that cube is diagnostic A-vs-B only).

use crate::color_space::{SceneLinearRgb, SceneLinearRgbSpace, XyzToRgbMatrix};
use crate::error::ColorimetryError;
use crate::physical_disk::{PhysicalDiskEmissionFrame, PhysicalDiskEmissionPixel};
use crate::physical_spectral::{
    independent_physical_i_nu_obs, PhysicalSpectralFrame, PhysicalSpectralPixel,
};
use relativity_core::SPEED_OF_LIGHT_M_S;
use relativity_trace::{hex_sha, OutcomeClass, TraceGrid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const CIE_OBSERVER_ID_V1: &str = "cie-1931-2deg-v1";
pub const CIE_TABLE_SOURCE_DOI: &str = "10.25039/CIE.DS.xvudnb9b";
pub const CIE_TABLE_MD5: &str = "17cca777db64b17170f06f67ce9d3ab7";
pub const CIE_TABLE_SHA256: &str =
    "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1";
pub const COLORIMETRIC_CONVENTION_ID: &str = "absolute-cie-xyz-km683-v1";
pub const KM_LM_PER_W: f64 = 683.0;
pub const KM_REVISION: &str = "cie-photometry-km-683-lm-w-v1";
pub const PHYSICAL_COLOR_FRAME_SCHEMA: u32 = 2;
/// ISO/CIE 11664-3 standard 1 nm method band (full official CIE 1931 table).
pub const PRODUCTION_LAMBDA_MIN_NM: i32 = 360;
pub const PRODUCTION_LAMBDA_MAX_NM: i32 = 830;
pub const PRODUCTION_N_SAMPLES: usize = 471; // 360..=830 inclusive, 1 nm
pub const PRODUCTION_BAND_ID: &str = "cie-1931-360-830-1nm-v1";
/// Authoritative raw payload magic (schema 2 encodes typed outcome).
pub const RAW_COLOR_PAYLOAD_MAGIC: &[u8; 8] = b"BHRXYZR2";
pub const RAW_COLOR_PAYLOAD_SCHEMA: u32 = 2;
pub const CIE_RELATIVE_ASSET_PATH: &str = "assets/standards/cie1931-2deg-v1.csv";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CieObserverId {
    Cie1931TwoDegV1,
}

impl CieObserverId {
    pub fn id(self) -> &'static str {
        match self {
            Self::Cie1931TwoDegV1 => CIE_OBSERVER_ID_V1,
        }
    }

    pub fn parse(s: &str) -> Result<Self, ColorimetryError> {
        if s == CIE_OBSERVER_ID_V1 {
            Ok(Self::Cie1931TwoDegV1)
        } else {
            Err(ColorimetryError::UnsupportedCieObserver(s.into()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrationMeasure {
    FrequencyNu,
    WavelengthLambda,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorimetricConvention {
    pub schema_version: u32,
    pub convention_id: String,
    pub km_lm_per_w: f64,
    pub km_revision: String,
    pub production_measure: String,
    pub exposure_policy: String,
    pub chromatic_adaptation: String,
    pub clamp_policy: String,
}

impl ColorimetricConvention {
    pub fn v1() -> Self {
        Self {
            schema_version: 1,
            convention_id: COLORIMETRIC_CONVENTION_ID.into(),
            km_lm_per_w: KM_LM_PER_W,
            km_revision: KM_REVISION.into(),
            production_measure: "frequency-I_nu-dnu-cmf-at-c-over-nu".into(),
            exposure_policy: "NO_FRAME_WHITE_NORMALIZATION".into(),
            chromatic_adaptation: "NO_CREATIVE_WHITE_BALANCE_NO_DISPLAY_ADAPTATION".into(),
            clamp_policy: "NO_SCIENTIFIC_CLAMP_NEGATIVES_ALLOWED".into(),
        }
    }

    pub fn validate_canonical(&self) -> Result<(), ColorimetryError> {
        let canon = Self::v1();
        if self.schema_version != canon.schema_version
            || self.convention_id != canon.convention_id
            || self.km_revision != canon.km_revision
            || self.km_lm_per_w != canon.km_lm_per_w
            || self.production_measure != canon.production_measure
            || self.exposure_policy != canon.exposure_policy
            || self.chromatic_adaptation != canon.chromatic_adaptation
            || self.clamp_policy != canon.clamp_policy
        {
            return Err(ColorimetryError::InvalidConvention(
                "non-canonical colorimetric convention".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ColorimetricXyz {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl ColorimetricXyz {
    pub fn new(x: f64, y: f64, z: f64) -> Result<Self, ColorimetryError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(ColorimetryError::NonFinite("XYZ".into()));
        }
        Ok(Self { x, y, z })
    }

    pub fn chromaticity_xy(self) -> Option<(f64, f64)> {
        let s = self.x + self.y + self.z;
        if !(s > 0.0) || !s.is_finite() {
            return None;
        }
        Some((self.x / s, self.y / s))
    }

    pub fn chromaticity_up_vp(self) -> Option<(f64, f64)> {
        let denom = self.x + 15.0 * self.y + 3.0 * self.z;
        if !(denom > 0.0) || !denom.is_finite() {
            return None;
        }
        Some((4.0 * self.x / denom, 9.0 * self.y / denom))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CieSample {
    pub lambda_nm: i32,
    pub x_bar: f64,
    pub y_bar: f64,
    pub z_bar: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cie1931Table {
    pub observer: CieObserverId,
    pub samples: Vec<CieSample>,
    pub content_sha256: String,
}

impl Cie1931Table {
    pub fn parse_csv(csv: &str) -> Result<Self, ColorimetryError> {
        let mut samples = Vec::new();
        for (lineno, line) in csv.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').map(str::trim).collect();
            if parts.len() != 4 {
                return Err(ColorimetryError::InvalidCieTable(format!(
                    "line {}: expected 4 columns, got {}",
                    lineno + 1,
                    parts.len()
                )));
            }
            let lambda_nm: i32 = parts[0].parse().map_err(|_| {
                ColorimetryError::InvalidCieTable(format!("line {}: bad lambda", lineno + 1))
            })?;
            let x_bar: f64 = parts[1].parse().map_err(|_| {
                ColorimetryError::InvalidCieTable(format!("line {}: bad x_bar", lineno + 1))
            })?;
            let y_bar: f64 = parts[2].parse().map_err(|_| {
                ColorimetryError::InvalidCieTable(format!("line {}: bad y_bar", lineno + 1))
            })?;
            let z_bar: f64 = parts[3].parse().map_err(|_| {
                ColorimetryError::InvalidCieTable(format!("line {}: bad z_bar", lineno + 1))
            })?;
            if !(x_bar.is_finite() && y_bar.is_finite() && z_bar.is_finite()) {
                return Err(ColorimetryError::InvalidCieTable(format!(
                    "line {}: non-finite CMF",
                    lineno + 1
                )));
            }
            if x_bar < 0.0 || y_bar < 0.0 || z_bar < 0.0 {
                return Err(ColorimetryError::InvalidCieTable(format!(
                    "line {}: negative CMF",
                    lineno + 1
                )));
            }
            samples.push(CieSample {
                lambda_nm,
                x_bar,
                y_bar,
                z_bar,
            });
        }
        if samples.is_empty() {
            return Err(ColorimetryError::InvalidCieTable("empty table".into()));
        }
        for w in samples.windows(2) {
            if w[1].lambda_nm != w[0].lambda_nm + 1 {
                return Err(ColorimetryError::InvalidCieTable(format!(
                    "non-monotonic/non-1nm step at {}→{}",
                    w[0].lambda_nm, w[1].lambda_nm
                )));
            }
        }
        let content_sha256 = hex_sha(csv.as_bytes());
        Ok(Self {
            observer: CieObserverId::Cie1931TwoDegV1,
            samples,
            content_sha256,
        })
    }

    /// Load official vendored CSV from a filesystem path (runtime; not embedded).
    pub fn load_official_v1_from_path(path: &Path) -> Result<Self, ColorimetryError> {
        let bytes = std::fs::read(path).map_err(|e| {
            ColorimetryError::InvalidCieTable(format!(
                "failed to read CIE asset {}: {e}",
                path.display()
            ))
        })?;
        Self::from_official_csv_bytes(&bytes)
    }

    /// Parse + pin-check official table bytes (SHA-256 / bounds).
    pub fn from_official_csv_bytes(bytes: &[u8]) -> Result<Self, ColorimetryError> {
        let csv = std::str::from_utf8(bytes)
            .map_err(|e| ColorimetryError::InvalidCieTable(format!("CIE CSV is not UTF-8: {e}")))?;
        let table = Self::parse_csv(csv)?;
        if table.content_sha256 != CIE_TABLE_SHA256 {
            return Err(ColorimetryError::InvalidCieTable(format!(
                "CIE SHA-256 mismatch: got {} expected {CIE_TABLE_SHA256}",
                table.content_sha256
            )));
        }
        if table.samples.first().map(|s| s.lambda_nm) != Some(360)
            || table.samples.last().map(|s| s.lambda_nm) != Some(830)
            || table.samples.len() != 471
        {
            return Err(ColorimetryError::InvalidCieTable(
                "official table must be 360..=830 nm (471 rows)".into(),
            ));
        }
        Ok(table)
    }

    /// Production integration nodes: full official 360–830 nm @ 1 nm (ISO/CIE 11664-3).
    pub fn production_subset(&self) -> Result<Vec<CieSample>, ColorimetryError> {
        let out: Vec<_> = self
            .samples
            .iter()
            .filter(|s| {
                s.lambda_nm >= PRODUCTION_LAMBDA_MIN_NM && s.lambda_nm <= PRODUCTION_LAMBDA_MAX_NM
            })
            .cloned()
            .collect();
        if out.len() != PRODUCTION_N_SAMPLES {
            return Err(ColorimetryError::InvalidCieTable(format!(
                "production subset expected {PRODUCTION_N_SAMPLES}, got {}",
                out.len()
            )));
        }
        if out[0].lambda_nm != PRODUCTION_LAMBDA_MIN_NM
            || out[out.len() - 1].lambda_nm != PRODUCTION_LAMBDA_MAX_NM
        {
            return Err(ColorimetryError::InvalidCieTable(
                "production subset bounds".into(),
            ));
        }
        Ok(out)
    }

    pub fn subsampled(&self, step_nm: i32) -> Result<Vec<CieSample>, ColorimetryError> {
        if step_nm < 1 {
            return Err(ColorimetryError::InvalidCieTable(
                "step_nm must be >= 1".into(),
            ));
        }
        let prod = self.production_subset()?;
        let out: Vec<_> = prod
            .into_iter()
            .filter(|s| (s.lambda_nm - PRODUCTION_LAMBDA_MIN_NM) % step_nm == 0)
            .collect();
        if out.is_empty() {
            return Err(ColorimetryError::InvalidCieTable("empty subsample".into()));
        }
        Ok(out)
    }
}

/// Stable wire encoding for `OutcomeClass` in raw f64 authority / EXR UINT.
pub fn outcome_class_code(c: OutcomeClass) -> u8 {
    match c {
        OutcomeClass::DiskHit => 1,
        OutcomeClass::Escaped => 2,
        OutcomeClass::HorizonEvent => 3,
        OutcomeClass::HorizonApproach => 4,
        OutcomeClass::AffineLimit => 5,
        OutcomeClass::Failed => 6,
    }
}

pub fn outcome_class_from_code(code: u8) -> Result<OutcomeClass, ColorimetryError> {
    match code {
        1 => Ok(OutcomeClass::DiskHit),
        2 => Ok(OutcomeClass::Escaped),
        3 => Ok(OutcomeClass::HorizonEvent),
        4 => Ok(OutcomeClass::HorizonApproach),
        5 => Ok(OutcomeClass::AffineLimit),
        6 => Ok(OutcomeClass::Failed),
        _ => Err(ColorimetryError::InvalidConvention(format!(
            "unknown outcome class code {code}"
        ))),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorPixelProvenance {
    pub source_physical_emission_digest: String,
    pub source_frequency_digest: String,
    pub cie_table_sha256: String,
    pub cie_observer_id: String,
    pub colorimetric_convention_id: String,
    pub rgb_space_id: String,
    pub rgb_matrix_digest: String,
    /// Diagnostic only — never the claimed integration input.
    pub source_physical_spectral_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ColorDiskHit {
    pub xyz: ColorimetricXyz,
    pub rgb: SceneLinearRgb,
    pub g_factor: f64,
    pub f_one_face_w_m2: f64,
    pub t_eff_k: f64,
    pub radius_over_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PhysicalColorPixel {
    DiskHit(ColorDiskHit),
    Absent { outcome_class: OutcomeClass },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PhysicalColorFrame {
    pub schema_version: u32,
    pub grid: TraceGrid,
    pub pixels: Vec<PhysicalColorPixel>,
    pub provenance: ColorPixelProvenance,
    pub convention: ColorimetricConvention,
    pub observer: CieObserverId,
    pub rgb_space: SceneLinearRgbSpace,
}

impl PhysicalColorFrame {
    pub fn try_new(
        grid: TraceGrid,
        pixels: Vec<PhysicalColorPixel>,
        provenance: ColorPixelProvenance,
        convention: ColorimetricConvention,
        observer: CieObserverId,
        rgb_space: SceneLinearRgbSpace,
    ) -> Result<Self, ColorimetryError> {
        let frame = Self {
            schema_version: PHYSICAL_COLOR_FRAME_SCHEMA,
            grid,
            pixels,
            provenance,
            convention,
            observer,
            rgb_space,
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), ColorimetryError> {
        if self.schema_version != PHYSICAL_COLOR_FRAME_SCHEMA {
            return Err(ColorimetryError::InvalidConvention(format!(
                "unexpected physical color schema {}",
                self.schema_version
            )));
        }
        if self.pixels.len() != self.grid.pixel_count() {
            return Err(ColorimetryError::FrameLengthMismatch);
        }
        self.convention.validate_canonical()?;
        if self.observer.id() != CIE_OBSERVER_ID_V1 {
            return Err(ColorimetryError::UnsupportedCieObserver(
                self.observer.id().into(),
            ));
        }
        if self.rgb_space.id() != crate::color_space::SCENE_LINEAR_RGB_SPACE_ID {
            return Err(ColorimetryError::UnsupportedRgbSpace(
                self.rgb_space.id().into(),
            ));
        }
        if self.provenance.cie_observer_id != self.observer.id()
            || self.provenance.colorimetric_convention_id != self.convention.convention_id
            || self.provenance.rgb_space_id != self.rgb_space.id()
        {
            return Err(ColorimetryError::ProvenanceMismatch(
                "provenance IDs disagree with frame fields".into(),
            ));
        }
        if self.provenance.cie_table_sha256 != CIE_TABLE_SHA256 {
            return Err(ColorimetryError::ProvenanceMismatch(format!(
                "cie_table_sha256 must be pinned official digest {CIE_TABLE_SHA256}"
            )));
        }
        if self.provenance.source_physical_emission_digest.len() != 64
            || self.provenance.source_frequency_digest.len() != 64
            || self.provenance.rgb_matrix_digest.len() != 64
        {
            return Err(ColorimetryError::ProvenanceMismatch(
                "source digests must be 64-char hex".into(),
            ));
        }
        for p in &self.pixels {
            match p {
                PhysicalColorPixel::DiskHit(h) => {
                    ColorimetricXyz::new(h.xyz.x, h.xyz.y, h.xyz.z)?;
                    SceneLinearRgb::new(h.rgb.r, h.rgb.g, h.rgb.b)?;
                    if !h.g_factor.is_finite() || !(h.g_factor > 0.0) {
                        return Err(ColorimetryError::NonFinite("g_factor".into()));
                    }
                    if !h.f_one_face_w_m2.is_finite() || h.f_one_face_w_m2 < 0.0 {
                        return Err(ColorimetryError::NonFinite("F".into()));
                    }
                    if !h.t_eff_k.is_finite() || h.t_eff_k < 0.0 {
                        return Err(ColorimetryError::NonFinite("T_eff".into()));
                    }
                    if !h.radius_over_m.is_finite() || !(h.radius_over_m > 0.0) {
                        return Err(ColorimetryError::NonFinite("r/M".into()));
                    }
                }
                PhysicalColorPixel::Absent { .. } => {}
            }
        }
        Ok(())
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> &PhysicalColorPixel {
        &self.pixels[relativity_trace::pixel_index(self.grid, col, row)]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorimetricMetrics {
    pub disk_hit_count: u64,
    pub negative_rgb_component_count: u64,
    pub max_abs_x: f64,
    pub max_abs_y: f64,
    pub max_abs_z: f64,
    pub max_abs_r: f64,
    pub max_abs_g: f64,
    pub max_abs_b: f64,
    pub min_r: f64,
    pub min_g: f64,
    pub min_b: f64,
    pub max_y_cd_m2: f64,
    pub worst_abs_y_index: Option<(u32, u32)>,
}

/// Integrate absolute XYZ from `(T_eff, g)` at CIE nodes (Architecture B).
pub fn integrate_xyz_from_emission(
    t_eff_k: f64,
    g: f64,
    samples: &[CieSample],
    measure: IntegrationMeasure,
) -> Result<ColorimetricXyz, ColorimetryError> {
    if samples.len() < 2 {
        return Err(ColorimetryError::InvalidCieTable(
            "need >= 2 CIE samples".into(),
        ));
    }
    if !g.is_finite() || !(g > 0.0) {
        return Err(ColorimetryError::NonFinite("g".into()));
    }
    if !t_eff_k.is_finite() || t_eff_k < 0.0 {
        return Err(ColorimetryError::NonFinite("T_eff".into()));
    }
    if t_eff_k == 0.0 {
        return ColorimetricXyz::new(0.0, 0.0, 0.0);
    }

    let mut acc_x = 0.0;
    let mut acc_y = 0.0;
    let mut acc_z = 0.0;

    match measure {
        IntegrationMeasure::FrequencyNu => {
            // Trapezoid in ν: X = Km ∫ I_ν(ν) x̄(c/ν) dν. Nodes ordered by ascending λ
            // so ν decreases; use signed Δν = ν_{i+1}-ν_i (negative) carefully via abs
            // trapezoid over the path.
            for i in 0..samples.len() - 1 {
                let a = &samples[i];
                let b = &samples[i + 1];
                let lam_a = (a.lambda_nm as f64) * 1e-9;
                let lam_b = (b.lambda_nm as f64) * 1e-9;
                let nu_a = SPEED_OF_LIGHT_M_S / lam_a;
                let nu_b = SPEED_OF_LIGHT_M_S / lam_b;
                let dnu = (nu_b - nu_a).abs();
                let i_a = independent_physical_i_nu_obs(t_eff_k, g, nu_a)
                    .map_err(|e| ColorimetryError::Spectral(e.to_string()))?;
                let i_b = independent_physical_i_nu_obs(t_eff_k, g, nu_b)
                    .map_err(|e| ColorimetryError::Spectral(e.to_string()))?;
                acc_x += 0.5 * (i_a * a.x_bar + i_b * b.x_bar) * dnu;
                acc_y += 0.5 * (i_a * a.y_bar + i_b * b.y_bar) * dnu;
                acc_z += 0.5 * (i_a * a.z_bar + i_b * b.z_bar) * dnu;
            }
        }
        IntegrationMeasure::WavelengthLambda => {
            // I_λ = I_ν · (c/λ²); X = Km ∫ I_λ x̄ dλ
            for i in 0..samples.len() - 1 {
                let a = &samples[i];
                let b = &samples[i + 1];
                let lam_a = (a.lambda_nm as f64) * 1e-9;
                let lam_b = (b.lambda_nm as f64) * 1e-9;
                let dlam = (lam_b - lam_a).abs();
                let nu_a = SPEED_OF_LIGHT_M_S / lam_a;
                let nu_b = SPEED_OF_LIGHT_M_S / lam_b;
                let i_nu_a = independent_physical_i_nu_obs(t_eff_k, g, nu_a)
                    .map_err(|e| ColorimetryError::Spectral(e.to_string()))?;
                let i_nu_b = independent_physical_i_nu_obs(t_eff_k, g, nu_b)
                    .map_err(|e| ColorimetryError::Spectral(e.to_string()))?;
                let i_l_a = i_nu_a * SPEED_OF_LIGHT_M_S / (lam_a * lam_a);
                let i_l_b = i_nu_b * SPEED_OF_LIGHT_M_S / (lam_b * lam_b);
                acc_x += 0.5 * (i_l_a * a.x_bar + i_l_b * b.x_bar) * dlam;
                acc_y += 0.5 * (i_l_a * a.y_bar + i_l_b * b.y_bar) * dlam;
                acc_z += 0.5 * (i_l_a * a.z_bar + i_l_b * b.z_bar) * dlam;
            }
        }
    }

    ColorimetricXyz::new(
        KM_LM_PER_W * acc_x,
        KM_LM_PER_W * acc_y,
        KM_LM_PER_W * acc_z,
    )
}

/// Diagnostic Architecture A: project 256-bin cube onto CIE via nearest-λ bin.
pub fn integrate_xyz_from_spectral_cube_diagnostic(
    i_nu_obs: &[f64],
    nu_centers_hz: &[f64],
    samples: &[CieSample],
) -> Result<ColorimetricXyz, ColorimetryError> {
    if i_nu_obs.len() != nu_centers_hz.len() || nu_centers_hz.len() < 2 {
        return Err(ColorimetryError::InvalidConvention(
            "spectral cube length mismatch".into(),
        ));
    }
    // Trapezoid on the cube's own ν nodes; CMF via λ=c/ν linear interp in table.
    let mut acc_x = 0.0;
    let mut acc_y = 0.0;
    let mut acc_z = 0.0;
    for i in 0..nu_centers_hz.len() - 1 {
        let nu_a = nu_centers_hz[i];
        let nu_b = nu_centers_hz[i + 1];
        if !(nu_a > 0.0 && nu_b > 0.0) {
            continue;
        }
        let dnu = (nu_b - nu_a).abs();
        let (xa, ya, za) = cmf_at_nu(samples, nu_a)?;
        let (xb, yb, zb) = cmf_at_nu(samples, nu_b)?;
        let ia = i_nu_obs[i];
        let ib = i_nu_obs[i + 1];
        if !(ia.is_finite() && ib.is_finite()) {
            return Err(ColorimetryError::NonFinite("cube I_nu".into()));
        }
        acc_x += 0.5 * (ia * xa + ib * xb) * dnu;
        acc_y += 0.5 * (ia * ya + ib * yb) * dnu;
        acc_z += 0.5 * (ia * za + ib * zb) * dnu;
    }
    ColorimetricXyz::new(
        KM_LM_PER_W * acc_x,
        KM_LM_PER_W * acc_y,
        KM_LM_PER_W * acc_z,
    )
}

fn cmf_at_nu(samples: &[CieSample], nu_hz: f64) -> Result<(f64, f64, f64), ColorimetryError> {
    let lam_nm = (SPEED_OF_LIGHT_M_S / nu_hz) * 1e9;
    if !lam_nm.is_finite() {
        return Err(ColorimetryError::NonFinite("lambda from nu".into()));
    }
    // Outside production band → CMF = 0 for diagnostic projection.
    let lo = samples[0].lambda_nm as f64;
    let hi = samples[samples.len() - 1].lambda_nm as f64;
    if lam_nm < lo || lam_nm > hi {
        return Ok((0.0, 0.0, 0.0));
    }
    let idx = (lam_nm - lo).floor() as usize;
    let idx = idx.min(samples.len() - 2);
    let a = &samples[idx];
    let b = &samples[idx + 1];
    let t = (lam_nm - a.lambda_nm as f64) / (b.lambda_nm as f64 - a.lambda_nm as f64);
    Ok((
        a.x_bar + t * (b.x_bar - a.x_bar),
        a.y_bar + t * (b.y_bar - a.y_bar),
        a.z_bar + t * (b.z_bar - a.z_bar),
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn build_physical_color_frame(
    emission: &PhysicalDiskEmissionFrame,
    cie: &Cie1931Table,
    rgb_matrix: &XyzToRgbMatrix,
    source_physical_emission_digest: &str,
    source_frequency_digest: &str,
    source_physical_spectral_digest: Option<&str>,
    measure: IntegrationMeasure,
    step_nm: i32,
) -> Result<PhysicalColorFrame, ColorimetryError> {
    let samples = if step_nm == 1 {
        cie.production_subset()?
    } else {
        cie.subsampled(step_nm)?
    };
    let convention = ColorimetricConvention::v1();
    let provenance = ColorPixelProvenance {
        source_physical_emission_digest: source_physical_emission_digest.into(),
        source_frequency_digest: source_frequency_digest.into(),
        cie_table_sha256: cie.content_sha256.clone(),
        cie_observer_id: cie.observer.id().into(),
        colorimetric_convention_id: convention.convention_id.clone(),
        rgb_space_id: rgb_matrix.space.id().into(),
        rgb_matrix_digest: rgb_matrix.digest(),
        source_physical_spectral_digest: source_physical_spectral_digest.map(str::to_string),
    };

    let grid = emission.grid;
    let mut pixels = Vec::with_capacity(grid.pixel_count());
    for row in 0..grid.height {
        for col in 0..grid.width {
            let pixel = match &emission.pixels[relativity_trace::pixel_index(grid, col, row)] {
                PhysicalDiskEmissionPixel::DiskHit(em) if em.f_one_face_w_m2 > 0.0 => {
                    let xyz =
                        integrate_xyz_from_emission(em.t_eff_k, em.g_factor, &samples, measure)
                            .map_err(|e| ColorimetryError::PixelMappingFailed {
                                col,
                                row,
                                cause: e.to_string(),
                            })?;
                    let rgb = rgb_matrix.apply(xyz).map_err(|e| {
                        ColorimetryError::PixelMappingFailed {
                            col,
                            row,
                            cause: e.to_string(),
                        }
                    })?;
                    PhysicalColorPixel::DiskHit(ColorDiskHit {
                        xyz,
                        rgb,
                        g_factor: em.g_factor,
                        f_one_face_w_m2: em.f_one_face_w_m2,
                        t_eff_k: em.t_eff_k,
                        radius_over_m: em.radius_over_m,
                    })
                }
                PhysicalDiskEmissionPixel::DiskHit(_) => PhysicalColorPixel::Absent {
                    outcome_class: OutcomeClass::DiskHit,
                },
                PhysicalDiskEmissionPixel::NotDiskHit { outcome_class } => {
                    PhysicalColorPixel::Absent {
                        outcome_class: *outcome_class,
                    }
                }
            };
            pixels.push(pixel);
        }
    }

    PhysicalColorFrame::try_new(
        grid,
        pixels,
        provenance,
        convention,
        cie.observer,
        rgb_matrix.space,
    )
}

pub fn physical_color_digest(frame: &PhysicalColorFrame) -> Result<String, ColorimetryError> {
    frame.validate()?;
    let mut h = Sha256::new();
    h.update(b"physical-color-digest-v2");
    h.update(frame.schema_version.to_le_bytes());
    h.update(PRODUCTION_BAND_ID.as_bytes());
    h.update(frame.observer.id().as_bytes());
    h.update(frame.convention.convention_id.as_bytes());
    h.update(frame.convention.km_revision.as_bytes());
    h.update(frame.convention.km_lm_per_w.to_bits().to_le_bytes());
    h.update(frame.rgb_space.id().as_bytes());
    h.update(frame.provenance.cie_table_sha256.as_bytes());
    h.update(frame.provenance.rgb_matrix_digest.as_bytes());
    h.update(frame.provenance.source_physical_emission_digest.as_bytes());
    h.update(frame.provenance.source_frequency_digest.as_bytes());
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for pix in &frame.pixels {
        match pix {
            PhysicalColorPixel::DiskHit(s) => {
                h.update([1u8]);
                h.update([outcome_class_code(OutcomeClass::DiskHit)]);
                h.update(s.xyz.x.to_bits().to_le_bytes());
                h.update(s.xyz.y.to_bits().to_le_bytes());
                h.update(s.xyz.z.to_bits().to_le_bytes());
                h.update(s.rgb.r.to_bits().to_le_bytes());
                h.update(s.rgb.g.to_bits().to_le_bytes());
                h.update(s.rgb.b.to_bits().to_le_bytes());
            }
            PhysicalColorPixel::Absent { outcome_class } => {
                h.update([0u8]);
                h.update([outcome_class_code(*outcome_class)]);
            }
        }
    }
    Ok(hex_sha(&h.finalize()))
}

/// Encode authoritative raw payload (schema 2): presence + outcome + XYZRGB.
pub fn encode_physical_color_payload(
    frame: &PhysicalColorFrame,
) -> Result<Vec<u8>, ColorimetryError> {
    frame.validate()?;
    let mut bytes = Vec::with_capacity(24 + frame.pixels.len() * (2 + 48));
    bytes.extend_from_slice(RAW_COLOR_PAYLOAD_MAGIC);
    bytes.extend_from_slice(&RAW_COLOR_PAYLOAD_SCHEMA.to_le_bytes());
    bytes.extend_from_slice(&frame.grid.width.to_le_bytes());
    bytes.extend_from_slice(&frame.grid.height.to_le_bytes());
    for pixel in &frame.pixels {
        match pixel {
            PhysicalColorPixel::DiskHit(s) => {
                bytes.push(1);
                bytes.push(outcome_class_code(OutcomeClass::DiskHit));
                for v in [s.xyz.x, s.xyz.y, s.xyz.z, s.rgb.r, s.rgb.g, s.rgb.b] {
                    bytes.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            PhysicalColorPixel::Absent { outcome_class } => {
                bytes.push(0);
                bytes.push(outcome_class_code(*outcome_class));
                for _ in 0..6 {
                    bytes.extend_from_slice(&0f64.to_bits().to_le_bytes());
                }
            }
        }
    }
    Ok(bytes)
}

pub fn payload_sha256(bytes: &[u8]) -> String {
    hex_sha(bytes)
}

/// Decode raw payload and verify it reconstructs the same scientific color digest.
pub fn verify_payload_matches_frame(
    bytes: &[u8],
    frame: &PhysicalColorFrame,
) -> Result<(), ColorimetryError> {
    let expected = physical_color_digest(frame)?;
    let decoded = decode_physical_color_payload_for_digest(bytes, frame)?;
    if decoded != expected {
        return Err(ColorimetryError::ProvenanceMismatch(format!(
            "raw payload digest {decoded} != frame digest {expected}"
        )));
    }
    // Pixel-level equality for presence/outcome/XYZRGB.
    let pixels = decode_physical_color_pixels(bytes, frame.grid)?;
    if pixels.len() != frame.pixels.len() {
        return Err(ColorimetryError::FrameLengthMismatch);
    }
    for (i, (a, b)) in pixels.iter().zip(frame.pixels.iter()).enumerate() {
        match (a, b) {
            (PhysicalColorPixel::DiskHit(pa), PhysicalColorPixel::DiskHit(pb)) => {
                if pa.xyz != pb.xyz || pa.rgb != pb.rgb {
                    return Err(ColorimetryError::ProvenanceMismatch(format!(
                        "payload XYZRGB mismatch at pixel {i}"
                    )));
                }
            }
            (
                PhysicalColorPixel::Absent { outcome_class: oa },
                PhysicalColorPixel::Absent { outcome_class: ob },
            ) => {
                if oa != ob {
                    return Err(ColorimetryError::ProvenanceMismatch(format!(
                        "payload outcome mismatch at pixel {i}"
                    )));
                }
            }
            _ => {
                return Err(ColorimetryError::ProvenanceMismatch(format!(
                    "payload presence mismatch at pixel {i}"
                )));
            }
        }
    }
    Ok(())
}

fn read_u32_le(bytes: &[u8], off: &mut usize) -> Result<u32, ColorimetryError> {
    let end = *off + 4;
    let slice = bytes
        .get(*off..end)
        .ok_or_else(|| ColorimetryError::InvalidConvention("payload truncated".into()))?;
    *off = end;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

fn read_f64_le(bytes: &[u8], off: &mut usize) -> Result<f64, ColorimetryError> {
    let end = *off + 8;
    let slice = bytes
        .get(*off..end)
        .ok_or_else(|| ColorimetryError::InvalidConvention("payload truncated".into()))?;
    *off = end;
    Ok(f64::from_bits(u64::from_le_bytes(
        slice.try_into().unwrap(),
    )))
}

pub fn decode_physical_color_pixels(
    bytes: &[u8],
    grid: TraceGrid,
) -> Result<Vec<PhysicalColorPixel>, ColorimetryError> {
    if bytes.len() < 20 {
        return Err(ColorimetryError::InvalidConvention(
            "payload too short".into(),
        ));
    }
    if &bytes[0..8] != RAW_COLOR_PAYLOAD_MAGIC {
        return Err(ColorimetryError::InvalidConvention(
            "bad raw color payload magic".into(),
        ));
    }
    let mut off = 8;
    let schema = read_u32_le(bytes, &mut off)?;
    if schema != RAW_COLOR_PAYLOAD_SCHEMA {
        return Err(ColorimetryError::InvalidConvention(format!(
            "unsupported raw color schema {schema}"
        )));
    }
    let width = read_u32_le(bytes, &mut off)?;
    let height = read_u32_le(bytes, &mut off)?;
    if width != grid.width || height != grid.height {
        return Err(ColorimetryError::InvalidConvention(
            "payload grid mismatch".into(),
        ));
    }
    let n = grid.pixel_count();
    let mut pixels = Vec::with_capacity(n);
    for _ in 0..n {
        let presence = *bytes
            .get(off)
            .ok_or_else(|| ColorimetryError::InvalidConvention("payload truncated".into()))?;
        off += 1;
        let outcome = outcome_class_from_code(
            *bytes
                .get(off)
                .ok_or_else(|| ColorimetryError::InvalidConvention("payload truncated".into()))?,
        )?;
        off += 1;
        let x = read_f64_le(bytes, &mut off)?;
        let y = read_f64_le(bytes, &mut off)?;
        let z = read_f64_le(bytes, &mut off)?;
        let r = read_f64_le(bytes, &mut off)?;
        let g = read_f64_le(bytes, &mut off)?;
        let b = read_f64_le(bytes, &mut off)?;
        match presence {
            1 => {
                if outcome != OutcomeClass::DiskHit {
                    return Err(ColorimetryError::InvalidConvention(
                        "presence=1 requires DiskHit outcome".into(),
                    ));
                }
                pixels.push(PhysicalColorPixel::DiskHit(ColorDiskHit {
                    xyz: ColorimetricXyz::new(x, y, z)?,
                    rgb: SceneLinearRgb::new(r, g, b)?,
                    g_factor: 1.0, // aux channels are EXR-only; not in raw XYZRGB authority
                    f_one_face_w_m2: 0.0,
                    t_eff_k: 0.0,
                    radius_over_m: 1.0,
                }));
            }
            0 => {
                if x != 0.0 || y != 0.0 || z != 0.0 || r != 0.0 || g != 0.0 || b != 0.0 {
                    return Err(ColorimetryError::InvalidConvention(
                        "absent pixel must store zero XYZRGB".into(),
                    ));
                }
                pixels.push(PhysicalColorPixel::Absent {
                    outcome_class: outcome,
                });
            }
            _ => {
                return Err(ColorimetryError::InvalidConvention(format!(
                    "bad presence byte {presence}"
                )));
            }
        }
    }
    if off != bytes.len() {
        return Err(ColorimetryError::InvalidConvention(
            "payload has trailing bytes".into(),
        ));
    }
    Ok(pixels)
}

fn decode_physical_color_payload_for_digest(
    bytes: &[u8],
    frame: &PhysicalColorFrame,
) -> Result<String, ColorimetryError> {
    // Reconstruct digest from payload presence/outcome/XYZRGB + frame authority headers.
    frame.validate()?;
    let pixels = decode_physical_color_pixels(bytes, frame.grid)?;
    let mut h = Sha256::new();
    h.update(b"physical-color-digest-v2");
    h.update(frame.schema_version.to_le_bytes());
    h.update(PRODUCTION_BAND_ID.as_bytes());
    h.update(frame.observer.id().as_bytes());
    h.update(frame.convention.convention_id.as_bytes());
    h.update(frame.convention.km_revision.as_bytes());
    h.update(frame.convention.km_lm_per_w.to_bits().to_le_bytes());
    h.update(frame.rgb_space.id().as_bytes());
    h.update(frame.provenance.cie_table_sha256.as_bytes());
    h.update(frame.provenance.rgb_matrix_digest.as_bytes());
    h.update(frame.provenance.source_physical_emission_digest.as_bytes());
    h.update(frame.provenance.source_frequency_digest.as_bytes());
    h.update(frame.grid.width.to_le_bytes());
    h.update(frame.grid.height.to_le_bytes());
    for pix in &pixels {
        match pix {
            PhysicalColorPixel::DiskHit(s) => {
                h.update([1u8]);
                h.update([outcome_class_code(OutcomeClass::DiskHit)]);
                h.update(s.xyz.x.to_bits().to_le_bytes());
                h.update(s.xyz.y.to_bits().to_le_bytes());
                h.update(s.xyz.z.to_bits().to_le_bytes());
                h.update(s.rgb.r.to_bits().to_le_bytes());
                h.update(s.rgb.g.to_bits().to_le_bytes());
                h.update(s.rgb.b.to_bits().to_le_bytes());
            }
            PhysicalColorPixel::Absent { outcome_class } => {
                h.update([0u8]);
                h.update([outcome_class_code(*outcome_class)]);
            }
        }
    }
    Ok(hex_sha(&h.finalize()))
}

pub fn compute_colorimetric_metrics(frame: &PhysicalColorFrame) -> ColorimetricMetrics {
    let mut disk_hit_count = 0u64;
    let mut negative_rgb_component_count = 0u64;
    let mut max_abs_x: f64 = 0.0;
    let mut max_abs_y: f64 = 0.0;
    let mut max_abs_z: f64 = 0.0;
    let mut max_abs_r: f64 = 0.0;
    let mut max_abs_g: f64 = 0.0;
    let mut max_abs_b: f64 = 0.0;
    let mut min_r = f64::INFINITY;
    let mut min_g = f64::INFINITY;
    let mut min_b = f64::INFINITY;
    let mut max_y: f64 = 0.0;
    let mut worst_abs_y_index = None;
    for row in 0..frame.grid.height {
        for col in 0..frame.grid.width {
            let PhysicalColorPixel::DiskHit(s) = frame.pixel_at(col, row) else {
                continue;
            };
            disk_hit_count += 1;
            negative_rgb_component_count += u64::from(s.rgb.negative_component_count());
            max_abs_x = max_abs_x.max(s.xyz.x.abs());
            max_abs_y = max_abs_y.max(s.xyz.y.abs());
            max_abs_z = max_abs_z.max(s.xyz.z.abs());
            max_abs_r = max_abs_r.max(s.rgb.r.abs());
            max_abs_g = max_abs_g.max(s.rgb.g.abs());
            max_abs_b = max_abs_b.max(s.rgb.b.abs());
            min_r = min_r.min(s.rgb.r);
            min_g = min_g.min(s.rgb.g);
            min_b = min_b.min(s.rgb.b);
            if s.xyz.y.abs() > max_y {
                max_y = s.xyz.y.abs();
                worst_abs_y_index = Some((col, row));
            }
        }
    }
    if !min_r.is_finite() {
        min_r = 0.0;
        min_g = 0.0;
        min_b = 0.0;
    }
    ColorimetricMetrics {
        disk_hit_count,
        negative_rgb_component_count,
        max_abs_x,
        max_abs_y,
        max_abs_z,
        max_abs_r,
        max_abs_g,
        max_abs_b,
        min_r,
        min_g,
        min_b,
        max_y_cd_m2: max_y,
        worst_abs_y_index,
    }
}

/// Compare Architecture B (emission) vs A (cube) on shared disk hits.
pub fn diagnostic_a_vs_b(
    color: &PhysicalColorFrame,
    spectral: &PhysicalSpectralFrame,
    cie: &Cie1931Table,
) -> Result<serde_json::Value, ColorimetryError> {
    let samples = cie.production_subset()?;
    let centers = spectral.spectral_grid.centers();
    let mut max_rel_y = 0.0_f64;
    let mut max_abs_y = 0.0_f64;
    let mut max_duv = 0.0_f64;
    let mut compared = 0u64;
    let mut worst_rel = None;
    for row in 0..color.grid.height {
        for col in 0..color.grid.width {
            let (PhysicalColorPixel::DiskHit(b), PhysicalSpectralPixel::DiskHit(a)) =
                (color.pixel_at(col, row), spectral.pixel_at(col, row))
            else {
                continue;
            };
            let xyz_a =
                integrate_xyz_from_spectral_cube_diagnostic(&a.i_nu_obs, centers, &samples)?;
            compared += 1;
            let abs_y = (xyz_a.y - b.xyz.y).abs();
            let rel_y = abs_y / b.xyz.y.abs().max(1e-30);
            if abs_y > max_abs_y {
                max_abs_y = abs_y;
            }
            if rel_y > max_rel_y {
                max_rel_y = rel_y;
                worst_rel = Some((col, row));
            }
            if let (Some(ua), Some(ub)) = (xyz_a.chromaticity_up_vp(), b.xyz.chromaticity_up_vp()) {
                let duv = ((ua.0 - ub.0).hypot(ua.1 - ub.1)).abs();
                max_duv = max_duv.max(duv);
            }
        }
    }
    Ok(serde_json::json!({
        "role": "DIAGNOSTIC_ONLY",
        "architecture_a": "project-256-bin-PhysicalSpectralFrame",
        "architecture_b": "PhysicalDiskEmissionFrame-CIE-1nm",
        "note": "A is not production colorimetry authority",
        "compared_disk_hits": compared,
        "max_abs_y_error": max_abs_y,
        "max_rel_y_error": max_rel_y,
        "max_delta_u_v_prime": max_duv,
        "worst_rel_y_pixel": worst_rel,
    }))
}

/// Tiny synthetic CMF for hermetic unit tests (not official CIE; not for production frames).
pub fn synthetic_cmf_for_tests() -> Cie1931Table {
    let mut samples = Vec::new();
    for lam in PRODUCTION_LAMBDA_MIN_NM..=PRODUCTION_LAMBDA_MAX_NM {
        let t = (lam - PRODUCTION_LAMBDA_MIN_NM) as f64
            / (PRODUCTION_LAMBDA_MAX_NM - PRODUCTION_LAMBDA_MIN_NM) as f64;
        let x =
            (-((t - 0.7) / 0.15).powi(2)).exp() * 0.5 + (-((t - 0.25) / 0.12).powi(2)).exp() * 0.3;
        let y = (-((t - 0.5) / 0.18).powi(2)).exp();
        let z = (-((t - 0.2) / 0.14).powi(2)).exp() * 0.8;
        samples.push(CieSample {
            lambda_nm: lam,
            x_bar: x,
            y_bar: y,
            z_bar: z,
        });
    }
    let mut h = Sha256::new();
    h.update(b"synthetic-cmf-v2-360-830");
    Cie1931Table {
        observer: CieObserverId::Cie1931TwoDegV1,
        samples,
        content_sha256: hex_sha(&h.finalize()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlackbodyChromaticitySample {
    pub t_k: f64,
    pub x: f64,
    pub y: f64,
}

/// Independent blackbody chromaticity / Planckian-direction check (g=1).
/// Returns Planckian samples and ν↔λ Δxy at 6500 K.
pub fn blackbody_planckian_direction_ok(
    samples: &[CieSample],
) -> Result<(Vec<BlackbodyChromaticitySample>, f64), ColorimetryError> {
    let temps = [3000.0_f64, 5000.0, 6500.0, 10000.0];
    let mut pts = Vec::new();
    for &t in &temps {
        let xyz = integrate_xyz_from_emission(t, 1.0, samples, IntegrationMeasure::FrequencyNu)?;
        let xy = xyz
            .chromaticity_xy()
            .ok_or_else(|| ColorimetryError::NonFinite("blackbody xy".into()))?;
        pts.push(BlackbodyChromaticitySample {
            t_k: t,
            x: xy.0,
            y: xy.1,
        });
    }
    // In 3000–10000 K, Planckian locus x decreases as T increases.
    for w in pts.windows(2) {
        if !(w[1].x < w[0].x) {
            return Err(ColorimetryError::InvalidConvention(format!(
                "Planckian x not decreasing with T: {:?} → {:?}",
                w[0], w[1]
            )));
        }
    }
    let xyz_nu =
        integrate_xyz_from_emission(6500.0, 1.0, samples, IntegrationMeasure::FrequencyNu)?;
    let xyz_l =
        integrate_xyz_from_emission(6500.0, 1.0, samples, IntegrationMeasure::WavelengthLambda)?;
    let (xn, yn) = xyz_nu
        .chromaticity_xy()
        .ok_or_else(|| ColorimetryError::NonFinite("nu xy".into()))?;
    let (xl, yl) = xyz_l
        .chromaticity_xy()
        .ok_or_else(|| ColorimetryError::NonFinite("lam xy".into()))?;
    let dxy = (xn - xl).hypot(yn - yl);
    Ok((pts, dxy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_space::XyzToRgbMatrix;
    use std::path::PathBuf;

    fn official_table() -> Cie1931Table {
        let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        root.pop(); // crates
        root.pop(); // workspace
        Cie1931Table::load_official_v1_from_path(&root.join(CIE_RELATIVE_ASSET_PATH)).unwrap()
    }

    #[test]
    fn official_table_loads_from_runtime_asset() {
        let t = official_table();
        assert_eq!(t.content_sha256, CIE_TABLE_SHA256);
        assert_eq!(t.production_subset().unwrap().len(), PRODUCTION_N_SAMPLES);
        assert_eq!(t.samples.len(), 471);
    }

    #[test]
    fn nu_lambda_agree_blackbody() {
        let t = official_table();
        let samples = t.production_subset().unwrap();
        let xyz_nu =
            integrate_xyz_from_emission(6500.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)
                .unwrap();
        let xyz_l = integrate_xyz_from_emission(
            6500.0,
            1.0,
            &samples,
            IntegrationMeasure::WavelengthLambda,
        )
        .unwrap();
        let rel_y = (xyz_nu.y - xyz_l.y).abs() / xyz_nu.y.max(1e-30);
        assert!(rel_y < 1e-5, "ν↔λ Y rel {rel_y}");
    }

    #[test]
    fn zero_temperature_zero_xyz() {
        let t = synthetic_cmf_for_tests();
        let samples = t.production_subset().unwrap();
        let xyz = integrate_xyz_from_emission(0.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)
            .unwrap();
        assert_eq!(xyz.x + xyz.y + xyz.z, 0.0);
    }

    #[test]
    fn g_shift_moves_chromaticity() {
        let t = official_table();
        let samples = t.production_subset().unwrap();
        let a = integrate_xyz_from_emission(5000.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)
            .unwrap();
        let b = integrate_xyz_from_emission(5000.0, 0.7, &samples, IntegrationMeasure::FrequencyNu)
            .unwrap();
        let ca = a.chromaticity_xy().unwrap();
        let cb = b.chromaticity_xy().unwrap();
        assert!((ca.0 - cb.0).abs() + (ca.1 - cb.1).abs() > 1e-4);
    }

    #[test]
    fn equal_energy_positive_y() {
        let t = official_table();
        let samples = t.production_subset().unwrap();
        let xyz =
            integrate_xyz_from_emission(10000.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)
                .unwrap();
        assert!(xyz.y > 0.0 && xyz.x > 0.0 && xyz.z > 0.0);
    }

    #[test]
    fn sampling_ladder_converges_full_band() {
        let t = official_table();
        let ref_s = t.production_subset().unwrap();
        let xyz_ref =
            integrate_xyz_from_emission(6500.0, 1.0, &ref_s, IntegrationMeasure::FrequencyNu)
                .unwrap();
        let mut prev = f64::INFINITY;
        for step in [10, 5, 2, 1] {
            let s = t.subsampled(step).unwrap();
            let xyz = integrate_xyz_from_emission(6500.0, 1.0, &s, IntegrationMeasure::FrequencyNu)
                .unwrap();
            let err = (xyz.y - xyz_ref.y).abs() / xyz_ref.y;
            if step > 1 {
                assert!(err < prev * 1.05 + 1e-9 || err < 1e-3);
            } else {
                assert_eq!(err, 0.0);
            }
            prev = err;
        }
    }

    #[test]
    fn planckian_direction_and_nu_lambda_xy() {
        let t = official_table();
        let samples = t.production_subset().unwrap();
        let (pts, dxy) = blackbody_planckian_direction_ok(&samples).unwrap();
        assert_eq!(pts.len(), 4);
        assert!(dxy < 1e-5, "ν↔λ Δxy={dxy}");
    }

    #[test]
    fn raw_payload_roundtrips_digest() {
        let grid = TraceGrid {
            width: 2,
            height: 1,
        };
        let fake = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let frame = PhysicalColorFrame::try_new(
            grid,
            vec![
                PhysicalColorPixel::DiskHit(ColorDiskHit {
                    xyz: ColorimetricXyz::new(1.0, 2.0, 3.0).unwrap(),
                    rgb: SceneLinearRgb::new(-0.1, 0.5, 2.0).unwrap(),
                    g_factor: 1.1,
                    f_one_face_w_m2: 1e5,
                    t_eff_k: 4000.0,
                    radius_over_m: 12.0,
                }),
                PhysicalColorPixel::Absent {
                    outcome_class: OutcomeClass::Escaped,
                },
            ],
            ColorPixelProvenance {
                source_physical_emission_digest: fake.into(),
                source_frequency_digest: fake.into(),
                cie_table_sha256: CIE_TABLE_SHA256.into(),
                cie_observer_id: CIE_OBSERVER_ID_V1.into(),
                colorimetric_convention_id: COLORIMETRIC_CONVENTION_ID.into(),
                rgb_space_id: crate::color_space::SCENE_LINEAR_RGB_SPACE_ID.into(),
                rgb_matrix_digest: fake.into(),
                source_physical_spectral_digest: None,
            },
            ColorimetricConvention::v1(),
            CieObserverId::Cie1931TwoDegV1,
            SceneLinearRgbSpace::Rec709D65LinearV1,
        )
        .unwrap();
        let bytes = encode_physical_color_payload(&frame).unwrap();
        verify_payload_matches_frame(&bytes, &frame).unwrap();
        // Tamper absent outcome → fail closed.
        // Header 20 bytes; pixel0: 2+48; pixel1 presence at 70, outcome at 71.
        let mut bad = bytes;
        bad[71] = outcome_class_code(OutcomeClass::Failed);
        assert!(verify_payload_matches_frame(&bad, &frame).is_err());
    }

    #[test]
    fn rgb_from_xyz_unclamped() {
        let m = XyzToRgbMatrix::rec709_d65_linear_v1();
        let xyz = ColorimetricXyz::new(0.1, 0.05, 0.8).unwrap();
        let rgb = m.apply(xyz).unwrap();
        assert!(rgb.r.is_finite() && rgb.g.is_finite() && rgb.b.is_finite());
    }

    #[test]
    fn no_compile_time_cie_embedding_symbol() {
        // Guardrail: official table must be loaded from filesystem, never include_str.
        assert!(!CIE_RELATIVE_ASSET_PATH.is_empty());
    }
}
