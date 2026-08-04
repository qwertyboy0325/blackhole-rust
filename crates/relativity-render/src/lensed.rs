//! Lensed celestial diagnostic frames (Gate 2A2).

use crate::error::CelestialRenderError;
use crate::texture::{sample_procedural_celestial, ProceduralCelestialTextureSpec};
use relativity_trace::{
    encode_ppm, hex_sha, pixel_index, CelestialCoordinateFrame, CelestialCoordinatePixel,
    OutcomeClass, RgbFrame, TraceSurfaceSet,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LensedCelestialMode {
    OpaqueDiskMask,
    DiskOmittedDiagnostic,
}

impl LensedCelestialMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpaqueDiskMask => "opaque-disk-mask",
            Self::DiskOmittedDiagnostic => "disk-omitted-diagnostic",
        }
    }

    pub const fn digest_tag(self) -> &'static str {
        match self {
            Self::OpaqueDiskMask => "lensed-celestial-mode:opaque-disk-mask",
            Self::DiskOmittedDiagnostic => "lensed-celestial-mode:disk-omitted-diagnostic",
        }
    }

    pub const fn required_surface_set(self) -> TraceSurfaceSet {
        match self {
            Self::OpaqueDiskMask => TraceSurfaceSet::OpaqueDiskHorizonEscape,
            Self::DiskOmittedDiagnostic => TraceSurfaceSet::HorizonEscapeOnly,
        }
    }

    pub const fn ppm_filename(self) -> &'static str {
        match self {
            Self::OpaqueDiskMask => "lensed-celestial-opaque-disk-mask.ppm",
            Self::DiskOmittedDiagnostic => "lensed-celestial-disk-omitted.ppm",
        }
    }
}

pub fn validate_mode_surface_set(
    mode: LensedCelestialMode,
    surface_set: TraceSurfaceSet,
) -> Result<(), CelestialRenderError> {
    if mode.required_surface_set() == surface_set {
        Ok(())
    } else {
        Err(CelestialRenderError::ModeSurfaceMismatch {
            mode: mode.as_str().into(),
            surface_set: surface_set.as_str().into(),
        })
    }
}

/// Flat diagnostic disk mask (opaque mode only) — not physical emission.
pub const OPAQUE_DISK_MASK_RGB: [u8; 3] = [220, 110, 16];
pub const HORIZON_RGB: [u8; 3] = [0, 0, 0];
pub const AFFINE_LIMIT_RGB: [u8; 3] = [128, 0, 128];
pub const FAILED_RGB: [u8; 3] = [255, 0, 0];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeColorCounts {
    pub texture_sampled: u64,
    pub disk_mask: u64,
    pub horizon: u64,
    pub affine_limit: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LensedCelestialFrame {
    pub mode: LensedCelestialMode,
    pub frame: RgbFrame,
    pub texture_sample_count: u64,
    pub non_escaped_count: u64,
    pub outcome_color_counts: OutcomeColorCounts,
    pub ppm_digest: String,
}

fn non_escaped_rgb(
    mode: LensedCelestialMode,
    class: OutcomeClass,
    col: u32,
    row: u32,
) -> Result<[u8; 3], CelestialRenderError> {
    match (mode, class) {
        (LensedCelestialMode::OpaqueDiskMask, OutcomeClass::DiskHit) => Ok(OPAQUE_DISK_MASK_RGB),
        (LensedCelestialMode::DiskOmittedDiagnostic, OutcomeClass::DiskHit) => {
            Err(CelestialRenderError::UnexpectedDiskHit { col, row })
        }
        (_, OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach) => Ok(HORIZON_RGB),
        (_, OutcomeClass::AffineLimit) => Ok(AFFINE_LIMIT_RGB),
        (_, OutcomeClass::Failed) => Ok(FAILED_RGB),
        (_, OutcomeClass::Escaped) => Err(CelestialRenderError::InvalidSample(
            "escaped class in NotEscaped pixel".into(),
        )),
    }
}

pub fn render_lensed_celestial(
    coordinates: &CelestialCoordinateFrame,
    spec: &ProceduralCelestialTextureSpec,
    mode: LensedCelestialMode,
) -> Result<LensedCelestialFrame, CelestialRenderError> {
    spec.validate()?;
    let grid = coordinates.grid();
    let n = grid.pixel_count();
    let mut pixels = Vec::with_capacity(n);
    let mut texture_sample_count = 0u64;
    let mut non_escaped_count = 0u64;
    let mut counts = OutcomeColorCounts {
        texture_sampled: 0,
        disk_mask: 0,
        horizon: 0,
        affine_limit: 0,
        failed: 0,
    };

    for row in 0..grid.height {
        for col in 0..grid.width {
            let rgb = match coordinates.pixel_at(col, row) {
                CelestialCoordinatePixel::Escaped(sample) => {
                    let rgb = sample_procedural_celestial(spec, sample)?;
                    texture_sample_count += 1;
                    counts.texture_sampled += 1;
                    rgb
                }
                CelestialCoordinatePixel::NotEscaped { outcome_class } => {
                    non_escaped_count += 1;
                    let rgb = non_escaped_rgb(mode, *outcome_class, col, row)?;
                    match outcome_class {
                        OutcomeClass::DiskHit => counts.disk_mask += 1,
                        OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => {
                            counts.horizon += 1
                        }
                        OutcomeClass::AffineLimit => counts.affine_limit += 1,
                        OutcomeClass::Failed => counts.failed += 1,
                        OutcomeClass::Escaped => {}
                    }
                    rgb
                }
            };
            debug_assert_eq!(pixel_index(grid, col, row), pixels.len());
            pixels.push(rgb);
        }
    }

    let frame =
        RgbFrame::try_new(grid, pixels).map_err(|_| CelestialRenderError::FrameLengthMismatch)?;
    let ppm_digest = hex_sha(&encode_ppm(&frame));
    Ok(LensedCelestialFrame {
        mode,
        frame,
        texture_sample_count,
        non_escaped_count,
        outcome_color_counts: counts,
        ppm_digest,
    })
}

/// Recompute every pixel and require exact RGB equality with `frame`.
pub fn verify_lensed_celestial_frame(
    coordinates: &CelestialCoordinateFrame,
    spec: &ProceduralCelestialTextureSpec,
    mode: LensedCelestialMode,
    frame: &RgbFrame,
) -> Result<(), CelestialRenderError> {
    if frame.grid() != coordinates.grid() {
        return Err(CelestialRenderError::FrameLengthMismatch);
    }
    let grid = coordinates.grid();
    for row in 0..grid.height {
        for col in 0..grid.width {
            let expected = match coordinates.pixel_at(col, row) {
                CelestialCoordinatePixel::Escaped(sample) => {
                    sample_procedural_celestial(spec, sample)?
                }
                CelestialCoordinatePixel::NotEscaped { outcome_class } => {
                    non_escaped_rgb(mode, *outcome_class, col, row)?
                }
            };
            if frame.pixel_at(col, row) != expected {
                return Err(CelestialRenderError::InvalidSample(format!(
                    "pixel RGB mismatch at ({col},{row})"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::procedural_coordinate_grid_v1;
    use relativity_core::SphericalKsAzimuthStatus;
    use relativity_trace::{
        CelestialBoundarySample, CelestialCoordinatePixel, CelestialDirectionSource, CelestialUv,
        TraceGrid,
    };

    fn escaped(u: f64, v: f64) -> CelestialCoordinatePixel {
        let theta = std::f64::consts::PI * v;
        let psi = std::f64::consts::TAU * u;
        CelestialCoordinatePixel::Escaped(CelestialBoundarySample {
            source: CelestialDirectionSource::FiniteOblateEscapeBoundaryPosition,
            oblate_radius: 80.0,
            theta,
            psi,
            unit_coordinate_direction: [
                theta.sin() * psi.cos(),
                theta.sin() * psi.sin(),
                theta.cos(),
            ],
            uv: CelestialUv { u, v },
            azimuth_status: SphericalKsAzimuthStatus::Defined,
            escape_event_value: 0.0,
        })
    }

    #[test]
    fn opaque_disk_mask_color() {
        let frame = CelestialCoordinateFrame::try_new(
            TraceGrid {
                width: 1,
                height: 1,
            },
            vec![CelestialCoordinatePixel::NotEscaped {
                outcome_class: OutcomeClass::DiskHit,
            }],
        )
        .unwrap();
        let out = render_lensed_celestial(
            &frame,
            &procedural_coordinate_grid_v1(),
            LensedCelestialMode::OpaqueDiskMask,
        )
        .unwrap();
        assert_eq!(out.frame.pixel_at(0, 0), OPAQUE_DISK_MASK_RGB);
    }

    #[test]
    fn disk_omitted_rejects_disk_hit() {
        let frame = CelestialCoordinateFrame::try_new(
            TraceGrid {
                width: 1,
                height: 1,
            },
            vec![CelestialCoordinatePixel::NotEscaped {
                outcome_class: OutcomeClass::DiskHit,
            }],
        )
        .unwrap();
        assert!(matches!(
            render_lensed_celestial(
                &frame,
                &procedural_coordinate_grid_v1(),
                LensedCelestialMode::DiskOmittedDiagnostic
            ),
            Err(CelestialRenderError::UnexpectedDiskHit { .. })
        ));
    }

    #[test]
    fn horizon_black_affine_failure_explicit() {
        let frame = CelestialCoordinateFrame::try_new(
            TraceGrid {
                width: 3,
                height: 1,
            },
            vec![
                CelestialCoordinatePixel::NotEscaped {
                    outcome_class: OutcomeClass::HorizonEvent,
                },
                CelestialCoordinatePixel::NotEscaped {
                    outcome_class: OutcomeClass::AffineLimit,
                },
                CelestialCoordinatePixel::NotEscaped {
                    outcome_class: OutcomeClass::Failed,
                },
            ],
        )
        .unwrap();
        let out = render_lensed_celestial(
            &frame,
            &procedural_coordinate_grid_v1(),
            LensedCelestialMode::DiskOmittedDiagnostic,
        )
        .unwrap();
        assert_eq!(out.frame.pixel_at(0, 0), HORIZON_RGB);
        assert_eq!(out.frame.pixel_at(1, 0), AFFINE_LIMIT_RGB);
        assert_eq!(out.frame.pixel_at(2, 0), FAILED_RGB);
    }

    #[test]
    fn escaped_sampled_once() {
        let frame = CelestialCoordinateFrame::try_new(
            TraceGrid {
                width: 2,
                height: 1,
            },
            vec![
                escaped(0.1, 0.4),
                CelestialCoordinatePixel::NotEscaped {
                    outcome_class: OutcomeClass::HorizonEvent,
                },
            ],
        )
        .unwrap();
        let out = render_lensed_celestial(
            &frame,
            &procedural_coordinate_grid_v1(),
            LensedCelestialMode::DiskOmittedDiagnostic,
        )
        .unwrap();
        assert_eq!(out.texture_sample_count, 1);
        assert_eq!(out.non_escaped_count, 1);
    }

    #[test]
    fn mode_surface_mismatch_rejected() {
        assert!(validate_mode_surface_set(
            LensedCelestialMode::OpaqueDiskMask,
            TraceSurfaceSet::HorizonEscapeOnly
        )
        .is_err());
        assert!(validate_mode_surface_set(
            LensedCelestialMode::DiskOmittedDiagnostic,
            TraceSurfaceSet::OpaqueDiskHorizonEscape
        )
        .is_err());
    }
}
