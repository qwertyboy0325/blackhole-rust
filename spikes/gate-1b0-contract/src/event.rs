//! Candidate-neutral event bracket and root localization on a dense interpolant.

use crate::schema::RootLocalizationEvidence;

/// Interpolate state at parameter `theta` in `[0,1]` on the current accepted step `[t0,t1]`.
pub type StepInterpolant = dyn Fn(f64) -> Vec<f64>;

/// Event function on state (first component used if scalar event).
pub type EventFn = dyn Fn(f64, &[f64]) -> f64;

/// Localize an event using bisection on the interpolant.
/// Returns root-localization evidence only — no solver stop/restart claims.
#[allow(clippy::too_many_arguments)]
pub fn localize_root(
    t0: f64,
    t1: f64,
    y0: &[f64],
    y1: &[f64],
    event: &EventFn,
    interpolant: Option<&StepInterpolant>,
    event_time_analytic: f64,
    analytic_at_event: &[f64],
    shallow: bool,
) -> RootLocalizationEvidence {
    let f0 = event(t0, y0);
    let f1 = event(t1, y1);

    let eval = |t: f64, y: &[f64]| event(t, y);

    let mut interp_calls = 0u32;
    let mut lo_t = t0;
    let mut hi_t = t1;
    let mut lo_y = y0.to_vec();
    let mut hi_y = y1.to_vec();
    let mut lo_f = f0;
    let mut hi_f = f1;

    if lo_f.signum() == hi_f.signum() {
        if let Some(interp) = interpolant {
            for k in 1..=16 {
                let theta = k as f64 / 16.0;
                let y_mid = interp(theta);
                interp_calls += 1;
                let t_mid = t0 + theta * (t1 - t0);
                let f_mid = eval(t_mid, &y_mid);
                if lo_f.signum() != f_mid.signum() {
                    hi_t = t_mid;
                    hi_y = y_mid;
                    hi_f = f_mid;
                    break;
                }
                lo_t = t_mid;
                lo_y = y_mid;
                lo_f = f_mid;
            }
        }
    }

    let mut root_t = 0.5 * (lo_t + hi_t);
    for _ in 0..64 {
        root_t = 0.5 * (lo_t + hi_t);
        let theta = if (t1 - t0).abs() > 0.0 {
            (root_t - t0) / (t1 - t0)
        } else {
            0.5
        };
        let y_root = if let Some(interp) = interpolant {
            interp_calls += 1;
            interp(theta.clamp(0.0, 1.0))
        } else {
            lo_y.iter()
                .zip(hi_y.iter())
                .map(|(a, b)| a + theta * (b - a))
                .collect()
        };
        let f_root = eval(root_t, &y_root);
        if lo_f.signum() != f_root.signum() {
            hi_t = root_t;
            hi_y = y_root;
            hi_f = f_root;
        } else {
            lo_t = root_t;
            lo_y = y_root;
            lo_f = f_root;
        }
        if (hi_t - lo_t).abs() < 1e-12 {
            break;
        }
    }

    let theta_final = if (t1 - t0).abs() > 0.0 {
        ((root_t - t0) / (t1 - t0)).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let y_event = if let Some(interp) = interpolant {
        interp_calls += 1;
        interp(theta_final)
    } else {
        lo_y.iter()
            .zip(hi_y.iter())
            .map(|(a, b)| a + theta_final * (b - a))
            .collect()
    };

    let root_residual = eval(root_t, &y_event).abs();
    let time_error = (root_t - event_time_analytic).abs();
    let state_error = y_event
        .iter()
        .zip(analytic_at_event.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);

    RootLocalizationEvidence {
        event_time_analytic,
        event_time_found: root_t,
        time_error,
        root_residual,
        state_error,
        interpolation_calls: interp_calls,
        localized_state: y_event,
        shallow_crossing_tested: shallow,
        shallow_sign_change_only_insufficient: shallow && lo_f.signum() == hi_f.signum(),
    }
}
