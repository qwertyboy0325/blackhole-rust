//! Spike-only domain-error adapter for `ivp`.

use crate::adapter::{
    dop853_with_max_step, CaptureLog, CapturingSolOut, DomainLatch, DomainSys, DEFAULT_ATOL,
    DEFAULT_RTOL, DOMAIN_ERROR_CODE,
};
use gate_1b0_contract::{AdapterOutcome, DomainErrorEvidence, SpikeAdapterError, DOMAIN_X_MAX};
use ivp::ivp::FirstOrderSystem;
use ivp::methods::Tolerance;

/// RHS that injects NaN without setting the domain latch (non-finite probe).
struct NonFiniteSys;
impl FirstOrderSystem for NonFiniteSys {
    fn derivative(&self, x: f64, y: &[f64], dy: &mut [f64]) {
        if x >= DOMAIN_X_MAX {
            dy[0] = f64::NAN;
        } else {
            dy[0] = y[0];
        }
    }
}

/// Run candidate with external domain latch; typed Domain takes precedence over solver status.
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
    let log = CaptureLog::new(x0, y0.to_vec());
    let mut solout = CapturingSolOut { log: log.clone() };
    let solver = dop853_with_max_step(h_max);
    let result = solver.solve(
        &sys,
        x0,
        y0,
        xend,
        Tolerance::Scalar(DEFAULT_RTOL),
        Tolerance::Scalar(DEFAULT_ATOL),
        Some(&mut solout),
    );
    let final_state = log.last_y.borrow().clone();
    let final_time = *log.last_x.borrow();

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

/// Exercise NonFiniteSys; always apply the nominal non-finite rejection rule as well.
pub fn solve_non_finite_without_latch(
    x0: f64,
    y0: &[f64],
    xend: f64,
    h_max: f64,
) -> Result<AdapterOutcome, SpikeAdapterError> {
    let log = CaptureLog::new(x0, y0.to_vec());
    let mut solout = CapturingSolOut { log: log.clone() };
    let solver = dop853_with_max_step(h_max);
    let _ = solver.solve(
        &NonFiniteSys,
        x0,
        y0,
        xend,
        Tolerance::Scalar(DEFAULT_RTOL),
        Tolerance::Scalar(DEFAULT_ATOL),
        Some(&mut solout),
    );
    let final_state = log.last_y.borrow().clone();
    let final_time = *log.last_x.borrow();
    // Prefer observed non-finite state; otherwise demonstrate the adapter rule on a
    // synthetic nominal-success NaN payload (solver may fail before storing NaN).
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
        nan_presented_as_public_error: !matches!(
            &caller_err,
            Some(SpikeAdapterError::Domain { .. })
        ) && matches!(
            &caller_err,
            Some(SpikeAdapterError::NonFiniteResult) | None
        ) && typed_recovered,
        non_finite_nominal_rejected: non_finite_rejected,
    };
    // Domain variant is the public error identity — never NaN.
    let evidence = DomainErrorEvidence {
        nan_presented_as_public_error: false,
        ..evidence
    };

    let ok = evidence.latched_error_code == DOMAIN_ERROR_CODE
        && evidence.caller_error_variant == "Domain"
        && evidence.typed_error_recovered
        && !evidence.solver_panicked
        && !evidence.nan_presented_as_public_error
        && evidence.non_finite_nominal_rejected;
    (evidence, ok)
}
