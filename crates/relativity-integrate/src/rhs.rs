//! Hamiltonian RHS via Gate 1A `evaluate_hamiltonian` — no projection, no duplicated Kerr math.

use relativity_core::{evaluate_hamiltonian, KerrParams};
use std::cell::RefCell;
use std::rc::Rc;

use crate::error::{IntegrationError, IntegrationStage};
use crate::state::GeodesicState;

#[derive(Clone, Default)]
pub struct DomainLatch(pub Rc<RefCell<Option<IntegrationError>>>);

impl DomainLatch {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(None)))
    }

    pub fn take(&self) -> Option<IntegrationError> {
        self.0.borrow_mut().take()
    }

    pub fn set(&self, err: IntegrationError) {
        *self.0.borrow_mut() = Some(err);
    }
}

pub struct HamiltonianRhs {
    pub params: KerrParams,
    pub latch: DomainLatch,
    pub eval_count: Rc<RefCell<u64>>,
    pub non_finite_checks: Rc<RefCell<u64>>,
    pub h_samples: Rc<RefCell<Vec<f64>>>,
    pub p_t_samples: Rc<RefCell<Vec<f64>>>,
}

impl HamiltonianRhs {
    pub fn new(params: KerrParams, latch: DomainLatch) -> Self {
        Self {
            params,
            latch,
            eval_count: Rc::new(RefCell::new(0)),
            non_finite_checks: Rc::new(RefCell::new(0)),
            h_samples: Rc::new(RefCell::new(Vec::new())),
            p_t_samples: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Share interior `Rc` handles without duplicating counters/samples.
    pub fn share(&self) -> Self {
        Self {
            params: self.params,
            latch: self.latch.clone(),
            eval_count: self.eval_count.clone(),
            non_finite_checks: self.non_finite_checks.clone(),
            h_samples: self.h_samples.clone(),
            p_t_samples: self.p_t_samples.clone(),
        }
    }

    pub fn derivative(&self, _lam: f64, y: &[f64], dy: &mut [f64]) {
        *self.eval_count.borrow_mut() += 1;
        *self.non_finite_checks.borrow_mut() += 1;
        if y.iter().any(|v| !v.is_finite()) {
            self.latch.set(IntegrationError::NonFiniteState {
                stage: IntegrationStage::Rhs,
            });
            dy.fill(f64::NAN);
            return;
        }
        let Ok(state) = GeodesicState::from_array(y) else {
            self.latch.set(IntegrationError::NonFiniteState {
                stage: IntegrationStage::Rhs,
            });
            dy.fill(f64::NAN);
            return;
        };
        match evaluate_hamiltonian(&self.params, &state.position, &state.momentum) {
            Ok(ev) => {
                self.h_samples.borrow_mut().push(ev.h);
                self.p_t_samples.borrow_mut().push(state.momentum.t);
                dy[0] = ev.dx_dlambda.t;
                dy[1] = ev.dx_dlambda.x;
                dy[2] = ev.dx_dlambda.y;
                dy[3] = ev.dx_dlambda.z;
                dy[4] = ev.dp_dlambda.t;
                dy[5] = ev.dp_dlambda.x;
                dy[6] = ev.dp_dlambda.y;
                dy[7] = ev.dp_dlambda.z;
                if dy.iter().any(|v| !v.is_finite()) {
                    self.latch.set(IntegrationError::NonFiniteState {
                        stage: IntegrationStage::Rhs,
                    });
                }
            }
            Err(source) => {
                self.latch.set(IntegrationError::from_core(source));
                dy.fill(f64::NAN);
            }
        }
    }
}

pub fn initial_hamiltonian(
    params: &KerrParams,
    state: &GeodesicState,
) -> Result<f64, IntegrationError> {
    evaluate_hamiltonian(params, &state.position, &state.momentum)
        .map(|e| e.h)
        .map_err(IntegrationError::from_core)
}
