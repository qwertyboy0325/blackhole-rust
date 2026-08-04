//! Pure diagnostic shading: `TraceBundle` → `RgbFrame` (no tracing, no radiometry).
//!
//! `TraceBundle` contains traced physical/numerical outcomes.
//! It does not contain display colors.
//! It may be shaded repeatedly without retracing.
//!
//! `DiskSuppressed` is a debug projection only — not a black-hole shadow and not
//! a physically emitted disk image.

use crate::camera::{pixel_index, TraceGrid};
use crate::diagnostics::{hex_sha, PixelCoord};
use crate::image::encode_ppm;
use crate::outcome::{OutcomeClass, RayOutcome};
use crate::trace::TraceBundle;
use serde::{Deserialize, Serialize};

/// Validated RGB frame: `pixels.len() == grid.pixel_count()`, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbFrame {
    grid: TraceGrid,
    pixels: Vec<[u8; 3]>,
}

impl RgbFrame {
    /// Construct a frame; rejects length mismatches.
    pub fn try_new(grid: TraceGrid, pixels: Vec<[u8; 3]>) -> Result<Self, &'static str> {
        if pixels.len() != grid.pixel_count() {
            return Err("RgbFrame pixel count must equal grid.pixel_count()");
        }
        Ok(Self { grid, pixels })
    }

    pub fn grid(&self) -> TraceGrid {
        self.grid
    }

    pub fn pixels(&self) -> &[[u8; 3]] {
        &self.pixels
    }

    pub fn pixel_at(&self, col: u32, row: u32) -> [u8; 3] {
        self.pixels[pixel_index(self.grid, col, row)]
    }
}

/// Project-owned diagnostic shade styles (not physical appearance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticShadeStyle {
    Gate1b2Categorical,
    DiskSuppressed,
}

impl DiagnosticShadeStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gate1b2Categorical => "gate1b2-categorical",
            Self::DiskSuppressed => "disk-suppressed",
        }
    }

    pub fn filename_stem(self) -> &'static str {
        self.as_str()
    }
}

/// Gate 1B2 categorical legend (exact).
pub fn categorical_rgb(class: OutcomeClass) -> [u8; 3] {
    match class {
        OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => [0, 0, 0],
        OutcomeClass::DiskHit => [255, 128, 0],
        OutcomeClass::Escaped => [0, 64, 255],
        OutcomeClass::AffineLimit => [128, 0, 128],
        OutcomeClass::Failed => [255, 0, 0],
    }
}

/// Debug style: disk hits suppressed to black; all other classes unchanged.
pub fn disk_suppressed_rgb(class: OutcomeClass) -> [u8; 3] {
    match class {
        OutcomeClass::DiskHit => [0, 0, 0],
        other => categorical_rgb(other),
    }
}

/// Shade one outcome under a diagnostic style.
pub fn shade_outcome(style: DiagnosticShadeStyle, outcome: &RayOutcome) -> [u8; 3] {
    match style {
        DiagnosticShadeStyle::Gate1b2Categorical => categorical_rgb(outcome.class()),
        DiagnosticShadeStyle::DiskSuppressed => disk_suppressed_rgb(outcome.class()),
    }
}

/// Generic row-major pure shader over a completed `TraceBundle`.
pub fn shade_trace_bundle<F>(bundle: &TraceBundle, mut shader: F) -> RgbFrame
where
    F: FnMut(PixelCoord, &RayOutcome) -> [u8; 3],
{
    let n = bundle.grid.pixel_count();
    let mut pixels = Vec::with_capacity(n);
    for row in 0..bundle.grid.height {
        for col in 0..bundle.grid.width {
            let coord = PixelCoord { col, row };
            let outcome = bundle.outcome_at(col, row);
            pixels.push(shader(coord, outcome));
        }
    }
    RgbFrame::try_new(bundle.grid, pixels).expect("row-major visit yields pixel_count entries")
}

/// Project-owned diagnostic shade of a traced frame.
pub fn shade_diagnostic(bundle: &TraceBundle, style: DiagnosticShadeStyle) -> RgbFrame {
    shade_trace_bundle(bundle, |_coord, outcome| shade_outcome(style, outcome))
}

/// One shaded output tied to a style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadedFrame {
    pub style: DiagnosticShadeStyle,
    pub frame: RgbFrame,
    pub ppm_digest: String,
}

/// Shade multiple styles against one `TraceBundle` (no tracing).
///
/// Preserves caller order. Duplicates are allowed and shaded independently.
pub fn shade_many(bundle: &TraceBundle, styles: &[DiagnosticShadeStyle]) -> Vec<ShadedFrame> {
    styles
        .iter()
        .copied()
        .map(|style| {
            let frame = shade_diagnostic(bundle, style);
            let ppm = encode_ppm(&frame);
            ShadedFrame {
                style,
                frame,
                ppm_digest: hex_sha(&ppm),
            }
        })
        .collect()
}

/// Count pixels that differ between two equal-sized frames.
pub fn rgb_frame_diff_count(a: &RgbFrame, b: &RgbFrame) -> Option<u64> {
    if a.grid != b.grid || a.pixels.len() != b.pixels.len() {
        return None;
    }
    let mut n = 0u64;
    for (pa, pb) in a.pixels.iter().zip(b.pixels.iter()) {
        if pa != pb {
            n += 1;
        }
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::{AffineLimitOutcome, OutcomeClass};
    use relativity_core::{Covector, PositionKs};
    use relativity_integrate::{
        AffineParameter, GeodesicState, IntegrationStats, InvariantDiagnostics,
    };

    fn dummy_outcome(class: OutcomeClass) -> RayOutcome {
        let state = GeodesicState::new(
            PositionKs::new(0.0, 10.0, 0.0, 0.0),
            Covector::new(-1.0, 0.0, 0.0, 0.0),
        )
        .unwrap();
        let integration = IntegrationStats {
            accepted_steps: 1,
            rejected_steps: 0,
            rhs_evaluations: 10,
            callback_count: 1,
        };
        let diagnostics = InvariantDiagnostics {
            h_initial: 0.0,
            h_final: 0.0,
            h_max_abs_residual: 0.0,
            p_t_initial: -1.0,
            p_t_final: -1.0,
            p_t_max_abs_drift: 0.0,
            non_finite_checks: 0,
            raw_vs_localized_lambda_separation: None,
            relative_tolerance: [1e-8; 8],
            absolute_tolerance: [1e-9; 8],
        };
        match class {
            OutcomeClass::AffineLimit => RayOutcome::AffineLimit(AffineLimitOutcome {
                lambda: AffineParameter(1.0),
                state,
                integration,
                diagnostics,
            }),
            OutcomeClass::Failed => RayOutcome::Failed(crate::outcome::RayFailure {
                error: relativity_integrate::IntegrationError::MissingEventOutcome,
            }),
            OutcomeClass::Escaped => RayOutcome::Escaped(crate::outcome::EscapeHit {
                lambda: AffineParameter(1.0),
                state,
                raw_solver_stop: relativity_integrate::RawSolverStop {
                    lambda: AffineParameter(1.0),
                    state,
                },
                integration,
                diagnostics,
                event_value: 0.0,
            }),
            OutcomeClass::DiskHit => RayOutcome::DiskHit(crate::outcome::DiskHit {
                lambda: AffineParameter(1.0),
                state,
                oblate_radius: 5.0,
                crossing_side: relativity_integrate::DiskCrossingSide::UpperToLower,
                raw_solver_stop: relativity_integrate::RawSolverStop {
                    lambda: AffineParameter(1.0),
                    state,
                },
                integration,
                diagnostics,
                event_value: 0.0,
                localization: relativity_integrate::EventLocalizationStats {
                    interpolation_calls: 1,
                    final_bracket_width: 0.0,
                    iterations: 1,
                    termination: relativity_integrate::LocalizationTermination::ExactEndpoint,
                },
            }),
            OutcomeClass::HorizonEvent | OutcomeClass::HorizonApproach => {
                // Use Failed as stand-in only for class_rgb tests via shade_outcome.
                RayOutcome::Failed(crate::outcome::RayFailure {
                    error: relativity_integrate::IntegrationError::MissingEventOutcome,
                })
            }
        }
    }

    fn tiny_bundle(classes: &[(u32, u32, OutcomeClass)]) -> TraceBundle {
        let width = classes.iter().map(|(c, _, _)| *c).max().unwrap_or(0) + 1;
        let height = classes.iter().map(|(_, r, _)| *r).max().unwrap_or(0) + 1;
        let grid = TraceGrid { width, height };
        let mut outcomes = vec![dummy_outcome(OutcomeClass::Escaped); grid.pixel_count()];
        for &(col, row, class) in classes {
            outcomes[pixel_index(grid, col, row)] = dummy_outcome(class);
        }
        TraceBundle { grid, outcomes }
    }

    #[test]
    fn shade_trace_bundle_row_major_once_per_pixel() {
        let bundle = tiny_bundle(&[(0, 0, OutcomeClass::Escaped), (1, 0, OutcomeClass::DiskHit)]);
        let mut visits = Vec::new();
        let frame = shade_trace_bundle(&bundle, |coord, _| {
            visits.push((coord.col, coord.row));
            [1, 2, 3]
        });
        assert_eq!(visits, vec![(0, 0), (1, 0)]);
        assert_eq!(frame.pixels().len(), 2);
        assert_eq!(frame.pixel_at(1, 0), [1, 2, 3]);
    }

    #[test]
    fn legacy_legend_colors() {
        assert_eq!(categorical_rgb(OutcomeClass::HorizonEvent), [0, 0, 0]);
        assert_eq!(categorical_rgb(OutcomeClass::HorizonApproach), [0, 0, 0]);
        assert_eq!(categorical_rgb(OutcomeClass::DiskHit), [255, 128, 0]);
        assert_eq!(categorical_rgb(OutcomeClass::Escaped), [0, 64, 255]);
        assert_eq!(categorical_rgb(OutcomeClass::AffineLimit), [128, 0, 128]);
        assert_eq!(categorical_rgb(OutcomeClass::Failed), [255, 0, 0]);
    }

    #[test]
    fn disk_suppressed_only_changes_disk_hit() {
        assert_eq!(disk_suppressed_rgb(OutcomeClass::DiskHit), [0, 0, 0]);
        assert_eq!(
            disk_suppressed_rgb(OutcomeClass::Escaped),
            categorical_rgb(OutcomeClass::Escaped)
        );
    }

    #[test]
    fn shade_many_preserves_order_and_is_deterministic() {
        let bundle = tiny_bundle(&[(0, 0, OutcomeClass::DiskHit)]);
        let styles = [
            DiagnosticShadeStyle::Gate1b2Categorical,
            DiagnosticShadeStyle::DiskSuppressed,
        ];
        let a = shade_many(&bundle, &styles);
        let b = shade_many(&bundle, &styles);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].style, DiagnosticShadeStyle::Gate1b2Categorical);
        assert_eq!(a[1].style, DiagnosticShadeStyle::DiskSuppressed);
        assert_eq!(a[0].ppm_digest, b[0].ppm_digest);
        assert_eq!(a[1].ppm_digest, b[1].ppm_digest);
        assert_ne!(a[0].ppm_digest, a[1].ppm_digest);
    }

    #[test]
    fn rgb_frame_rejects_length_mismatch() {
        assert!(RgbFrame::try_new(
            TraceGrid {
                width: 2,
                height: 2
            },
            vec![[0, 0, 0]; 3]
        )
        .is_err());
    }
}
