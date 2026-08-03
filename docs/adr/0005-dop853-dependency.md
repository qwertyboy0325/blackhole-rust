# ADR 0005: DOP853 Rust dependency selection

- Status: Proposed
- Date: 2026-08-03

## Context

ADR 0002 selects adaptive DOP853 with dense-output event localization for the
CPU `f64` geodesic oracle. Gate 1A must audit Rust crates before adopting one.
A from-scratch DOP853 implementation is not approved unless the audit shows no
credible crate can meet the contract.

## Decision (proposed)

Adopt `ode_solvers` (Apache-2.0) behind an internal integrator adapter in a
future `relativity-integrate` crate, subject to a Gate 1B spike that proves:

- component-scaled tolerances for the 8D Hamiltonian state;
- dense-output coefficient access adequate for disk/horizon/sky root finding;
- solout-driven geometry `h_max` guards;
- deterministic step statistics in evaluation reports.

Do not add the dependency in Gate 1A.

## Alternatives

- `ivp`: also provides DOP853; younger ecosystem; keep as backup.
- `diffsol`: MIT, strong events/dense output, but no DOP853 — requires revisiting
  ADR 0002.
- From-scratch DOP853: rejected unless the spike shows both crates fail the
  contract.

## Consequences

Gate 1B owns the adapter and calibration. Physics RHS evaluation remains in
`relativity-core` and must not depend on the ODE crate.

## References

- `docs/research/dop853-rust-dependency-audit.md`
- ADR 0002
