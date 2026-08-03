//! Deterministic safeguarded bisection on an accepted-step interpolant.

use crate::error::IntegrationError;
use crate::state::{AffineParameter, GeodesicState};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EventLocalizationStats {
    pub interpolation_calls: u64,
    pub final_bracket_width: f64,
    pub iterations: u64,
}

/// Interpolate state at absolute affine `lambda` within `[lam0, lam1]`.
pub type StepInterpFn<'a> = dyn Fn(f64) -> Result<GeodesicState, IntegrationError> + 'a;

/// Localize a sign-changing root of `event_value(lambda, state)` on `[lam0, lam1]`.
#[allow(clippy::too_many_arguments)]
pub fn localize_sign_change(
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
    if !(lam0.is_finite() && lam1.is_finite()) || (lam1 - lam0).abs() <= 0.0 {
        return Err(IntegrationError::InvalidInterpolantBounds);
    }
    if f0.signum() == f1.signum() && f0 != 0.0 && f1 != 0.0 {
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

    let mut root_lam = 0.5 * (lo + hi);
    let mut root_y = y_lo;
    let mut root_f = flo;

    for _ in 0..80 {
        iters += 1;
        root_lam = 0.5 * (lo + hi);
        if !(root_lam >= lam0.min(lam1) && root_lam <= lam0.max(lam1)) {
            return Err(IntegrationError::InvalidInterpolantBounds);
        }
        root_y = interp(root_lam)?;
        calls += 1;
        root_f = event_value(AffineParameter(root_lam), &root_y)?;
        if !root_f.is_finite() {
            return Err(IntegrationError::NonFiniteState {
                stage: crate::error::IntegrationStage::Localization,
            });
        }

        if root_f.abs() <= value_tol || (hi - lo).abs() <= time_tol {
            break;
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

    Ok((
        AffineParameter(root_lam),
        root_y,
        root_f,
        EventLocalizationStats {
            interpolation_calls: calls,
            final_bracket_width: (hi - lo).abs(),
            iterations: iters,
        },
    ))
}
