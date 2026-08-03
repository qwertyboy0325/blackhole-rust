//! Deterministic safeguarded bisection on an accepted-step interpolant.

use crate::error::IntegrationError;
use crate::event::EventId;
use crate::state::{AffineParameter, GeodesicState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalizationTermination {
    ExactEndpoint,
    EventValueTolerance,
    AffineWidthTolerance,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EventLocalizationStats {
    pub interpolation_calls: u64,
    pub final_bracket_width: f64,
    pub iterations: u64,
    pub termination: LocalizationTermination,
}

/// Interpolate state at absolute affine `lambda` within `[lam0, lam1]`.
pub type StepInterpFn<'a> = dyn Fn(f64) -> Result<GeodesicState, IntegrationError> + 'a;

const MAX_LOCALIZATION_ITERS: u64 = 80;

/// Localize a sign-changing root of `event_value(lambda, state)` on `[lam0, lam1]`.
#[allow(clippy::too_many_arguments)]
pub fn localize_sign_change(
    event_id: EventId,
    lam0: f64,
    lam1: f64,
    y0: &GeodesicState,
    y1: &GeodesicState,
    f0: f64,
    f1: f64,
    interp: &StepInterpFn<'_>,
    event_value: &dyn Fn(AffineParameter, &GeodesicState) -> Result<f64, IntegrationError>,
    time_tol: f64,
    value_tol: f64,
) -> Result<(AffineParameter, GeodesicState, f64, EventLocalizationStats), IntegrationError> {
    let lam_min = lam0.min(lam1);
    let lam_max = lam0.max(lam1);
    if !(lam0.is_finite() && lam1.is_finite()) || (lam1 - lam0).abs() <= 0.0 {
        return Err(IntegrationError::InvalidInterpolantBounds);
    }

    // Exact endpoint root — no bisection required.
    if f1 == 0.0 {
        return Ok((
            AffineParameter(lam1),
            *y1,
            f1,
            EventLocalizationStats {
                interpolation_calls: 0,
                final_bracket_width: 0.0,
                iterations: 0,
                termination: LocalizationTermination::ExactEndpoint,
            },
        ));
    }
    if f0 == 0.0 {
        return Ok((
            AffineParameter(lam0),
            *y0,
            f0,
            EventLocalizationStats {
                interpolation_calls: 0,
                final_bracket_width: 0.0,
                iterations: 0,
                termination: LocalizationTermination::ExactEndpoint,
            },
        ));
    }
    if f0.signum() == f1.signum() {
        return Err(IntegrationError::InvalidInterpolantBounds);
    }

    let mut lo = lam0;
    let mut hi = lam1;
    let mut flo = f0;
    let mut fhi = f1;
    let mut y_lo = *y0;
    let mut y_hi = *y1;
    let mut calls = 0u64;
    let mut iters = 0u64;
    let mut root_lam;
    let mut root_y = y_lo;
    let mut root_f = flo;
    let mut prev_mid = f64::NAN;

    #[allow(clippy::explicit_counter_loop)]
    for _ in 0..MAX_LOCALIZATION_ITERS {
        iters += 1;
        root_lam = 0.5 * (lo + hi);
        if !(root_lam >= lam_min && root_lam <= lam_max) {
            return Err(IntegrationError::InvalidInterpolantBounds);
        }
        // Floating-point midpoint stagnation: lo/hi no longer separable.
        if root_lam == prev_mid || root_lam == lo || root_lam == hi {
            let width = (hi - lo).abs();
            if root_f.abs() <= value_tol {
                return Ok((
                    AffineParameter(root_lam),
                    root_y,
                    root_f,
                    EventLocalizationStats {
                        interpolation_calls: calls,
                        final_bracket_width: width,
                        iterations: iters,
                        termination: LocalizationTermination::EventValueTolerance,
                    },
                ));
            }
            if width <= time_tol {
                return Ok((
                    AffineParameter(root_lam),
                    root_y,
                    root_f,
                    EventLocalizationStats {
                        interpolation_calls: calls,
                        final_bracket_width: width,
                        iterations: iters,
                        termination: LocalizationTermination::AffineWidthTolerance,
                    },
                ));
            }
            return Err(IntegrationError::EventLocalizationDidNotConverge {
                event_id,
                iterations: iters,
                residual: root_f.abs(),
                bracket_width: width,
            });
        }
        prev_mid = root_lam;

        root_y = interp(root_lam)?;
        calls += 1;
        root_f = event_value(AffineParameter(root_lam), &root_y)?;
        if !root_f.is_finite() {
            return Err(IntegrationError::NonFiniteState {
                stage: crate::error::IntegrationStage::Localization,
            });
        }

        let width = (hi - lo).abs();
        if root_f.abs() <= value_tol {
            return Ok((
                AffineParameter(root_lam),
                root_y,
                root_f,
                EventLocalizationStats {
                    interpolation_calls: calls,
                    final_bracket_width: width,
                    iterations: iters,
                    termination: LocalizationTermination::EventValueTolerance,
                },
            ));
        }
        if width <= time_tol {
            return Ok((
                AffineParameter(root_lam),
                root_y,
                root_f,
                EventLocalizationStats {
                    interpolation_calls: calls,
                    final_bracket_width: width,
                    iterations: iters,
                    termination: LocalizationTermination::AffineWidthTolerance,
                },
            ));
        }

        if flo.signum() != root_f.signum() || (flo == 0.0) != (root_f == 0.0) {
            hi = root_lam;
            fhi = root_f;
            y_hi = root_y;
        } else if fhi.signum() != root_f.signum() || (fhi == 0.0) != (root_f == 0.0) {
            lo = root_lam;
            flo = root_f;
            y_lo = root_y;
        } else {
            return Err(IntegrationError::InvalidInterpolantBounds);
        }
        let _ = (y_lo, y_hi);
    }

    Err(IntegrationError::EventLocalizationDidNotConverge {
        event_id,
        iterations: iters,
        residual: root_f.abs(),
        bracket_width: (hi - lo).abs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_core::{Covector, PositionKs};

    fn state_at_x(x: f64) -> GeodesicState {
        GeodesicState::new(
            PositionKs::new(0.0, x, 0.0, 0.0),
            Covector::new(1.0, 1.0, 0.0, 0.0),
        )
        .unwrap()
    }

    #[test]
    fn value_tolerance_termination() {
        let y0 = state_at_x(-1.0);
        let y1 = state_at_x(1.0);
        let interp = |lam: f64| Ok(state_at_x(2.0 * lam - 1.0));
        let ev = |_l: AffineParameter, st: &GeodesicState| Ok(st.position.x);
        let (lam, _, f, stats) = localize_sign_change(
            EventId::EscapeSphere,
            0.0,
            1.0,
            &y0,
            &y1,
            -1.0,
            1.0,
            &interp,
            &ev,
            1e-30,
            1e-3,
        )
        .unwrap();
        assert!(f.abs() <= 1e-3);
        assert_eq!(
            stats.termination,
            LocalizationTermination::EventValueTolerance
        );
        assert!(lam.0 >= 0.0 && lam.0 <= 1.0);
    }

    #[test]
    fn affine_width_termination() {
        let y0 = state_at_x(-1.0);
        let y1 = state_at_x(1.0);
        let interp = |lam: f64| Ok(state_at_x(2.0 * lam - 1.0));
        // Piecewise ±1 so |f| never meets value tol; affine-width criterion wins.
        let ev = |_l: AffineParameter, st: &GeodesicState| {
            Ok(if st.position.x < 0.0 { -1.0 } else { 1.0 })
        };
        let (_, _, _, stats) = localize_sign_change(
            EventId::EscapeSphere,
            0.0,
            1.0,
            &y0,
            &y1,
            -1.0,
            1.0,
            &interp,
            &ev,
            1e-4,
            1e-30,
        )
        .unwrap();
        assert_eq!(
            stats.termination,
            LocalizationTermination::AffineWidthTolerance
        );
        assert!(stats.final_bracket_width <= 1e-4 + 1e-15);
    }

    #[test]
    fn exact_endpoint() {
        let y0 = state_at_x(1.0);
        let y1 = state_at_x(0.0);
        let interp = |_lam: f64| Ok(y1);
        let ev = |_l: AffineParameter, st: &GeodesicState| Ok(st.position.x);
        let (_, _, f, stats) = localize_sign_change(
            EventId::OuterHorizon,
            0.0,
            1.0,
            &y0,
            &y1,
            1.0,
            0.0,
            &interp,
            &ev,
            1e-12,
            1e-12,
        )
        .unwrap();
        assert_eq!(f, 0.0);
        assert_eq!(stats.termination, LocalizationTermination::ExactEndpoint);
    }

    #[test]
    fn lost_bracket_errors() {
        let y0 = state_at_x(1.0);
        let y1 = state_at_x(2.0);
        let interp = |lam: f64| Ok(state_at_x(lam));
        let ev = |_l: AffineParameter, st: &GeodesicState| Ok(st.position.x);
        let err = localize_sign_change(
            EventId::EscapeSphere,
            0.0,
            1.0,
            &y0,
            &y1,
            1.0,
            2.0,
            &interp,
            &ev,
            1e-12,
            1e-12,
        )
        .unwrap_err();
        assert!(matches!(err, IntegrationError::InvalidInterpolantBounds));
    }

    #[test]
    fn result_stays_in_bounds() {
        let y0 = state_at_x(-2.0);
        let y1 = state_at_x(2.0);
        let interp = |lam: f64| {
            assert!((0.0..=1.0).contains(&lam));
            Ok(state_at_x(4.0 * lam - 2.0))
        };
        let ev = |_l: AffineParameter, st: &GeodesicState| Ok(st.position.x);
        let (lam, _, _, _) = localize_sign_change(
            EventId::EscapeSphere,
            0.0,
            1.0,
            &y0,
            &y1,
            -2.0,
            2.0,
            &interp,
            &ev,
            1e-12,
            1e-12,
        )
        .unwrap();
        assert!((0.0..=1.0).contains(&lam.0));
    }
}
