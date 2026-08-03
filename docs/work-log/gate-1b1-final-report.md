# Gate 1B1 Final Report

## 1. Branch / commits / PR

- Branch: `gate-1b1-production-integrator`
- Commits: `86332e4` (ADR Accepted), `1afbb8d` (production crate + evaluator)
- Authoritative evaluate: PASS at `1afbb8d`; artifact digest
  `1df50a6a8fdfc5dea7c4b510cef02b9bf80323b3f102d6d740624697b8c9f1ea`

## 2. ADR 0005

Status **Accepted**; Decision `ivp = "=0.6.0"`; evidence commit `bd561cb` with
Gate 1B0 artifact digests recorded in the ADR.

## 3. Production public API

`relativity-integrate::{integrate, GeodesicState, Dop853Config, IntegrationOutcome,
EventSurface, OuterHorizon, EscapeSphere, IntegrationError, …}` — no public `ivp`.

## 4. Dependency pin

`crates/relativity-integrate/Cargo.toml`: `ivp = "=0.6.0"`.

## 5. Tolerance model

Per-component vector rtol/atol via `Tolerance::Vector`; diagnostic defaults only.

## 6. Event abstraction

`EventSurface` + `CrossingDirection::{Any,Increasing,Decreasing}`; sign-change
primary; endpoint/stall capture within value tolerance for f64 horizon approach.

## 7. Raw stop vs localized outcome

Escape-sphere tests: interpolant localization; raw stop λ/state retained and
separated; adapter outcome equals localized state.

## 8. Horizon / escape corpus

`schwarzschild_inward_horizon` → `Event(OuterHorizon)`;
`minkowski_escape_sphere` → `Event(EscapeSphere)`.

## 9. Minkowski analytic

Straight null line + constant momentum; escape λ ≈ 10 for r: 10 → 20.

## 10. Kerr invariants

`p_t` drift reported; H residual reported; no projection; tighter tol convergence.

## 11. Typed errors

`PhysicsDomain`, `EventDomain` (preserves EventId), `StepLimitExceeded`,
`NonFiniteState`, `Solver` (incl. StepSizeTooSmall without surface capture).

## 12. Determinism

In-process ×5 per corpus case; subprocess corpus test ×3 in evaluator.

## 13. Artifacts

`artifacts/gate-1b1/evaluation.json` + `evaluation.md` (digests in JSON).

## 14. Commands / CI

`cargo fmt`, `clippy -D warnings`, `test --workspace`;
`cargo xtask evaluate --scope gate-1b1`;
`cargo xtask integrate-ray …`.

## 15. Remaining risks

- Horizon crossing in f64 Cartesian KS is stiff; relies on value-tol endpoint/stall
  capture rather than a deep interior sample.
- Quasi-Minkowski uses `M = 1e-18` (core forbids `M = 0`).
- Exact extremal Kerr excluded from corpus.

## 16. Recommended Gate 1B2 scope

Disk intersection event + opaque first-hit policy; ray-bundle termination
taxonomy; optional named horizon safety offset as a separate policy surface;
broader Kerr camera corpus; CI wire-up for `evaluate --scope gate-1b1`.
