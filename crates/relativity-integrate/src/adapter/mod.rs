//! Public integration entry points. `ivp` stays private in `ivp_backend`.

mod ivp_backend;

use relativity_core::KerrParams;

use crate::config::Dop853Config;
use crate::error::{IntegrationError, IntegrationStage};
use crate::event::EventSurface;
use crate::outcome::{IntegrationOutcome, IntegrationReport, InvariantDiagnostics};
use crate::rhs::initial_hamiltonian;
use crate::state::{AffineParameter, GeodesicState};

use ivp_backend::{integrate_ivp, pending_to_event_hit};

/// Integrate a geodesic from `y0` with optional sign-changing event surfaces.
///
/// Public API is independent of `ivp` types. Caller-authoritative event results
/// use adapter-localized state; raw solver stop is retained separately.
pub fn integrate(
    params: KerrParams,
    y0: &GeodesicState,
    config: &Dop853Config,
    surfaces: &[&dyn EventSurface],
) -> Result<IntegrationReport, IntegrationError> {
    config.validate()?;
    y0.require_finite(IntegrationStage::InitialState)?;

    let h0 = initial_hamiltonian(&params, y0)?;
    let pt0 = y0.momentum.t;

    let backend = integrate_ivp(params, y0, config, surfaces)?;

    let outcome = if let Some(pending) = backend.pending {
        if backend.steps_after_interrupt != 0 {
            return Err(IntegrationError::Solver {
                detail: "accepted steps continued after event interrupt".into(),
            });
        }
        IntegrationOutcome::Event(pending_to_event_hit(pending, backend.stats))
    } else if backend.interrupted {
        return Err(IntegrationError::MissingEventOutcome);
    } else {
        IntegrationOutcome::AffineLimit {
            lambda: AffineParameter(backend.final_lambda),
            state: backend.final_state,
            stats: backend.stats,
        }
    };

    let (h_final, pt_final, raw_sep) = match &outcome {
        IntegrationOutcome::Event(hit) => {
            let h = initial_hamiltonian(&params, &hit.state)?;
            let sep = (hit.raw_solver_stop.lambda.0 - hit.lambda.0).abs();
            (h, hit.state.momentum.t, Some(sep))
        }
        IntegrationOutcome::AffineLimit { state, .. } => {
            let h = initial_hamiltonian(&params, state)?;
            (h, state.momentum.t, None)
        }
    };

    let h_max = backend
        .endpoint_h
        .iter()
        .map(|h| h.abs())
        .fold(h0.abs().max(h_final.abs()), f64::max);
    let pt_max_drift = backend
        .endpoint_pt
        .iter()
        .map(|p| (p - pt0).abs())
        .fold((pt_final - pt0).abs(), f64::max);

    let diagnostics = InvariantDiagnostics {
        h_initial: h0,
        h_final,
        h_max_abs_residual: h_max,
        p_t_initial: pt0,
        p_t_final: pt_final,
        p_t_max_abs_drift: pt_max_drift,
        non_finite_checks: backend.non_finite_checks,
        raw_vs_localized_lambda_separation: raw_sep,
        relative_tolerance: config.relative_tolerance,
        absolute_tolerance: config.absolute_tolerance,
    };

    Ok(IntegrationReport {
        outcome,
        diagnostics,
    })
}

/// Convenience: integrate with no event surfaces until the affine limit.
pub fn integrate_to_affine_limit(
    params: KerrParams,
    y0: &GeodesicState,
    config: &Dop853Config,
) -> Result<IntegrationReport, IntegrationError> {
    integrate(params, y0, config, &[])
}
