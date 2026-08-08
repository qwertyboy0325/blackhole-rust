//! Gate 2D1 scene appearance composition (DISPLAY + ARTISTIC layers).
//!
//! S2: middle-gray-relative scene-linear Rec.709/D65, then Gate 2D0 post-exposure
//! presentation via `present_exposed_linear_rgb` (A4/A5).

use crate::celestial_environment::{
    sample_environment_linear, CelestialEnvironment, EnvironmentSpec,
};
use crate::colorimetry::{physical_color_digest, PhysicalColorFrame, PhysicalColorPixel};
use crate::disk_appearance::{AppearanceDiskColorFrame, AppearanceDiskColorPixel};
use crate::error::AppearanceError;
use crate::presentation::{
    present_exposed_linear_rgb, presentation_spec_digest, ExposedLinearPixel, PresentationFrame,
    PresentationSpec, REC709_LUMA_WB, REC709_LUMA_WG, REC709_LUMA_WR,
};
use crate::tone_map::LinearRgb;
use relativity_trace::{
    hex_sha, CelestialCoordinateFrame, CelestialCoordinatePixel, OutcomeClass, TraceBundle,
    TraceGrid,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const SCENE_APPEARANCE_MODEL_ID: &str = "scene-appearance-v1";

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SceneAppearancePixel {
    pub rgb: LinearRgb,
    pub outcome_class: OutcomeClass,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneAppearanceFrame {
    pub grid: TraceGrid,
    pub pixels: Vec<SceneAppearancePixel>,
    pub model_id: String,
    pub source_physical_color_digest: String,
    pub disk_appearance_digest: String,
    pub environment_spec_digest: String,
    pub scene_appearance_digest: String,
    pub affine_limit_count: u64,
    pub failed_count: u64,
    pub disk_hit_count: u64,
    pub escaped_count: u64,
    pub horizon_count: u64,
    /// Diagnostic: relative change in integrated Rec.709 luma vs identity disk (A3).
    pub integrated_luma_appearance: f64,
    pub integrated_luma_base_disk: f64,
}

pub fn scene_linear_from_absolute(
    abs: LinearRgb,
    middle_gray_luminance_cd_m2: f64,
) -> Result<LinearRgb, AppearanceError> {
    if !(middle_gray_luminance_cd_m2.is_finite() && middle_gray_luminance_cd_m2 > 0.0) {
        return Err(AppearanceError::InvalidSpec(
            "middle_gray_luminance_cd_m2 must be finite > 0".into(),
        ));
    }
    let scale = 0.18 / middle_gray_luminance_cd_m2;
    LinearRgb::new(abs.r * scale, abs.g * scale, abs.b * scale)
        .map_err(|e| AppearanceError::Presentation(e.to_string()))
}

fn apply_ev(rgb: LinearRgb, exposure_ev: f64) -> Result<LinearRgb, AppearanceError> {
    if !exposure_ev.is_finite() {
        return Err(AppearanceError::InvalidSpec(
            "exposure_ev must be finite".into(),
        ));
    }
    let s = (2.0_f64).powf(exposure_ev);
    LinearRgb::new(rgb.r * s, rgb.g * s, rgb.b * s)
        .map_err(|e| AppearanceError::Presentation(e.to_string()))
}

fn luma(rgb: LinearRgb) -> f64 {
    REC709_LUMA_WR * rgb.r + REC709_LUMA_WG * rgb.g + REC709_LUMA_WB * rgb.b
}

fn outcome_from_bundle(bundle: &TraceBundle, col: u32, row: u32) -> OutcomeClass {
    bundle.outcome_at(col, row).class()
}

/// Compose one TraceBundle into a scene-linear appearance frame (pre-tone).
pub fn build_scene_appearance_frame(
    bundle: &TraceBundle,
    physical_color: &PhysicalColorFrame,
    appearance_disk_color: &AppearanceDiskColorFrame,
    celestial: &CelestialCoordinateFrame,
    environment: &CelestialEnvironment,
    environment_spec_digest: &str,
    presentation: &PresentationSpec,
) -> Result<SceneAppearanceFrame, AppearanceError> {
    let grid = physical_color.grid;
    if appearance_disk_color.grid != grid || celestial.grid() != grid || bundle.grid != grid {
        return Err(AppearanceError::GridMismatch);
    }
    let n = grid.pixel_count();
    if physical_color.pixels.len() != n
        || appearance_disk_color.pixels.len() != n
        || celestial.pixels().len() != n
        || bundle.outcomes.len() != n
    {
        return Err(AppearanceError::FrameLengthMismatch);
    }

    let source_physical_color_digest = physical_color_digest(physical_color)
        .map_err(|e| AppearanceError::Colorimetry(e.to_string()))?;
    let disk_appearance_digest = appearance_disk_color.disk_appearance_spec_digest.clone();

    let mut pixels = Vec::with_capacity(n);
    let mut affine_limit_count = 0u64;
    let mut failed_count = 0u64;
    let mut disk_hit_count = 0u64;
    let mut escaped_count = 0u64;
    let mut horizon_count = 0u64;
    let mut integrated_luma_appearance = 0.0;
    let mut integrated_luma_base_disk = 0.0;

    let l_ref = presentation.middle_gray_luminance_cd_m2;
    let ev = presentation.exposure_ev;

    for row in 0..grid.height {
        for col in 0..grid.width {
            let idx = relativity_trace::pixel_index(grid, col, row);
            let oc = outcome_from_bundle(bundle, col, row);

            // Parity across frames.
            let phys = &physical_color.pixels[idx];
            let app = &appearance_disk_color.pixels[idx];
            let cel = &celestial.pixels()[idx];
            match oc {
                OutcomeClass::DiskHit => match (phys, app, cel) {
                    (
                        PhysicalColorPixel::DiskHit(_) | PhysicalColorPixel::Absent { .. },
                        AppearanceDiskColorPixel::DiskHit(_)
                        | AppearanceDiskColorPixel::Absent { .. },
                        CelestialCoordinatePixel::NotEscaped {
                            outcome_class: OutcomeClass::DiskHit,
                        },
                    ) => {}
                    _ => {
                        return Err(AppearanceError::SceneOutcomeParity {
                            col,
                            row,
                            detail: "DiskHit mismatch phys/app/cel".into(),
                        });
                    }
                },
                OutcomeClass::Escaped => match (phys, app, cel) {
                    (
                        PhysicalColorPixel::Absent {
                            outcome_class: OutcomeClass::Escaped,
                        },
                        AppearanceDiskColorPixel::Absent {
                            outcome_class: OutcomeClass::Escaped,
                        },
                        CelestialCoordinatePixel::Escaped(_),
                    ) => {}
                    _ => {
                        return Err(AppearanceError::SceneOutcomeParity {
                            col,
                            row,
                            detail: "Escaped mismatch".into(),
                        });
                    }
                },
                OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => {
                    match (phys, app, cel) {
                        (
                            PhysicalColorPixel::Absent { outcome_class },
                            AppearanceDiskColorPixel::Absent { outcome_class: aoc },
                            CelestialCoordinatePixel::NotEscaped { outcome_class: coc },
                        ) if *outcome_class == oc && *aoc == oc && *coc == oc => {}
                        _ => {
                            return Err(AppearanceError::SceneOutcomeParity {
                                col,
                                row,
                                detail: format!("Horizon mismatch {oc:?}"),
                            });
                        }
                    }
                }
                OutcomeClass::AffineLimit => {
                    affine_limit_count += 1;
                }
                OutcomeClass::Failed => {
                    failed_count += 1;
                }
            }

            let rgb_ev0 = match oc {
                OutcomeClass::DiskHit => {
                    disk_hit_count += 1;
                    match app {
                        AppearanceDiskColorPixel::DiskHit(s) => {
                            let abs = LinearRgb::new(s.rgb.r, s.rgb.g, s.rgb.b)
                                .map_err(|e| AppearanceError::Presentation(e.to_string()))?;
                            let ev0 = scene_linear_from_absolute(abs, l_ref)?;
                            integrated_luma_appearance += luma(ev0);
                            if let PhysicalColorPixel::DiskHit(base) = phys {
                                let base_abs = LinearRgb::new(base.rgb.r, base.rgb.g, base.rgb.b)
                                    .map_err(|e| {
                                    AppearanceError::Presentation(e.to_string())
                                })?;
                                let base_ev0 = scene_linear_from_absolute(base_abs, l_ref)?;
                                integrated_luma_base_disk += luma(base_ev0);
                            }
                            ev0
                        }
                        AppearanceDiskColorPixel::Absent { .. } => LinearRgb {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                        },
                    }
                }
                OutcomeClass::Escaped => {
                    escaped_count += 1;
                    match cel {
                        CelestialCoordinatePixel::Escaped(sample) => {
                            sample_environment_linear(environment, sample)?
                        }
                        _ => {
                            return Err(AppearanceError::SceneOutcomeParity {
                                col,
                                row,
                                detail: "Escaped without celestial sample".into(),
                            });
                        }
                    }
                }
                OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => {
                    horizon_count += 1;
                    LinearRgb {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                    }
                }
                OutcomeClass::AffineLimit | OutcomeClass::Failed => LinearRgb {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                },
            };

            let rgb = apply_ev(rgb_ev0, ev)?;
            pixels.push(SceneAppearancePixel {
                rgb,
                outcome_class: oc,
            });
        }
    }

    if affine_limit_count > 0 || failed_count > 0 {
        return Err(AppearanceError::SceneNumericalFailure {
            affine_limit: affine_limit_count,
            failed: failed_count,
        });
    }

    let mut h = Sha256::new();
    h.update(b"scene-appearance-digest-v1");
    h.update(b"APPEARANCE_REPRODUCIBILITY_DIGEST");
    h.update(source_physical_color_digest.as_bytes());
    h.update(disk_appearance_digest.as_bytes());
    h.update(environment_spec_digest.as_bytes());
    h.update(grid.width.to_le_bytes());
    h.update(grid.height.to_le_bytes());
    for p in &pixels {
        h.update(p.rgb.r.to_bits().to_le_bytes());
        h.update(p.rgb.g.to_bits().to_le_bytes());
        h.update(p.rgb.b.to_bits().to_le_bytes());
        h.update([crate::colorimetry::outcome_class_code(p.outcome_class)]);
    }
    let scene_appearance_digest = hex_sha(&h.finalize());

    Ok(SceneAppearanceFrame {
        grid,
        pixels,
        model_id: SCENE_APPEARANCE_MODEL_ID.into(),
        source_physical_color_digest,
        disk_appearance_digest,
        environment_spec_digest: environment_spec_digest.into(),
        scene_appearance_digest,
        affine_limit_count,
        failed_count,
        disk_hit_count,
        escaped_count,
        horizon_count,
        integrated_luma_appearance,
        integrated_luma_base_disk,
    })
}

/// Present a scene-linear (already EV-scaled) frame through Gate 2D0 post-exposure tail (A4).
///
/// `presentation_source_digest` is hashed into `presentation_frame_digest`.
/// Identity scene must pass Gate 2C1 `physical_color_digest` for A5.
pub fn present_scene_appearance_frame(
    scene: &SceneAppearanceFrame,
    presentation: &PresentationSpec,
    presentation_source_digest: &str,
) -> Result<PresentationFrame, AppearanceError> {
    presentation
        .validate()
        .map_err(|e| AppearanceError::Presentation(e.to_string()))?;
    let spec_digest = presentation_spec_digest(presentation)
        .map_err(|e| AppearanceError::Presentation(e.to_string()))?;
    let gamut = presentation
        .gamut_operator()
        .map_err(|e| AppearanceError::Presentation(e.to_string()))?;
    let tone = presentation
        .tone_operator()
        .map_err(|e| AppearanceError::Presentation(e.to_string()))?;

    let mut exposed = Vec::with_capacity(scene.pixels.len());
    for p in &scene.pixels {
        match p.outcome_class {
            OutcomeClass::HorizonEvent
            | OutcomeClass::HorizonApproach
            | OutcomeClass::AffineLimit
            | OutcomeClass::Failed => {
                exposed.push(ExposedLinearPixel::Black);
            }
            OutcomeClass::DiskHit => {
                if p.rgb.r == 0.0 && p.rgb.g == 0.0 && p.rgb.b == 0.0 {
                    exposed.push(ExposedLinearPixel::Black);
                } else {
                    exposed.push(ExposedLinearPixel::ExposedLinear {
                        rgb: p.rgb,
                        count_as_lit: true,
                    });
                }
            }
            OutcomeClass::Escaped => {
                if p.rgb.r == 0.0 && p.rgb.g == 0.0 && p.rgb.b == 0.0 {
                    // Identity black sky matches Gate 2D0 Absent → Black.
                    exposed.push(ExposedLinearPixel::Black);
                } else {
                    exposed.push(ExposedLinearPixel::ExposedLinear {
                        rgb: p.rgb,
                        count_as_lit: false,
                    });
                }
            }
        }
    }

    present_exposed_linear_rgb(
        scene.grid.width,
        scene.grid.height,
        &exposed,
        gamut,
        tone,
        presentation_source_digest,
        &spec_digest,
    )
    .map_err(|e| AppearanceError::Presentation(e.to_string()))
}

/// A5 identity helpers: force disk identity + black environment.
pub fn is_identity_scene_config(disk_identity: bool, env: &EnvironmentSpec) -> bool {
    disk_identity && env.identity_black
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_gray_scale() {
        let abs = LinearRgb::new(2.41e9, 2.41e9, 2.41e9).unwrap();
        let ev0 = scene_linear_from_absolute(abs, 2.41e9).unwrap();
        assert!((ev0.r - 0.18).abs() < 1e-12);
    }
}
