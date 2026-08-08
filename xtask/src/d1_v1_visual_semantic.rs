//! Gate 2D1-V1 visual-semantic closure diagnostics (owner review evidence).
//!
//! Not scientific authority. Does not change beauty digests. Labels all outputs
//! as visual-semantic diagnostics for disk vs sky occlusion / appearance review.

use crate::render_presentation::write_beauty_png;
use relativity_render::{
    authored_rgb16_bytes, present_scene_appearance_frame, quantize_u16, srgb_oetf,
    AppearanceDiskColorFrame, AppearanceDiskEmissionFrame, AppearanceDiskEmissionPixel,
    DisplayEncodedRgb16, LinearRgb, PresentationFrame, PresentationMetrics, PresentationSpec,
    SceneAppearanceFrame, SceneAppearancePixel, REC709_LUMA_WB, REC709_LUMA_WG, REC709_LUMA_WR,
};
use relativity_trace::{OutcomeClass, TraceBundle};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct ClassLumaStats {
    pub count: u64,
    pub fraction: f64,
    pub luma_min: f64,
    pub luma_p50: f64,
    pub luma_p90: f64,
    pub luma_p99: f64,
    pub luma_max: f64,
    pub luma_mean: f64,
    pub integrated_luma: f64,
}

#[derive(Debug, Serialize)]
pub struct ModulationVisibilityStats {
    pub disk_lit_count: u64,
    pub modulation_min: f64,
    pub modulation_max: f64,
    pub modulation_mean: f64,
    pub modulation_std: f64,
    pub fraction_abs_delta_gt_0_05: f64,
    pub fraction_abs_delta_gt_0_15: f64,
}

#[derive(Debug, Serialize)]
pub struct BrightArcCandidate {
    pub col: u32,
    pub row: u32,
    pub pre_tone_luma: f64,
    pub radius_over_m: f64,
    pub azimuth: f64,
    pub g_factor: f64,
    pub f_base_w_m2: f64,
    pub f_app_w_m2: f64,
    pub modulation: f64,
    pub t_app_k: f64,
    pub classification_hint: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ResolutionConsistencyStats {
    pub low_width: u32,
    pub high_width: u32,
    pub mse_all_codes: f64,
    pub mse_disk_codes: f64,
    pub mse_escaped_codes: f64,
    pub mse_horizon_codes: f64,
    pub max_abs_code_delta: u16,
    pub note: &'static str,
}

#[derive(Debug, Serialize)]
pub struct VisualSemanticReport {
    pub diagnostic_id: &'static str,
    pub role: &'static str,
    pub width: u32,
    pub height: u32,
    pub disk_hit_count: u64,
    pub escaped_count: u64,
    pub horizon_count: u64,
    pub affine_limit_count: u64,
    pub failed_count: u64,
    pub source_mask_counts_match_bundle: bool,
    pub class_luma_pre_tone: ClassLumaBreakdown,
    pub modulation_visibility: ModulationVisibilityStats,
    pub bright_arc_candidates: Vec<BrightArcCandidate>,
    pub artifacts: Vec<&'static str>,
    pub resolution_consistency: Option<ResolutionConsistencyStats>,
}

#[derive(Debug, Serialize)]
pub struct ClassLumaBreakdown {
    pub disk: ClassLumaStats,
    pub escaped: ClassLumaStats,
    pub horizon: ClassLumaStats,
}

fn luma(rgb: LinearRgb) -> f64 {
    REC709_LUMA_WR * rgb.r + REC709_LUMA_WG * rgb.g + REC709_LUMA_WB * rgb.b
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn class_stats(values: &[f64], total_pixels: u64) -> ClassLumaStats {
    let count = values.len() as u64;
    if values.is_empty() {
        return ClassLumaStats {
            count: 0,
            fraction: 0.0,
            luma_min: 0.0,
            luma_p50: 0.0,
            luma_p90: 0.0,
            luma_p99: 0.0,
            luma_max: 0.0,
            luma_mean: 0.0,
            integrated_luma: 0.0,
        };
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let sum: f64 = values.iter().sum();
    ClassLumaStats {
        count,
        fraction: count as f64 / total_pixels as f64,
        luma_min: sorted[0],
        luma_p50: percentile(&sorted, 0.50),
        luma_p90: percentile(&sorted, 0.90),
        luma_p99: percentile(&sorted, 0.99),
        luma_max: *sorted.last().unwrap(),
        luma_mean: sum / count as f64,
        integrated_luma: sum,
    }
}

fn mask_scene(
    scene: &SceneAppearanceFrame,
    keep: impl Fn(OutcomeClass) -> bool,
) -> SceneAppearanceFrame {
    let pixels = scene
        .pixels
        .iter()
        .map(|p| {
            if keep(p.outcome_class) {
                *p
            } else {
                SceneAppearancePixel {
                    rgb: LinearRgb {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                    },
                    outcome_class: p.outcome_class,
                }
            }
        })
        .collect();
    SceneAppearanceFrame {
        pixels,
        scene_appearance_digest: format!("{}-masked-diagnostic", scene.scene_appearance_digest),
        ..scene.clone()
    }
}

fn present_diag(
    scene: &SceneAppearanceFrame,
    presentation: &PresentationSpec,
    label: &str,
) -> Result<PresentationFrame, Box<dyn std::error::Error>> {
    Ok(present_scene_appearance_frame(
        scene,
        presentation,
        &format!("d1-v1-diagnostic:{label}"),
    )?)
}

/// Encode pre-tone scene-linear with clamp→OETF only (no PBR Neutral). Diagnostic only.
fn encode_pre_tone_diagnostic(
    scene: &SceneAppearanceFrame,
) -> Result<PresentationFrame, Box<dyn std::error::Error>> {
    let mut pixels = Vec::with_capacity(scene.pixels.len());
    let mut final_code_min = u16::MAX;
    let mut final_code_max = 0u16;
    for p in &scene.pixels {
        let r = srgb_oetf(p.rgb.r.clamp(0.0, 1.0))?;
        let g = srgb_oetf(p.rgb.g.clamp(0.0, 1.0))?;
        let b = srgb_oetf(p.rgb.b.clamp(0.0, 1.0))?;
        let enc = DisplayEncodedRgb16 {
            r: quantize_u16(r)?,
            g: quantize_u16(g)?,
            b: quantize_u16(b)?,
        };
        final_code_min = final_code_min.min(enc.r).min(enc.g).min(enc.b);
        final_code_max = final_code_max.max(enc.r).max(enc.g).max(enc.b);
        pixels.push(enc);
    }
    if final_code_min == u16::MAX {
        final_code_min = 0;
    }
    Ok(PresentationFrame {
        width: scene.grid.width,
        height: scene.grid.height,
        pixels,
        source_physical_color_digest: "d1-v1-pre-tone-diagnostic".into(),
        presentation_spec_digest: "d1-v1-pre-tone-clamp-oetf".into(),
        presentation_frame_digest: "d1-v1-pre-tone-diagnostic".into(),
        metrics: PresentationMetrics {
            pixel_count: scene.pixels.len() as u64,
            source_disk_hit_count: scene.disk_hit_count,
            negative_component_count_before_gamut: 0,
            negative_pixel_count_before_gamut: 0,
            gamut_adjusted_pixel_count: 0,
            max_gamut_correction: 0.0,
            worst_gamut_raster_index: None,
            pre_tone_max_rgb: 0.0,
            pre_tone_min_luma: 0.0,
            pre_tone_max_luma: 0.0,
            pre_tone_median_luma_estimate: 0.0,
            post_tone_min: 0.0,
            post_tone_max: 0.0,
            endpoint_epsilon_canonicalization_count: 0,
            final_code_min,
            final_code_max,
        },
    })
}

fn write_source_mask_ppm(
    path: &Path,
    scene: &SceneAppearanceFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let w = scene.grid.width;
    let h = scene.grid.height;
    let mut body = Vec::with_capacity((w * h * 3) as usize);
    for p in &scene.pixels {
        let rgb = match p.outcome_class {
            OutcomeClass::DiskHit => [255u8, 64, 64],
            OutcomeClass::Escaped => [64, 160, 255],
            OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => [0, 0, 0],
            OutcomeClass::AffineLimit => [255, 255, 0],
            OutcomeClass::Failed => [255, 0, 255],
        };
        body.extend_from_slice(&rgb);
    }
    let header = format!("P6\n{w} {h}\n255\n");
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(&body);
    std::fs::write(path, bytes)?;
    Ok(())
}

fn modulation_stats(app_em: &AppearanceDiskEmissionFrame) -> ModulationVisibilityStats {
    let mut mods = Vec::new();
    for pix in &app_em.pixels {
        if let AppearanceDiskEmissionPixel::DiskHit(s) = pix {
            if s.f_base_w_m2 > 0.0 {
                mods.push(s.modulation);
            }
        }
    }
    if mods.is_empty() {
        return ModulationVisibilityStats {
            disk_lit_count: 0,
            modulation_min: 1.0,
            modulation_max: 1.0,
            modulation_mean: 1.0,
            modulation_std: 0.0,
            fraction_abs_delta_gt_0_05: 0.0,
            fraction_abs_delta_gt_0_15: 0.0,
        };
    }
    let n = mods.len() as f64;
    let mean = mods.iter().sum::<f64>() / n;
    let var = mods.iter().map(|m| (m - mean).powi(2)).sum::<f64>() / n;
    let gt05 = mods.iter().filter(|m| (*m - 1.0).abs() > 0.05).count() as f64 / n;
    let gt15 = mods.iter().filter(|m| (*m - 1.0).abs() > 0.15).count() as f64 / n;
    ModulationVisibilityStats {
        disk_lit_count: mods.len() as u64,
        modulation_min: mods.iter().cloned().fold(f64::INFINITY, f64::min),
        modulation_max: mods.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        modulation_mean: mean,
        modulation_std: var.sqrt(),
        fraction_abs_delta_gt_0_05: gt05,
        fraction_abs_delta_gt_0_15: gt15,
    }
}

fn bright_arc_candidates(
    scene: &SceneAppearanceFrame,
    app_em: &AppearanceDiskEmissionFrame,
    app_color: &AppearanceDiskColorFrame,
) -> Vec<BrightArcCandidate> {
    let mut disk_lumas = Vec::new();
    for (i, p) in scene.pixels.iter().enumerate() {
        if p.outcome_class == OutcomeClass::DiskHit {
            disk_lumas.push((i, luma(p.rgb)));
        }
    }
    if disk_lumas.is_empty() {
        return Vec::new();
    }
    disk_lumas.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let take = (disk_lumas.len() / 1000).clamp(8, 32);
    let w = scene.grid.width;
    let mut out = Vec::new();
    for &(i, y) in disk_lumas.iter().take(take) {
        let col = (i as u32) % w;
        let row = (i as u32) / w;
        let (radius, az, g, f_base, f_app, m, t_app) = match &app_em.pixels[i] {
            AppearanceDiskEmissionPixel::DiskHit(s) => (
                s.radius_over_m,
                s.azimuth,
                s.g_factor,
                s.f_base_w_m2,
                s.f_app_w_m2,
                s.modulation,
                s.t_app_k,
            ),
            _ => continue,
        };
        let _ = (app_color, i); // shared pixel index joins emission/color/scene
        let hint = if g > 1.5 {
            "likely_high_g_lensed_disk_structure"
        } else if m > 1.2 {
            "appearance_modulation_peak"
        } else {
            "bright_disk_pixel_review"
        };
        out.push(BrightArcCandidate {
            col,
            row,
            pre_tone_luma: y,
            radius_over_m: radius,
            azimuth: az,
            g_factor: g,
            f_base_w_m2: f_base,
            f_app_w_m2: f_app,
            modulation: m,
            t_app_k: t_app,
            classification_hint: hint,
        });
    }
    out
}

fn counts_match_bundle(scene: &SceneAppearanceFrame, bundle: &TraceBundle) -> bool {
    if scene.grid != bundle.grid || scene.pixels.len() != bundle.outcomes.len() {
        return false;
    }
    let mut d = 0u64;
    let mut e = 0u64;
    let mut h = 0u64;
    let mut a = 0u64;
    let mut f = 0u64;
    for (sp, out) in scene.pixels.iter().zip(bundle.outcomes.iter()) {
        let oc = out.class();
        if sp.outcome_class != oc {
            return false;
        }
        match oc {
            OutcomeClass::DiskHit => d += 1,
            OutcomeClass::Escaped => e += 1,
            OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => h += 1,
            OutcomeClass::AffineLimit => a += 1,
            OutcomeClass::Failed => f += 1,
        }
    }
    d == scene.disk_hit_count
        && e == scene.escaped_count
        && h == scene.horizon_count
        && a == scene.affine_limit_count
        && f == scene.failed_count
}

/// Box-filter downsample RGB16 codes by integer factor.
pub fn downsample_rgb16_box(
    width: u32,
    height: u32,
    pixels: &[DisplayEncodedRgb16],
    factor: u32,
) -> Result<(u32, u32, Vec<DisplayEncodedRgb16>), Box<dyn std::error::Error>> {
    if factor == 0 || !width.is_multiple_of(factor) || !height.is_multiple_of(factor) {
        return Err("downsample factor must divide width/height".into());
    }
    let nw = width / factor;
    let nh = height / factor;
    let mut out = Vec::with_capacity((nw * nh) as usize);
    for row in 0..nh {
        for col in 0..nw {
            let mut sr = 0u64;
            let mut sg = 0u64;
            let mut sb = 0u64;
            let mut n = 0u64;
            for dy in 0..factor {
                for dx in 0..factor {
                    let x = col * factor + dx;
                    let y = row * factor + dy;
                    let p = pixels[(y * width + x) as usize];
                    sr += u64::from(p.r);
                    sg += u64::from(p.g);
                    sb += u64::from(p.b);
                    n += 1;
                }
            }
            out.push(DisplayEncodedRgb16 {
                r: (sr / n) as u16,
                g: (sg / n) as u16,
                b: (sb / n) as u16,
            });
        }
    }
    Ok((nw, nh, out))
}

pub fn compare_resolution_rasters(
    low: &PresentationFrame,
    high: &PresentationFrame,
    low_mask: &SceneAppearanceFrame,
) -> Result<ResolutionConsistencyStats, Box<dyn std::error::Error>> {
    if !high.width.is_multiple_of(low.width) || !high.height.is_multiple_of(low.height) {
        return Err("high-res dimensions must be integer multiple of low-res".into());
    }
    let fx = high.width / low.width;
    let fy = high.height / low.height;
    if fx != fy {
        return Err("anisotropic downsample not supported".into());
    }
    let (_nw, _nh, down) = downsample_rgb16_box(high.width, high.height, &high.pixels, fx)?;
    if down.len() != low.pixels.len() {
        return Err("downsampled length mismatch".into());
    }
    let mut sse_all = 0.0;
    let mut sse_disk = 0.0;
    let mut sse_esc = 0.0;
    let mut sse_hor = 0.0;
    let mut n_disk = 0.0;
    let mut n_esc = 0.0;
    let mut n_hor = 0.0;
    let mut max_abs = 0u16;
    for (i, (a, b)) in low.pixels.iter().zip(down.iter()).enumerate() {
        let dr = a.r.abs_diff(b.r);
        let dg = a.g.abs_diff(b.g);
        let db = a.b.abs_diff(b.b);
        max_abs = max_abs.max(dr).max(dg).max(db);
        let e = (f64::from(dr).powi(2) + f64::from(dg).powi(2) + f64::from(db).powi(2)) / 3.0;
        sse_all += e;
        match low_mask.pixels[i].outcome_class {
            OutcomeClass::DiskHit => {
                sse_disk += e;
                n_disk += 1.0;
            }
            OutcomeClass::Escaped => {
                sse_esc += e;
                n_esc += 1.0;
            }
            OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => {
                sse_hor += e;
                n_hor += 1.0;
            }
            _ => {}
        }
    }
    let n = low.pixels.len() as f64;
    Ok(ResolutionConsistencyStats {
        low_width: low.width,
        high_width: high.width,
        mse_all_codes: sse_all / n,
        mse_disk_codes: if n_disk > 0.0 { sse_disk / n_disk } else { 0.0 },
        mse_escaped_codes: if n_esc > 0.0 { sse_esc / n_esc } else { 0.0 },
        mse_horizon_codes: if n_hor > 0.0 { sse_hor / n_hor } else { 0.0 },
        max_abs_code_delta: max_abs,
        note: "diagnostic only — not a frozen IQ tolerance; E2 trigger if escaped/critical-curve mismatch persists",
    })
}

pub fn write_visual_semantic_diagnostics(
    out_dir: &Path,
    scene: &SceneAppearanceFrame,
    bundle: &TraceBundle,
    app_em: &AppearanceDiskEmissionFrame,
    app_color: &AppearanceDiskColorFrame,
    presentation: &PresentationSpec,
    resolution_consistency: Option<ResolutionConsistencyStats>,
) -> Result<VisualSemanticReport, Box<dyn std::error::Error>> {
    let diag_dir = out_dir.join("d1-v1-visual-semantic");
    std::fs::create_dir_all(&diag_dir)?;

    let total = scene.pixels.len() as u64;
    let mut disk_l = Vec::new();
    let mut esc_l = Vec::new();
    let mut hor_l = Vec::new();
    for p in &scene.pixels {
        let y = luma(p.rgb);
        match p.outcome_class {
            OutcomeClass::DiskHit => disk_l.push(y),
            OutcomeClass::Escaped => esc_l.push(y),
            OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => hor_l.push(y),
            _ => {}
        }
    }

    let disk_only = mask_scene(scene, |oc| oc == OutcomeClass::DiskHit);
    let env_only = mask_scene(scene, |oc| oc == OutcomeClass::Escaped);
    let disk_pres = present_diag(&disk_only, presentation, "disk-only")?;
    let env_pres = present_diag(&env_only, presentation, "lensed-environment-only")?;
    let pre_tone = encode_pre_tone_diagnostic(scene)?;

    write_beauty_png(&diag_dir.join("disk-only-srgb16.png"), &disk_pres)?;
    write_beauty_png(
        &diag_dir.join("lensed-environment-only-srgb16.png"),
        &env_pres,
    )?;
    write_beauty_png(&diag_dir.join("pre-tone-clamp-oetf-srgb16.png"), &pre_tone)?;
    write_source_mask_ppm(&diag_dir.join("source-mask.ppm"), scene)?;

    // Also keep authored RGB16 bytes for mask-side MSE tools.
    std::fs::write(
        diag_dir.join("disk-only.rgb16"),
        authored_rgb16_bytes(&disk_pres.pixels),
    )?;
    std::fs::write(
        diag_dir.join("lensed-environment-only.rgb16"),
        authored_rgb16_bytes(&env_pres.pixels),
    )?;

    let report = VisualSemanticReport {
        diagnostic_id: "gate-2d1-v1-visual-semantic",
        role: "VISUAL_SEMANTIC_DIAGNOSTIC_NOT_BEAUTY_AUTHORITY",
        width: scene.grid.width,
        height: scene.grid.height,
        disk_hit_count: scene.disk_hit_count,
        escaped_count: scene.escaped_count,
        horizon_count: scene.horizon_count,
        affine_limit_count: scene.affine_limit_count,
        failed_count: scene.failed_count,
        source_mask_counts_match_bundle: counts_match_bundle(scene, bundle),
        class_luma_pre_tone: ClassLumaBreakdown {
            disk: class_stats(&disk_l, total),
            escaped: class_stats(&esc_l, total),
            horizon: class_stats(&hor_l, total),
        },
        modulation_visibility: modulation_stats(app_em),
        bright_arc_candidates: bright_arc_candidates(scene, app_em, app_color),
        artifacts: vec![
            "d1-v1-visual-semantic/disk-only-srgb16.png",
            "d1-v1-visual-semantic/lensed-environment-only-srgb16.png",
            "d1-v1-visual-semantic/pre-tone-clamp-oetf-srgb16.png",
            "d1-v1-visual-semantic/source-mask.ppm",
            "d1-v1-visual-semantic/visual-semantic-report.json",
        ],
        resolution_consistency,
    };
    std::fs::write(
        diag_dir.join("visual-semantic-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(report)
}

pub fn compare_resolution_from_artifacts(
    low_png: &Path,
    high_png: &Path,
    mask_ppm: &Path,
) -> Result<ResolutionConsistencyStats, Box<dyn std::error::Error>> {
    let (lw, lh, low_pix) = decode_png_rgb16(low_png)?;
    let (hw, hh, high_pix) = decode_png_rgb16(high_png)?;
    let (mw, mh, mask) = decode_mask_ppm(mask_ppm)?;
    if (mw, mh) != (lw, lh) {
        return Err("mask size != low beauty size".into());
    }
    let low = PresentationFrame {
        width: lw,
        height: lh,
        pixels: low_pix,
        source_physical_color_digest: String::new(),
        presentation_spec_digest: String::new(),
        presentation_frame_digest: String::new(),
        metrics: empty_metrics(lw, lh),
    };
    let high = PresentationFrame {
        width: hw,
        height: hh,
        pixels: high_pix,
        source_physical_color_digest: String::new(),
        presentation_spec_digest: String::new(),
        presentation_frame_digest: String::new(),
        metrics: empty_metrics(hw, hh),
    };
    // Build a lightweight mask frame for class MSE.
    let pixels = mask
        .into_iter()
        .map(|oc| SceneAppearancePixel {
            rgb: LinearRgb {
                r: 0.0,
                g: 0.0,
                b: 0.0,
            },
            outcome_class: oc,
        })
        .collect();
    let mask_frame = SceneAppearanceFrame {
        grid: relativity_trace::TraceGrid {
            width: lw,
            height: lh,
        },
        pixels,
        model_id: "mask".into(),
        source_physical_color_digest: String::new(),
        disk_appearance_digest: String::new(),
        environment_spec_digest: String::new(),
        scene_appearance_digest: String::new(),
        affine_limit_count: 0,
        failed_count: 0,
        disk_hit_count: 0,
        escaped_count: 0,
        horizon_count: 0,
        integrated_luma_appearance: 0.0,
        integrated_luma_base_disk: 0.0,
    };
    compare_resolution_rasters(&low, &high, &mask_frame)
}

fn empty_metrics(w: u32, h: u32) -> PresentationMetrics {
    PresentationMetrics {
        pixel_count: u64::from(w) * u64::from(h),
        source_disk_hit_count: 0,
        negative_component_count_before_gamut: 0,
        negative_pixel_count_before_gamut: 0,
        gamut_adjusted_pixel_count: 0,
        max_gamut_correction: 0.0,
        worst_gamut_raster_index: None,
        pre_tone_max_rgb: 0.0,
        pre_tone_min_luma: 0.0,
        pre_tone_max_luma: 0.0,
        pre_tone_median_luma_estimate: 0.0,
        post_tone_min: 0.0,
        post_tone_max: 0.0,
        endpoint_epsilon_canonicalization_count: 0,
        final_code_min: 0,
        final_code_max: 0,
    }
}

fn decode_png_rgb16(
    path: &Path,
) -> Result<(u32, u32, Vec<DisplayEncodedRgb16>), Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    let w = info.width;
    let h = info.height;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("PNG buffer")?];
    let frame = reader.next_frame(&mut buf)?;
    let data = &buf[..frame.buffer_size()];
    if data.len() != (w * h * 6) as usize {
        return Err("unexpected PNG RGB16 size".into());
    }
    let mut pixels = Vec::with_capacity((w * h) as usize);
    for chunk in data.chunks_exact(6) {
        pixels.push(DisplayEncodedRgb16 {
            r: u16::from_be_bytes([chunk[0], chunk[1]]),
            g: u16::from_be_bytes([chunk[2], chunk[3]]),
            b: u16::from_be_bytes([chunk[4], chunk[5]]),
        });
    }
    Ok((w, h, pixels))
}

fn decode_mask_ppm(
    path: &Path,
) -> Result<(u32, u32, Vec<OutcomeClass>), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.split('\n');
    let magic = lines.next().ok_or("ppm magic")?;
    if magic.trim() != "P6" {
        return Err("expected P6 mask".into());
    }
    let dims = lines.next().ok_or("ppm dims")?;
    let mut parts = dims.split_whitespace();
    let w: u32 = parts.next().ok_or("w")?.parse()?;
    let h: u32 = parts.next().ok_or("h")?.parse()?;
    let _max = lines.next().ok_or("ppm max")?;
    // Binary payload starts after header; find second newline after "255\n"
    let header_end = bytes
        .windows(4)
        .position(|w| w == b"255\n")
        .ok_or("ppm header end")?
        + 4;
    let data = &bytes[header_end..];
    if data.len() != (w * h * 3) as usize {
        return Err("ppm payload size mismatch".into());
    }
    let mut out = Vec::with_capacity((w * h) as usize);
    for chunk in data.chunks_exact(3) {
        let oc = match chunk {
            [255, 64, 64] => OutcomeClass::DiskHit,
            [64, 160, 255] => OutcomeClass::Escaped,
            [0, 0, 0] => OutcomeClass::HorizonEvent,
            [255, 255, 0] => OutcomeClass::AffineLimit,
            [255, 0, 255] => OutcomeClass::Failed,
            _ => OutcomeClass::Failed,
        };
        out.push(oc);
    }
    Ok((w, h, out))
}
