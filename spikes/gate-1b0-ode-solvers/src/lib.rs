//! Gate 1B0 spike runner for `ode_solvers::Dop853`.

#![forbid(unsafe_code)]

mod adapter;
mod audit;
mod domain_adapter;
mod runner;

pub use runner::run_candidate_report;

pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEP_VERSION: &str = "0.6.1";
