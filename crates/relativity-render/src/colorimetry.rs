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

pub const CIE_OBSERVER_ID_V1: &str = "cie-1931-2deg-v1";
pub const CIE_TABLE_SOURCE_DOI: &str = "10.25039/CIE.DS.xvudnb9b";
pub const CIE_TABLE_MD5: &str = "17cca777db64b17170f06f67ce9d3ab7";
pub const CIE_TABLE_SHA256: &str =
    "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1";
pub const COLORIMETRIC_CONVENTION_ID: &str = "absolute-cie-xyz-km683-v1";
pub const KM_LM_PER_W: f64 = 683.0;
pub const KM_REVISION: &str = "cie-photometry-km-683-lm-w-v1";
pub const PHYSICAL_COLOR_FRAME_SCHEMA: u32 = 1;
pub const PRODUCTION_LAMBDA_MIN_NM: i32 = 380;
pub const PRODUCTION_LAMBDA_MAX_NM: i32 = 780;
pub const PRODUCTION_N_SAMPLES: usize = 401; // 380..=780 inclusive, 1 nm

/// Official vendored CSV (360–830 nm). Embedded for hermetic digests.
pub const OFFICIAL_CIE_CSV: &str = include_str!("../../../assets/standards/cie1931-2deg-v1.csv");

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
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    pub fn official_v1() -> Result<Self, ColorimetryError> {
        let table = Self::parse_csv(OFFICIAL_CIE_CSV)?;
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

    /// Production integration nodes: 380–780 nm @ 1 nm (401 samples).
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorDiskHit {
    pub xyz: ColorimetricXyz,
    pub rgb: SceneLinearRgb,
    pub g_factor: f64,
    pub f_one_face_w_m2: f64,
    pub t_eff_k: f64,
    pub radius_over_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        if pixels.len() != grid.pixel_count() {
            return Err(ColorimetryError::FrameLengthMismatch);
        }
        for p in &pixels {
            if let PhysicalColorPixel::DiskHit(h) = p {
                ColorimetricXyz::new(h.xyz.x, h.xyz.y, h.xyz.z)?;
                SceneLinearRgb::new(h.rgb.r, h.rgb.g, h.rgb.b)?;
                if !h.g_factor.is_finite() || !(h.g_factor > 0.0) {
                    return Err(ColorimetryError::NonFinite("g_factor".into()));
                }
            }
        }
        Ok(Self {
            schema_version: PHYSICAL_COLOR_FRAME_SCHEMA,
            grid,
            pixels,
            provenance,
            convention,
            observer,
            rgb_space,
        })
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

pub fn physical_color_digest(frame: &PhysicalColorFrame) -> String {
    let mut h = Sha256::new();
    h.update(b"physical-color-digest-v1");
    h.update(frame.schema_version.to_le_bytes());
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
                h.update(s.xyz.x.to_bits().to_le_bytes());
                h.update(s.xyz.y.to_bits().to_le_bytes());
                h.update(s.xyz.z.to_bits().to_le_bytes());
                h.update(s.rgb.r.to_bits().to_le_bytes());
                h.update(s.rgb.g.to_bits().to_le_bytes());
                h.update(s.rgb.b.to_bits().to_le_bytes());
            }
            PhysicalColorPixel::Absent { outcome_class } => {
                h.update([0u8]);
                h.update(outcome_class.digest_tag().as_bytes());
            }
        }
    }
    hex_sha(&h.finalize())
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

/// Tiny synthetic CMF for hermetic unit tests (not official CIE).
pub fn synthetic_cmf_for_tests() -> Cie1931Table {
    let mut samples = Vec::new();
    for lam in 380..=780 {
        let t = (lam - 380) as f64 / 400.0;
        // Broad overlapping bumps — not physically accurate.
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
    h.update(b"synthetic-cmf-v1");
    Cie1931Table {
        observer: CieObserverId::Cie1931TwoDegV1,
        samples,
        content_sha256: hex_sha(&h.finalize()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_space::XyzToRgbMatrix;

    #[test]
    fn official_table_loads_and_digests() {
        let t = Cie1931Table::official_v1().unwrap();
        assert_eq!(t.content_sha256, CIE_TABLE_SHA256);
        assert_eq!(t.production_subset().unwrap().len(), 401);
    }

    #[test]
    fn nu_lambda_agree_blackbody() {
        let t = Cie1931Table::official_v1().unwrap();
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
        assert!(
            rel_y < 1e-5,
            "ν↔λ Y rel {rel_y}: nu={} lam={}",
            xyz_nu.y,
            xyz_l.y
        );
        let rel_x = (xyz_nu.x - xyz_l.x).abs() / xyz_nu.x.max(1e-30);
        let rel_z = (xyz_nu.z - xyz_l.z).abs() / xyz_nu.z.max(1e-30);
        assert!(rel_x < 1e-5 && rel_z < 1e-5);
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
        let t = Cie1931Table::official_v1().unwrap();
        let samples = t.production_subset().unwrap();
        let a = integrate_xyz_from_emission(5000.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)
            .unwrap();
        let b = integrate_xyz_from_emission(5000.0, 0.7, &samples, IntegrationMeasure::FrequencyNu)
            .unwrap();
        let ca = a.chromaticity_xy().unwrap();
        let cb = b.chromaticity_xy().unwrap();
        // Blueshift of spectrum in observer frame for g<1 (ν_em = ν_obs/g higher).
        assert!((ca.0 - cb.0).abs() + (ca.1 - cb.1).abs() > 1e-4);
    }

    #[test]
    fn equal_energy_positive_y() {
        // Direct equal-energy: I_ν=1 constant via synthetic path — use blackbody high T.
        let t = Cie1931Table::official_v1().unwrap();
        let samples = t.production_subset().unwrap();
        let xyz =
            integrate_xyz_from_emission(10000.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)
                .unwrap();
        assert!(xyz.y > 0.0);
        assert!(xyz.x > 0.0 && xyz.z > 0.0);
    }

    #[test]
    fn sampling_ladder_converges() {
        let t = Cie1931Table::official_v1().unwrap();
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
    fn rgb_from_xyz_unclamped() {
        let m = XyzToRgbMatrix::rec709_d65_linear_v1();
        // Highly saturated blue-ish XYZ may go negative in Rec.709.
        let xyz = ColorimetricXyz::new(0.1, 0.05, 0.8).unwrap();
        let rgb = m.apply(xyz).unwrap();
        assert!(rgb.r.is_finite() && rgb.g.is_finite() && rgb.b.is_finite());
    }
}
