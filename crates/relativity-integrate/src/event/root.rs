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

/// Maximum safeguarded-bisection iterations before typed non-convergence.
pub const MAX_LOCALIZATION_ITERS: u64 = 80;

/// Localize a sign-changing root of `event_value(lambda, state)` on `[lam0, lam1]`.
///
/// On success, returned `(lambda, state, residual)` are mutually consistent:
/// `residual == event_value(lambda, state)` from the last sample at `lambda`.
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
    // Last successfully sampled (lambda, state, f) — always mutually consistent.
    let mut sampled_lam = lam0;
    let mut sampled_y = *y0;
    let mut sampled_f = f0;
    let mut prev_mid = f64::NAN;

    #[allow(clippy::explicit_counter_loop)]
    for _ in 0..MAX_LOCALIZATION_ITERS {
        iters += 1;
        let mid = 0.5 * (lo + hi);
        if !(mid >= lam_min && mid <= lam_max) {
            return Err(IntegrationError::InvalidInterpolantBounds);
        }
        // Floating-point midpoint stagnation: lo/hi no longer separable.
        if mid == prev_mid || mid == lo || mid == hi {
            let width = (hi - lo).abs();
            // Return last consistent sample — never a different lambda with stale f/state.
            if sampled_f.abs() <= value_tol {
                return Ok((
                    AffineParameter(sampled_lam),
                    sampled_y,
                    sampled_f,
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
                    AffineParameter(sampled_lam),
                    sampled_y,
                    sampled_f,
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
                residual: sampled_f.abs(),
                bracket_width: width,
            });
        }
        prev_mid = mid;

        let y_mid = interp(mid)?;
        calls += 1;
        let f_mid = event_value(AffineParameter(mid), &y_mid)?;
        if !f_mid.is_finite() {
            return Err(IntegrationError::NonFiniteState {
                stage: crate::error::IntegrationStage::Localization,
            });
        }
        sampled_lam = mid;
        sampled_y = y_mid;
        sampled_f = f_mid;

        let width = (hi - lo).abs();
        if sampled_f.abs() <= value_tol {
            return Ok((
                AffineParameter(sampled_lam),
                sampled_y,
                sampled_f,
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
                AffineParameter(sampled_lam),
                sampled_y,
                sampled_f,
                EventLocalizationStats {
                    interpolation_calls: calls,
                    final_bracket_width: width,
                    iterations: iters,
                    termination: LocalizationTermination::AffineWidthTolerance,
                },
            ));
        }

        if flo.signum() != sampled_f.signum() || (flo == 0.0) != (sampled_f == 0.0) {
            hi = mid;
            fhi = sampled_f;
            y_hi = sampled_y;
        } else if fhi.signum() != sampled_f.signum() || (fhi == 0.0) != (sampled_f == 0.0) {
            lo = mid;
            flo = sampled_f;
            y_lo = sampled_y;
        } else {
            return Err(IntegrationError::InvalidInterpolantBounds);
        }
        let _ = (y_lo, y_hi);
    }

    Err(IntegrationError::EventLocalizationDidNotConverge {
        event_id,
        iterations: iters,
        residual: sampled_f.abs(),
        bracket_width: (hi - lo).abs(),
    })
}

/// Structured evidence that stagnation and exhaustion paths are executable.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocalizationNonconvergenceEvidence {
    pub stagnation_event_id: EventId,
    pub stagnation_iterations: u64,
    pub stagnation_residual: f64,
    pub stagnation_bracket_width: f64,
    pub exhaustion_event_id: EventId,
    pub exhaustion_iterations: u64,
    pub exhaustion_residual: f64,
    pub exhaustion_bracket_width: f64,
}

/// Deterministic project-owned self-check for Gate 1B1 evaluator.
///
/// Fails if midpoint-stagnation or iteration-exhaustion non-convergence paths
/// do not return the typed error with the documented invariants.
pub fn localization_nonconvergence_self_check() -> Result<LocalizationNonconvergenceEvidence, String>
{
    use std::cell::Cell;

    use relativity_core::{Covector, PositionKs};

    let state_at = |x: f64| {
        GeodesicState::new(
            PositionKs::new(0.0, x, 0.0, 0.0),
            Covector::new(1.0, 1.0, 0.0, 0.0),
        )
        .unwrap()
    };
    let piecewise =
        |_l: AffineParameter, st: &GeodesicState| Ok(if st.position.x < 0.0 { -1.0 } else { 1.0 });

    // Midpoint stagnation: consecutive f64 at large magnitude → mid collapses.
    let lo: f64 = 1.0e16;
    let hi = f64::from_bits(lo.to_bits() + 1);
    let mid = 0.5 * (lo + hi);
    if mid != lo && mid != hi {
        return Err(format!(
            "stagnation fixture invalid: mid={mid} lo={lo} hi={hi}"
        ));
    }
    let outside = Cell::new(0u64);
    let y0 = state_at(-1.0);
    let y1 = state_at(1.0);
    let interp_stag = |lam: f64| {
        if lam < lo || lam > hi {
            outside.set(outside.get() + 1);
        }
        let t = (lam - lo) / (hi - lo).max(f64::MIN_POSITIVE);
        Ok(state_at(2.0 * t - 1.0))
    };
    let stag_err = localize_sign_change(
        EventId::OuterHorizon,
        lo,
        hi,
        &y0,
        &y1,
        -1.0,
        1.0,
        &interp_stag,
        &piecewise,
        1e-30,
        1e-30,
    )
    .err()
    .ok_or_else(|| "stagnation expected EventLocalizationDidNotConverge".to_string())?;
    let IntegrationError::EventLocalizationDidNotConverge {
        event_id: stag_id,
        iterations: stag_iters,
        residual: stag_res,
        bracket_width: stag_w,
    } = stag_err
    else {
        return Err(format!("stagnation wrong error: {stag_err}"));
    };
    if stag_id != EventId::OuterHorizon {
        return Err(format!("stagnation event_id {stag_id:?}"));
    }
    if !(stag_res.is_finite() && stag_res > 0.0) {
        return Err(format!("stagnation residual {stag_res}"));
    }
    if !(stag_w.is_finite() && stag_w > 0.0) {
        return Err(format!("stagnation width {stag_w}"));
    }
    if stag_iters == 0 {
        return Err("stagnation iterations == 0".into());
    }
    if outside.get() != 0 {
        return Err(format!(
            "stagnation interpolated outside bounds ({})",
            outside.get()
        ));
    }

    // Iteration exhaustion: root pinned at λ=0 so hi shrinks while lo stays 0.
    // After 80 steps width ≈ 2^-80 > 1e-30; |f|=1 never meets value_tol;
    // midpoint does not stagnate until ~10^3 iterations.
    let outside_ex = Cell::new(0u64);
    let interp_ex = |lam: f64| {
        if !(0.0..=1.0).contains(&lam) {
            outside_ex.set(outside_ex.get() + 1);
        }
        Ok(state_at(lam))
    };
    let root_at_zero =
        |l: AffineParameter, _st: &GeodesicState| Ok(if l.0 <= 0.0 { -1.0 } else { 1.0 });
    let ex_err = localize_sign_change(
        EventId::EscapeSphere,
        0.0,
        1.0,
        &y0,
        &y1,
        -1.0,
        1.0,
        &interp_ex,
        &root_at_zero,
        1e-30,
        1e-30,
    )
    .err()
    .ok_or_else(|| "exhaustion expected EventLocalizationDidNotConverge".to_string())?;
    let IntegrationError::EventLocalizationDidNotConverge {
        event_id: ex_id,
        iterations: ex_iters,
        residual: ex_res,
        bracket_width: ex_w,
    } = ex_err
    else {
        return Err(format!("exhaustion wrong error: {ex_err}"));
    };
    if ex_id != EventId::EscapeSphere {
        return Err(format!("exhaustion event_id {ex_id:?}"));
    }
    if ex_iters != MAX_LOCALIZATION_ITERS {
        return Err(format!(
            "exhaustion iterations {ex_iters} != {MAX_LOCALIZATION_ITERS}"
        ));
    }
    if !(ex_res.is_finite() && ex_res > 1e-30) {
        return Err(format!("exhaustion residual {ex_res}"));
    }
    if !(ex_w.is_finite() && ex_w > 1e-30) {
        return Err(format!("exhaustion width {ex_w}"));
    }
    if outside_ex.get() != 0 {
        return Err(format!(
            "exhaustion interpolated outside bounds ({})",
            outside_ex.get()
        ));
    }

    Ok(LocalizationNonconvergenceEvidence {
        stagnation_event_id: stag_id,
        stagnation_iterations: stag_iters,
        stagnation_residual: stag_res,
        stagnation_bracket_width: stag_w,
        exhaustion_event_id: ex_id,
        exhaustion_iterations: ex_iters,
        exhaustion_residual: ex_res,
        exhaustion_bracket_width: ex_w,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use relativity_core::{Covector, PositionKs};
    use std::cell::Cell;

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
        let (lam, st, f, stats) = localize_sign_change(
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
        assert!((0.0..=1.0).contains(&lam.0));
        assert!((f - st.position.x).abs() < 1e-15);
        assert!((st.position.x - (2.0 * lam.0 - 1.0)).abs() < 1e-12);
    }

    #[test]
    fn affine_width_termination() {
        let y0 = state_at_x(-1.0);
        let y1 = state_at_x(1.0);
        let interp = |lam: f64| Ok(state_at_x(2.0 * lam - 1.0));
        let ev = |_l: AffineParameter, st: &GeodesicState| {
            Ok(if st.position.x < 0.0 { -1.0 } else { 1.0 })
        };
        let (lam, st, f, stats) = localize_sign_change(
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
        let expected_f = if st.position.x < 0.0 { -1.0 } else { 1.0 };
        assert_eq!(f, expected_f);
        assert!((st.position.x - (2.0 * lam.0 - 1.0)).abs() < 1e-12);
    }

    #[test]
    fn exact_endpoint() {
        let y0 = state_at_x(1.0);
        let y1 = state_at_x(0.0);
        let interp = |_lam: f64| Ok(y1);
        let ev = |_l: AffineParameter, st: &GeodesicState| Ok(st.position.x);
        let (_, st, f, stats) = localize_sign_change(
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
        assert_eq!(st.position.x, 0.0);
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

    #[test]
    fn midpoint_stagnation_nonconvergence() {
        let lo: f64 = 1.0e16;
        let hi = f64::from_bits(lo.to_bits() + 1);
        let mid = 0.5 * (lo + hi);
        assert!(mid == lo || mid == hi, "fixture must stagnate");

        let y0 = state_at_x(-1.0);
        let y1 = state_at_x(1.0);
        let outside = Cell::new(0u64);
        let interp = |lam: f64| {
            if lam < lo || lam > hi {
                outside.set(outside.get() + 1);
            }
            let t = (lam - lo) / (hi - lo).max(f64::MIN_POSITIVE);
            Ok(state_at_x(2.0 * t - 1.0))
        };
        let ev = |_l: AffineParameter, st: &GeodesicState| {
            Ok(if st.position.x < 0.0 { -1.0 } else { 1.0 })
        };
        let err = localize_sign_change(
            EventId::OuterHorizon,
            lo,
            hi,
            &y0,
            &y1,
            -1.0,
            1.0,
            &interp,
            &ev,
            1e-30,
            1e-30,
        )
        .unwrap_err();
        match err {
            IntegrationError::EventLocalizationDidNotConverge {
                event_id,
                iterations,
                residual,
                bracket_width,
            } => {
                assert_eq!(event_id, EventId::OuterHorizon);
                assert!(iterations > 0);
                assert!(residual.is_finite() && residual > 0.0);
                assert!(bracket_width.is_finite() && bracket_width > 0.0);
                assert_eq!(iterations, 1);
            }
            other => panic!("unexpected {other}"),
        }
        assert_eq!(outside.get(), 0);
    }

    #[test]
    fn iteration_exhaustion_nonconvergence() {
        let y0 = state_at_x(0.0);
        let y1 = state_at_x(1.0);
        let outside = Cell::new(0u64);
        let interp = |lam: f64| {
            if !(0.0..=1.0).contains(&lam) {
                outside.set(outside.get() + 1);
            }
            Ok(state_at_x(lam))
        };
        // Root at λ=0: every mid > 0 keeps lo fixed and shrinks hi.
        let ev = |l: AffineParameter, _st: &GeodesicState| Ok(if l.0 <= 0.0 { -1.0 } else { 1.0 });
        let err = localize_sign_change(
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
            1e-30,
        )
        .unwrap_err();
        match err {
            IntegrationError::EventLocalizationDidNotConverge {
                event_id,
                iterations,
                residual,
                bracket_width,
            } => {
                assert_eq!(event_id, EventId::EscapeSphere);
                assert_eq!(iterations, MAX_LOCALIZATION_ITERS);
                assert!(residual.is_finite() && residual > 1e-30);
                assert!(bracket_width.is_finite() && bracket_width > 1e-30);
            }
            other => panic!("unexpected {other}"),
        }
        assert_eq!(outside.get(), 0);
    }

    #[test]
    fn stagnation_success_keeps_lambda_state_residual_consistent() {
        let lo: f64 = 1.0e16;
        let hi = f64::from_bits(lo.to_bits() + 1);
        assert!(0.5 * (lo + hi) == lo || 0.5 * (lo + hi) == hi);

        let y0 = state_at_x(-1.0);
        let y1 = state_at_x(1.0);
        let interp = |lam: f64| {
            let t = (lam - lo) / (hi - lo).max(f64::MIN_POSITIVE);
            Ok(state_at_x(2.0 * t - 1.0))
        };
        let ev = |_l: AffineParameter, st: &GeodesicState| {
            Ok(if st.position.x < 0.0 { -1.0 } else { 1.0 })
        };
        // value_tol covers |f0|=1 → stagnation success via EventValueTolerance.
        let (lam, st, f, stats) = localize_sign_change(
            EventId::OuterHorizon,
            lo,
            hi,
            &y0,
            &y1,
            -1.0,
            1.0,
            &interp,
            &ev,
            1e-30,
            1.0,
        )
        .unwrap();
        assert_eq!(
            stats.termination,
            LocalizationTermination::EventValueTolerance
        );
        assert_eq!(lam.0, lo);
        assert_eq!(st.position.x, y0.position.x);
        assert_eq!(f, -1.0);
        let recomputed = ev(lam, &st).unwrap();
        assert_eq!(f, recomputed);
    }

    #[test]
    fn self_check_matches_unit_tests() {
        let ev = localization_nonconvergence_self_check().unwrap();
        assert_eq!(ev.stagnation_event_id, EventId::OuterHorizon);
        assert_eq!(ev.exhaustion_iterations, MAX_LOCALIZATION_ITERS);
        assert_eq!(ev.exhaustion_event_id, EventId::EscapeSphere);
    }
}
