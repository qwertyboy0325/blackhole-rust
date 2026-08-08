//! Gate 2D0 presentation transform: exposure → gamut → tone map → display encode.
//!
//! Presentation state is derived and replaceable. It never mutates scientific
//! `PhysicalColorFrame` authority.

use crate::colorimetry::{physical_color_digest, PhysicalColorFrame, PhysicalColorPixel};
use crate::display_encoding::{
    DisplayEncodedRgb16, DISPLAY_TARGET_SRGB_V1, OETF_ID_SRGB_IEC61966_2_1_V1,
    PNG_FORMAT_RGB16_SRGB_V1, PNG_GAMA_SRGB, PNG_SRGB_INTENT_PERCEPTUAL,
};
use crate::error::PresentationError;
use crate::tone_map::{apply_tone_map, LinearRgb, ToneMapOperator};
use relativity_trace::hex_sha;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PRESENTATION_MODEL_V1: &str = "presentation-model-v1";
pub const GAMUT_MAPPER_ID_LUMINANCE_AXIS_DESAT_V1: &str = "luminance-axis-desat-v1";
pub const BIT_DEPTH_RGB16: u16 = 16;

/// Rec. ITU-R BT.709 / IEC 61966-2-1 luminance weights (sum to 1).
pub const REC709_LUMA_WR: f64 = 0.2126;
pub const REC709_LUMA_WG: f64 = 0.7152;
pub const REC709_LUMA_WB: f64 = 0.0722;

/// Roundoff band for gamut luminance preservation / near-zero Y.
pub const GAMUT_EPS: f64 = 1e-12;

impl LinearRgb {
    pub fn new(r: f64, g: f64, b: f64) -> Result<Self, PresentationError> {
        if !(r.is_finite() && g.is_finite() && b.is_finite()) {
            return Err(PresentationError::NonFiniteSourceColor(
                "non-finite linear RGB".into(),
            ));
        }
        Ok(Self { r, g, b })
    }

    pub fn luminance_rec709(self) -> f64 {
        REC709_LUMA_WR * self.r + REC709_LUMA_WG * self.g + REC709_LUMA_WB * self.b
    }

    pub fn negative_component_count(self) -> u32 {
        u32::from(self.r < 0.0) + u32::from(self.g < 0.0) + u32::from(self.b < 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamutMapOperator {
    LuminanceAxisDesatV1,
}

impl GamutMapOperator {
    pub fn id(self) -> &'static str {
        match self {
            Self::LuminanceAxisDesatV1 => GAMUT_MAPPER_ID_LUMINANCE_AXIS_DESAT_V1,
        }
    }

    pub fn parse(id: &str) -> Result<Self, PresentationError> {
        if id == GAMUT_MAPPER_ID_LUMINANCE_AXIS_DESAT_V1 {
            Ok(Self::LuminanceAxisDesatV1)
        } else {
            Err(PresentationError::UnsupportedOperator(id.into()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExposureSpec {
    pub middle_gray_luminance_cd_m2: f64,
    pub exposure_ev: f64,
}

impl ExposureSpec {
    pub fn new(
        middle_gray_luminance_cd_m2: f64,
        exposure_ev: f64,
    ) -> Result<Self, PresentationError> {
        if !middle_gray_luminance_cd_m2.is_finite() || !(middle_gray_luminance_cd_m2 > 0.0) {
            return Err(PresentationError::InvalidExposure(
                "middle_gray_luminance_cd_m2 must be finite and > 0".into(),
            ));
        }
        if !exposure_ev.is_finite() {
            return Err(PresentationError::InvalidExposure(
                "exposure_ev must be finite".into(),
            ));
        }
        Ok(Self {
            middle_gray_luminance_cd_m2,
            exposure_ev,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationSpec {
    pub schema_version: u32,
    pub model_id: String,
    pub middle_gray_luminance_cd_m2: f64,
    pub exposure_ev: f64,
    pub tone_mapper: String,
    pub gamut_mapper: String,
    pub display_target: String,
    pub oetf: String,
    pub bit_depth: u16,
}

impl PresentationSpec {
    pub fn v1(
        middle_gray_luminance_cd_m2: f64,
        exposure_ev: f64,
    ) -> Result<Self, PresentationError> {
        let exposure = ExposureSpec::new(middle_gray_luminance_cd_m2, exposure_ev)?;
        let tone = ToneMapOperator::KhronosPbrNeutralV1;
        let gamut = GamutMapOperator::LuminanceAxisDesatV1;
        Ok(Self {
            schema_version: 1,
            model_id: PRESENTATION_MODEL_V1.into(),
            middle_gray_luminance_cd_m2: exposure.middle_gray_luminance_cd_m2,
            exposure_ev: exposure.exposure_ev,
            tone_mapper: tone.id().into(),
            gamut_mapper: gamut.id().into(),
            display_target: DISPLAY_TARGET_SRGB_V1.into(),
            oetf: OETF_ID_SRGB_IEC61966_2_1_V1.into(),
            bit_depth: BIT_DEPTH_RGB16,
        })
    }

    pub fn validate(&self) -> Result<(), PresentationError> {
        if self.schema_version != 1 || self.model_id != PRESENTATION_MODEL_V1 {
            return Err(PresentationError::InvalidPresentationSpec(
                "unsupported presentation schema/model".into(),
            ));
        }
        let _ = ExposureSpec::new(self.middle_gray_luminance_cd_m2, self.exposure_ev)?;
        let _ = ToneMapOperator::parse(&self.tone_mapper)?;
        let _ = GamutMapOperator::parse(&self.gamut_mapper)?;
        if self.display_target != DISPLAY_TARGET_SRGB_V1 {
            return Err(PresentationError::InvalidPresentationSpec(format!(
                "unsupported display_target {}",
                self.display_target
            )));
        }
        if self.oetf != OETF_ID_SRGB_IEC61966_2_1_V1 {
            return Err(PresentationError::InvalidPresentationSpec(format!(
                "unsupported oetf {}",
                self.oetf
            )));
        }
        if self.bit_depth != BIT_DEPTH_RGB16 {
            return Err(PresentationError::InvalidPresentationSpec(format!(
                "unsupported bit_depth {}",
                self.bit_depth
            )));
        }
        Ok(())
    }

    pub fn tone_operator(&self) -> Result<ToneMapOperator, PresentationError> {
        ToneMapOperator::parse(&self.tone_mapper)
    }

    pub fn gamut_operator(&self) -> Result<GamutMapOperator, PresentationError> {
        GamutMapOperator::parse(&self.gamut_mapper)
    }

    pub fn exposure(&self) -> Result<ExposureSpec, PresentationError> {
        ExposureSpec::new(self.middle_gray_luminance_cd_m2, self.exposure_ev)
    }
}

/// A1 exposure:
/// `RGB_exposed = RGB_abs * 0.18 * 2^EV / middle_gray_luminance_cd_m2`
pub fn apply_exposure(
    rgb: LinearRgb,
    exposure: &ExposureSpec,
) -> Result<LinearRgb, PresentationError> {
    let scale = 0.18 * (2.0_f64).powf(exposure.exposure_ev) / exposure.middle_gray_luminance_cd_m2;
    if !scale.is_finite() {
        return Err(PresentationError::InvalidExposure(
            "non-finite exposure scale".into(),
        ));
    }
    LinearRgb::new(rgb.r * scale, rgb.g * scale, rgb.b * scale)
}

/// `luminance-axis-desat-v1` — exact owner A2 contract.
///
/// HDR components `> 1` are valid and returned unchanged when all channels ≥ 0.
pub fn luminance_axis_desat_v1(c: LinearRgb) -> Result<(LinearRgb, bool), PresentationError> {
    if !(c.r.is_finite() && c.g.is_finite() && c.b.is_finite()) {
        return Err(PresentationError::PresentationGamutFailure(
            "non-finite gamut input".into(),
        ));
    }
    let y = c.luminance_rec709();
    if !y.is_finite() {
        return Err(PresentationError::PresentationGamutFailure(
            "non-finite luminance".into(),
        ));
    }
    if y < -GAMUT_EPS {
        return Err(PresentationError::PresentationGamutFailure(format!(
            "significant negative luminance Y={y}"
        )));
    }

    if c.r >= 0.0 && c.g >= 0.0 && c.b >= 0.0 {
        return Ok((c, false));
    }

    // Near-zero Y with negatives → only nonnegative equal-luminance point is black.
    if y.abs() <= GAMUT_EPS {
        return Ok((
            LinearRgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
            },
            true,
        ));
    }

    let mut t_star = 1.0_f64;
    for &ci in &[c.r, c.g, c.b] {
        if ci < 0.0 {
            let denom = y - ci;
            if !(denom > 0.0) {
                return Err(PresentationError::PresentationGamutFailure(
                    "invalid desaturation denominator".into(),
                ));
            }
            t_star = t_star.min(y / denom);
        }
    }
    if !(t_star.is_finite() && (0.0..=1.0).contains(&t_star)) {
        return Err(PresentationError::PresentationGamutFailure(format!(
            "invalid t*={t_star}"
        )));
    }

    let mut out = LinearRgb {
        r: y + t_star * (c.r - y),
        g: y + t_star * (c.g - y),
        b: y + t_star * (c.b - y),
    };
    // ε-only negative canonicalization
    for v in [&mut out.r, &mut out.g, &mut out.b] {
        if *v < -GAMUT_EPS {
            return Err(PresentationError::PresentationGamutFailure(format!(
                "post-desat significant negative {v}"
            )));
        }
        if *v < 0.0 {
            *v = 0.0;
        }
    }
    let y_out = out.luminance_rec709();
    let y_err = (y_out - y).abs();
    let y_tol = GAMUT_EPS * (1.0 + y.abs());
    if y_err > y_tol {
        return Err(PresentationError::PresentationGamutFailure(format!(
            "luminance not preserved: in={y} out={y_out} err={y_err}"
        )));
    }
    Ok((out, true))
}

pub fn apply_gamut(
    op: GamutMapOperator,
    c: LinearRgb,
) -> Result<(LinearRgb, bool), PresentationError> {
    match op {
        GamutMapOperator::LuminanceAxisDesatV1 => luminance_axis_desat_v1(c),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationMetrics {
    pub pixel_count: u64,
    pub source_disk_hit_count: u64,
    pub negative_component_count_before_gamut: u64,
    pub negative_pixel_count_before_gamut: u64,
    pub gamut_adjusted_pixel_count: u64,
    pub max_gamut_correction: f64,
    pub worst_gamut_raster_index: Option<u32>,
    pub pre_tone_max_rgb: f64,
    pub pre_tone_min_luma: f64,
    pub pre_tone_max_luma: f64,
    pub pre_tone_median_luma_estimate: f64,
    pub post_tone_min: f64,
    pub post_tone_max: f64,
    pub endpoint_epsilon_canonicalization_count: u64,
    pub final_code_min: u16,
    pub final_code_max: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<DisplayEncodedRgb16>,
    pub source_physical_color_digest: String,
    pub presentation_spec_digest: String,
    pub presentation_frame_digest: String,
    pub metrics: PresentationMetrics,
}

/// PRESENTATION_REPRODUCIBILITY_DIGEST over semantic spec fields (not JSON text).
pub fn presentation_spec_digest(spec: &PresentationSpec) -> Result<String, PresentationError> {
    spec.validate()?;
    let mut h = Sha256::new();
    h.update(b"presentation-spec-digest-v1");
    h.update(spec.schema_version.to_le_bytes());
    h.update(spec.model_id.as_bytes());
    h.update(spec.middle_gray_luminance_cd_m2.to_bits().to_le_bytes());
    h.update(spec.exposure_ev.to_bits().to_le_bytes());
    h.update(spec.tone_mapper.as_bytes());
    h.update(spec.gamut_mapper.as_bytes());
    h.update(spec.display_target.as_bytes());
    h.update(spec.oetf.as_bytes());
    h.update(spec.bit_depth.to_le_bytes());
    Ok(hex_sha(&h.finalize()))
}

pub fn presentation_frame_digest(
    source_physical_color_digest: &str,
    presentation_spec_digest: &str,
    width: u32,
    height: u32,
    pixels: &[DisplayEncodedRgb16],
) -> String {
    let mut h = Sha256::new();
    h.update(b"presentation-frame-digest-v1");
    h.update(b"PRESENTATION_REPRODUCIBILITY_DIGEST");
    h.update(source_physical_color_digest.as_bytes());
    h.update(presentation_spec_digest.as_bytes());
    h.update(width.to_le_bytes());
    h.update(height.to_le_bytes());
    h.update(DISPLAY_TARGET_SRGB_V1.as_bytes());
    h.update(PNG_FORMAT_RGB16_SRGB_V1.as_bytes());
    h.update(BIT_DEPTH_RGB16.to_le_bytes());
    for p in pixels {
        h.update(p.r.to_le_bytes());
        h.update(p.g.to_le_bytes());
        h.update(p.b.to_le_bytes());
    }
    hex_sha(&h.finalize())
}

pub fn authored_rgb16_bytes(pixels: &[DisplayEncodedRgb16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 6);
    for p in pixels {
        out.extend_from_slice(&p.r.to_be_bytes());
        out.extend_from_slice(&p.g.to_be_bytes());
        out.extend_from_slice(&p.b.to_be_bytes());
    }
    out
}

/// Apply the full presentation pipeline to a scientific color frame (immutable consume).
pub fn present_physical_color_frame(
    frame: &PhysicalColorFrame,
    spec: &PresentationSpec,
) -> Result<PresentationFrame, PresentationError> {
    spec.validate()?;
    let exposure = spec.exposure()?;
    let tone_op = spec.tone_operator()?;
    let gamut_op = spec.gamut_operator()?;
    let spec_digest = presentation_spec_digest(spec)?;
    let source_digest = physical_color_digest(frame).map_err(|e| {
        PresentationError::NonFiniteSourceColor(format!("source digest failed: {e}"))
    })?;

    let n = frame.pixels.len();
    if n != frame.grid.pixel_count() {
        return Err(PresentationError::FrameLengthMismatch);
    }

    let mut out_pixels = Vec::with_capacity(n);
    let mut negative_component_count_before_gamut = 0u64;
    let mut negative_pixel_count_before_gamut = 0u64;
    let mut gamut_adjusted_pixel_count = 0u64;
    let mut max_gamut_correction = 0.0_f64;
    let mut worst_gamut_raster_index = None;
    let mut source_disk_hit_count = 0u64;
    let mut pre_tone_max_rgb = 0.0_f64;
    let mut pre_tone_min_luma = f64::INFINITY;
    let mut pre_tone_max_luma = 0.0_f64;
    let mut pre_tone_lumas = Vec::new();
    let mut post_tone_min = f64::INFINITY;
    let mut post_tone_max = 0.0_f64;
    let mut endpoint_epsilon_canonicalization_count = 0u64;
    let mut final_code_min = u16::MAX;
    let mut final_code_max = 0u16;

    for (i, pixel) in frame.pixels.iter().enumerate() {
        let encoded = match pixel {
            PhysicalColorPixel::Absent { .. } => DisplayEncodedRgb16::BLACK,
            PhysicalColorPixel::DiskHit(hit) => {
                source_disk_hit_count += 1;
                let abs = LinearRgb::new(hit.rgb.r, hit.rgb.g, hit.rgb.b)?;
                let exposed = apply_exposure(abs, &exposure)?;
                let neg_n = exposed.negative_component_count();
                if neg_n > 0 {
                    negative_component_count_before_gamut += u64::from(neg_n);
                    negative_pixel_count_before_gamut += 1;
                }
                let (gamut, adjusted) = apply_gamut(gamut_op, exposed)?;
                if adjusted {
                    gamut_adjusted_pixel_count += 1;
                    let corr = (exposed.r - gamut.r)
                        .abs()
                        .max((exposed.g - gamut.g).abs())
                        .max((exposed.b - gamut.b).abs());
                    if corr >= max_gamut_correction {
                        max_gamut_correction = corr;
                        worst_gamut_raster_index = Some(i as u32);
                    }
                }
                let max_comp = gamut.r.max(gamut.g).max(gamut.b);
                pre_tone_max_rgb = pre_tone_max_rgb.max(max_comp);
                let luma = gamut.luminance_rec709();
                pre_tone_min_luma = pre_tone_min_luma.min(luma);
                pre_tone_max_luma = pre_tone_max_luma.max(luma);
                pre_tone_lumas.push(luma);

                let (mapped, eps_n) = apply_tone_map(tone_op, gamut)?;
                endpoint_epsilon_canonicalization_count += u64::from(eps_n);
                post_tone_min = post_tone_min.min(mapped.r).min(mapped.g).min(mapped.b);
                post_tone_max = post_tone_max.max(mapped.r).max(mapped.g).max(mapped.b);

                let enc =
                    DisplayEncodedRgb16::from_linear_display_rgb(mapped.r, mapped.g, mapped.b)?;
                final_code_min = final_code_min.min(enc.r).min(enc.g).min(enc.b);
                final_code_max = final_code_max.max(enc.r).max(enc.g).max(enc.b);
                enc
            }
        };
        out_pixels.push(encoded);
    }

    if !pre_tone_min_luma.is_finite() {
        pre_tone_min_luma = 0.0;
        pre_tone_max_luma = 0.0;
    }
    if !post_tone_min.is_finite() {
        post_tone_min = 0.0;
        post_tone_max = 0.0;
    }
    if final_code_min == u16::MAX {
        final_code_min = 0;
    }

    let median_luma = if pre_tone_lumas.is_empty() {
        0.0
    } else {
        pre_tone_lumas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = pre_tone_lumas.len() / 2;
        if pre_tone_lumas.len() % 2 == 0 {
            0.5 * (pre_tone_lumas[mid - 1] + pre_tone_lumas[mid])
        } else {
            pre_tone_lumas[mid]
        }
    };

    let frame_digest = presentation_frame_digest(
        &source_digest,
        &spec_digest,
        frame.grid.width,
        frame.grid.height,
        &out_pixels,
    );

    Ok(PresentationFrame {
        width: frame.grid.width,
        height: frame.grid.height,
        pixels: out_pixels,
        source_physical_color_digest: source_digest,
        presentation_spec_digest: spec_digest,
        presentation_frame_digest: frame_digest,
        metrics: PresentationMetrics {
            pixel_count: n as u64,
            source_disk_hit_count,
            negative_component_count_before_gamut,
            negative_pixel_count_before_gamut,
            gamut_adjusted_pixel_count,
            max_gamut_correction,
            worst_gamut_raster_index,
            pre_tone_max_rgb,
            pre_tone_min_luma,
            pre_tone_max_luma,
            pre_tone_median_luma_estimate: median_luma,
            post_tone_min,
            post_tone_max,
            endpoint_epsilon_canonicalization_count,
            final_code_min,
            final_code_max,
        },
    })
}

/// Fixed Gate 2D0 PNG metadata constants (A3/A4).
pub fn png_metadata_constants() -> (u8, u32) {
    (PNG_SRGB_INTENT_PERCEPTUAL, PNG_GAMA_SRGB)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_ev0_maps_middle_gray_to_018() {
        let e = ExposureSpec::new(2.41e9, 0.0).unwrap();
        let c = LinearRgb::new(2.41e9, 2.41e9, 2.41e9).unwrap();
        let o = apply_exposure(c, &e).unwrap();
        assert!((o.r - 0.18).abs() < 1e-12);
        assert!((o.g - 0.18).abs() < 1e-12);
        assert!((o.b - 0.18).abs() < 1e-12);
    }

    #[test]
    fn exposure_stop_ratio() {
        let e0 = ExposureSpec::new(1.0e9, 0.0).unwrap();
        let e1 = ExposureSpec::new(1.0e9, 1.0).unwrap();
        let em1 = ExposureSpec::new(1.0e9, -1.0).unwrap();
        let c = LinearRgb::new(1.0e9, 1.0e9, 1.0e9).unwrap();
        let a = apply_exposure(c, &e0).unwrap().r;
        let b = apply_exposure(c, &e1).unwrap().r;
        let d = apply_exposure(c, &em1).unwrap().r;
        assert!((b / a - 2.0).abs() < 1e-12);
        assert!((a / d - 2.0).abs() < 1e-12);
    }

    #[test]
    fn exposure_rejects_bad_lref() {
        assert!(ExposureSpec::new(0.0, 0.0).is_err());
        assert!(ExposureSpec::new(-1.0, 0.0).is_err());
        assert!(ExposureSpec::new(f64::NAN, 0.0).is_err());
        assert!(ExposureSpec::new(1.0, f64::INFINITY).is_err());
    }

    #[test]
    fn gamut_identity_nonneg_hdr() {
        let c = LinearRgb::new(0.1, 2.5, 10.0).unwrap();
        let (o, adj) = luminance_axis_desat_v1(c).unwrap();
        assert!(!adj);
        assert_eq!(o, c);
    }

    #[test]
    fn gamut_one_negative_channel() {
        let c = LinearRgb::new(-0.2, 0.5, 0.4).unwrap();
        let y = c.luminance_rec709();
        let (o, adj) = luminance_axis_desat_v1(c).unwrap();
        assert!(adj);
        assert!(o.r >= -GAMUT_EPS && o.g >= -GAMUT_EPS && o.b >= -GAMUT_EPS);
        assert!((o.luminance_rec709() - y).abs() <= GAMUT_EPS * (1.0 + y.abs()));
        // Maximum feasible t / minimum desaturation property: r lands ~0.
        assert!(o.r.abs() < 1e-10);
    }

    #[test]
    fn gamut_two_negative_channels() {
        let c = LinearRgb::new(-0.1, -0.05, 1.0).unwrap();
        let y = c.luminance_rec709();
        let (o, adj) = luminance_axis_desat_v1(c).unwrap();
        assert!(adj);
        assert!(o.r >= -1e-12 && o.g >= -1e-12 && o.b >= -1e-12);
        assert!((o.luminance_rec709() - y).abs() <= GAMUT_EPS * (1.0 + y.abs()));
    }

    #[test]
    fn gamut_near_zero_y_to_black() {
        let c = LinearRgb::new(-1e-15, 1e-15, 0.0).unwrap();
        let (o, adj) = luminance_axis_desat_v1(c).unwrap();
        assert!(adj);
        assert_eq!(o.r, 0.0);
        assert_eq!(o.g, 0.0);
        assert_eq!(o.b, 0.0);
    }

    #[test]
    fn gamut_rejects_significant_neg_luma() {
        let c = LinearRgb::new(-1.0, -1.0, -1.0).unwrap();
        assert!(luminance_axis_desat_v1(c).is_err());
    }

    #[test]
    fn gamut_rejects_nan() {
        assert!(luminance_axis_desat_v1(LinearRgb {
            r: f64::NAN,
            g: 0.0,
            b: 0.0
        })
        .is_err());
    }

    #[test]
    fn spec_digest_stable() {
        let a = PresentationSpec::v1(2.41e9, 0.0).unwrap();
        let b = PresentationSpec::v1(2.41e9, 0.0).unwrap();
        assert_eq!(
            presentation_spec_digest(&a).unwrap(),
            presentation_spec_digest(&b).unwrap()
        );
        let c = PresentationSpec::v1(2.41e9, 1.0).unwrap();
        assert_ne!(
            presentation_spec_digest(&a).unwrap(),
            presentation_spec_digest(&c).unwrap()
        );
    }
}
