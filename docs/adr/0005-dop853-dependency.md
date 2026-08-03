# ADR 0005: DOP853 Rust dependency selection

- Status: Proposed
- Date: 2026-08-03
- Updated: 2026-08-03 (PR #1 owner review)

## Context

ADR 0002 selects adaptive DOP853 with dense-output event localization for the
CPU `f64` geodesic oracle. Gate 1A audits crates only. Owner review requires a
Gate 1B0 spike proving the exact callback, dense-output/coefficient,
stop/restart, guard, and statistics contract before selection.

## Decision (proposed — not adopted)

Defer crate selection until a Gate 1B0 spike compares `ode_solvers::Dop853` and
`ivp` DOP853 against the frozen checklist in
`docs/research/dop853-rust-dependency-audit.md`.

Notes from the survey (not a selection):

- `ode_solvers` provides DOP853 with scalar tolerances; public accepted-step
  dense-coefficient access for an external localizer is unproven.
- `ivp` exposes vector tolerances, `SolOut`, DOP853 interpolation, and
  statistics, but is younger and also requires the spike.
- `diffsol` lacks DOP853.

Do not add an ODE dependency in Gate 1A.

## Alternatives

- From-scratch DOP853: rejected unless both crates fail the 1B0 contract and
  the owner approves a later ADR.

## Consequences

Gate 1B owns the adapter and calibration after 1B0 evidence. Physics RHS
evaluation remains in `relativity-core` and must not depend on the ODE crate.

## References

- `docs/research/dop853-rust-dependency-audit.md`
- ADR 0002
