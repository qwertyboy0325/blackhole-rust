//! Spike-only adapter outcome and typed error surface (not production).

use serde::{Deserialize, Serialize};

/// Raw solver endpoint at interrupt / completion (accepted-step values).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSolverStop {
    pub time: f64,
    pub state: Vec<f64>,
}

/// Adapter-owned result after interpreting the solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdapterOutcome {
    Completed {
        time: f64,
        state: Vec<f64>,
    },
    /// Preferred event contract: caller receives localized root; raw stop retained separately.
    Event {
        time: f64,
        state: Vec<f64>,
        raw_solver_stop: RawSolverStop,
    },
    Interrupted {
        time: f64,
        state: Vec<f64>,
    },
}

impl AdapterOutcome {
    pub fn time(&self) -> f64 {
        match self {
            Self::Completed { time, .. }
            | Self::Event { time, .. }
            | Self::Interrupted { time, .. } => *time,
        }
    }

    pub fn state(&self) -> &[f64] {
        match self {
            Self::Completed { state, .. }
            | Self::Event { state, .. }
            | Self::Interrupted { state, .. } => state,
        }
    }
}

/// Public spike adapter error — pattern-matchable by callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "variant", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpikeAdapterError {
    Domain { code: String },
    NonFiniteResult,
    Solver { message: String },
}

impl SpikeAdapterError {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Domain { .. } => "Domain",
            Self::NonFiniteResult => "NonFiniteResult",
            Self::Solver { .. } => "Solver",
        }
    }

    pub fn domain_code(&self) -> Option<&str> {
        match self {
            Self::Domain { code } => Some(code.as_str()),
            _ => None,
        }
    }
}

/// Interpret latch + solver status + final state into a typed adapter result.
pub fn interpret_domain_result(
    latch_code: Option<&str>,
    solver_ok: bool,
    solver_status: &str,
    final_state: &[f64],
) -> Result<AdapterOutcome, SpikeAdapterError> {
    if let Some(code) = latch_code {
        if code == "DOMAIN_X_EXCEEDED" {
            return Err(SpikeAdapterError::Domain {
                code: code.to_string(),
            });
        }
    }
    if !solver_ok {
        return Err(SpikeAdapterError::Solver {
            message: solver_status.to_string(),
        });
    }
    if final_state.iter().any(|v| !v.is_finite()) {
        return Err(SpikeAdapterError::NonFiniteResult);
    }
    Ok(AdapterOutcome::Completed {
        time: 0.0,
        state: final_state.to_vec(),
    })
}

pub fn states_match(a: &[f64], b: &[f64], tol: f64) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|(x, y)| (x - y).abs() <= tol || (x.to_bits() == y.to_bits()))
}
