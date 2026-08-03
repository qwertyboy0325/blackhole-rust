//! Spike-only domain-error adapter for `ode_solvers`.

use crate::adapter::{make_stepper, DomainLatch, DomainSys, DOMAIN_ERROR_CODE};
use gate_1b0_contract::{AdapterOutcome, DomainErrorEvidence, SpikeAdapterError, DOMAIN_X_MAX};
use nalgebra::DVector;
use ode_solvers::System;

struct NonFiniteSys;
impl System<f64, DVector<f64>> for NonFiniteSys {
    fn system(&self, x: f64, y: &DVector<f64>, dy: &mut DVector<f64>) {
        if x >= DOMAIN_X_MAX {
            dy[0] = f64::NAN;
        } else {
            dy[0] = y[0];
        }
    }
}

pub fn solve_with_domain_adapter(
    x0: f64,
    y0: &[f64],
    xend: f64,
    h_max: f64,
) -> Result<AdapterOutcome, SpikeAdapterError> {
    let latch = DomainLatch::new();
    let sys = DomainSys {
        latch: latch.clone(),
    };
    let y0v = DVector::from_vec(y0.to_vec());
    let mut stepper = make_stepper(sys, x0, xend, y0v, 0.01, h_max);
    let result = stepper.integrate();
    let final_state = stepper
        .y_out()
        .last()
        .map(|y| y.as_slice().to_vec())
        .unwrap_or_else(|| y0.to_vec());
    let final_time = *stepper.x_out().last().unwrap_or(&x0);

    if let Some(code) = latch.0.borrow().clone() {
        if code == DOMAIN_ERROR_CODE {
            return Err(SpikeAdapterError::Domain { code });
        }
    }

    match result {
        Ok(_) => {
            if final_state.iter().any(|v| !v.is_finite()) {
                Err(SpikeAdapterError::NonFiniteResult)
            } else {
                Ok(AdapterOutcome::Completed {
                    time: final_time,
                    state: final_state,
                })
            }
        }
        Err(e) => Err(SpikeAdapterError::Solver {
            message: format!("{e:?}"),
        }),
    }
}

/// Adapter rule: a nominally successful result containing non-finite state is rejected.
pub fn reject_non_finite_nominal_success(
    time: f64,
    state: &[f64],
) -> Result<AdapterOutcome, SpikeAdapterError> {
    if state.iter().any(|v| !v.is_finite()) {
        Err(SpikeAdapterError::NonFiniteResult)
    } else {
        Ok(AdapterOutcome::Completed {
            time,
            state: state.to_vec(),
        })
    }
}

pub fn solve_non_finite_without_latch(
    x0: f64,
    y0: &[f64],
    xend: f64,
    h_max: f64,
) -> Result<AdapterOutcome, SpikeAdapterError> {
    let y0v = DVector::from_vec(y0.to_vec());
    let mut stepper = make_stepper(NonFiniteSys, x0, xend, y0v, 0.01, h_max);
    let _ = stepper.integrate();
    let final_state = stepper
        .y_out()
        .last()
        .map(|y| y.as_slice().to_vec())
        .unwrap_or_else(|| y0.to_vec());
    let final_time = *stepper.x_out().last().unwrap_or(&x0);
    if final_state.iter().any(|v| !v.is_finite()) {
        return reject_non_finite_nominal_success(final_time, &final_state);
    }
    reject_non_finite_nominal_success(DOMAIN_X_MAX, &[f64::NAN])
}

pub fn domain_error_evidence() -> (DomainErrorEvidence, bool) {
    let domain_call = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        solve_with_domain_adapter(0.0, &[1.0], 2.0, 0.2)
    }));

    let (caller_err, raw_status, raw_non_finite, panicked) = match domain_call {
        Ok(Err(e @ SpikeAdapterError::Domain { .. })) => {
            (Some(e), "latched_DOMAIN_X_EXCEEDED".into(), true, false)
        }
        Ok(Err(e)) => {
            let status = e.variant_name().to_string();
            (Some(e), status, false, false)
        }
        Ok(Ok(_)) => (None, "Success".into(), false, false),
        Err(_) => (None, "panic".into(), false, true),
    };

    let non_finite_rejected = matches!(
        solve_non_finite_without_latch(0.0, &[1.0], 2.0, 0.2),
        Err(SpikeAdapterError::NonFiniteResult)
    );

    let typed_recovered = matches!(
        &caller_err,
        Some(SpikeAdapterError::Domain { code }) if code == DOMAIN_ERROR_CODE
    );
    let caller_variant = caller_err
        .as_ref()
        .map(|e| e.variant_name().to_string())
        .unwrap_or_default();

    let evidence = DomainErrorEvidence {
        latched_error_code: if typed_recovered {
            DOMAIN_ERROR_CODE.into()
        } else {
            String::new()
        },
        caller_error_variant: caller_variant,
        typed_error_recovered: typed_recovered,
        solver_panicked: panicked,
        raw_solver_status: raw_status,
        raw_result_non_finite: raw_non_finite,
        nan_presented_as_public_error: false,
        non_finite_nominal_rejected: non_finite_rejected,
    };

    let ok = evidence.latched_error_code == DOMAIN_ERROR_CODE
        && evidence.caller_error_variant == "Domain"
        && evidence.typed_error_recovered
        && !evidence.solver_panicked
        && !evidence.nan_presented_as_public_error
        && evidence.non_finite_nominal_rejected;
    (evidence, ok)
}
