# ADR 0005: DOP853 Rust dependency selection

- Status: **Proposed** (Gate 1B0 spike complete; see `docs/research/gate-1b0-dop853-spike-report.md`)
- Updated: 2026-08-03 (Gate 1B0 remediation evidence)

## Context

ADR 0002 selects adaptive DOP853 with dense-output event localization for the
CPU `f64` geodesic oracle. Gate 1B0 executed executable spikes for
`ode_solvers::Dop853` and `ivp` DOP853 against a frozen checklist.

## Decision (proposed — not adopted)

**Remain Proposed.** Gate 1B0 measured capabilities:

| Requirement | ode_solvers 0.6.1 | ivp 0.6.0 |
|---|---|---|
| DOP853 f64 | Supported | Supported |
| 8D state | Supported | Supported |
| Vector tolerance direct | Unsupported | Supported |
| Accepted-step dense interpolant | **Unsupported** (public API: predetermined dx grid only) | **Supported** (SolOut `StepInterpolant` probed in callback) |
| Event localization (preferred arch) | **Unsupported** | **Supported** (callback interpolant → interrupt → restart) |
| Stop/restart semantics | **Unsupported** | **Supported** (x5 deterministic) |

Spike JSON: `artifacts/gate-1b0/` (regenerate via `cargo xtask evaluate --scope gate-1b0`).

**Do not add a production ODE crate until owner accepts ADR 0005.**

If owner accepts: **`ivp`** is the stronger measured fit for ADR 0002 event +
tolerance contract. `ode_solvers` remains viable only with an adapter accepting
predetermined-sample dense output instead of coefficient access.

## Alternatives

- From-scratch DOP853: rejected unless both crates fail the 1B0 contract and
  the owner approves a later ADR.
- Fork/upstream contribution: recommended if accepted-step coefficient API is
  required on `ode_solvers`.

## Consequences

Gate 1B1 owns the production adapter after ADR acceptance. Physics RHS remains in
`relativity-core`.

## References

- `docs/research/gate-1b0-dop853-spike-report.md`
- `docs/research/dop853-rust-dependency-audit.md`
- ADR 0002
