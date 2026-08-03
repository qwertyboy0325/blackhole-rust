//! Gate 1B0 experimental contract — shared experiment schema and analytic systems.
//! Not a production integration abstraction.

#![forbid(unsafe_code)]

pub mod adapter_outcome;
pub mod audit;
pub mod determinism;
pub mod digest;
pub mod event;
pub mod schema;
pub mod systems;
pub mod validate;

pub use adapter_outcome::{
    interpret_domain_result, states_match, AdapterOutcome, RawSolverStop, SpikeAdapterError,
};
pub use audit::audit_direct_dependency;
pub use determinism::{
    endpoint_bits, repeat_in_process, repeat_in_process_sig, signature_join, RepeatSummary,
};
pub use digest::json_digest;
pub use event::localize_root;
pub use schema::*;
pub use systems::*;
pub use validate::{validate_candidate_report, ValidationIssue};
