# Gate 1B0 DOP853 spike report

**Status:** remediation evidence (non-production)  
**Date:** 2026-08-03  
**Branch:** `gate-1b0-dop853-spike` (rebased on Gate 1A `main`)  
**Authoritative commit:** `5180cfe42db6f344603c6cba0d0d388da555ea88`

## Candidates (pinned)

| Crate | Version | License |
|---|---|---|
| `ode_solvers` | `=0.6.1` | Apache-2.0 |
| `ivp` | `=0.6.0` | Apache-2.0 |

## Schema

Contract `gate-1b0-v2` splits observational evidence:

- `RootLocalizationEvidence` — pure root finder output (no synthetic stop/restart)
- `SolverStopEvidence` / `RestartEvidence` — from actual solver experiments
- `CallbackStopEvidence` / `DomainErrorEvidence` — Experiment F
- `AcceptedStepProbe` — Experiment D interpolant measurements

## Decision matrix (authoritative)

| Requirement | ode_solvers | ivp |
|---|---|---|
| vector_tolerance_direct | Unsupported | Supported |
| accepted_step_dense_interpolation | **Unsupported** | **Supported** (StepInterpolant probed in SolOut) |
| event_localization_fit | **Unsupported** | **Supported** (callback StepInterpolant + interrupt) |
| stop_restart_semantics | **Unsupported** | **Supported** (x5 deterministic stop/restart) |

`ode_solvers` fixed-grid / external linear reconstruction recorded separately; does not satisfy preferred accepted-step event architecture.

## ivp Experiment D evidence

`DenseProbeSolOut` evaluates `StepInterpolant` at θ ∈ {0.1, 0.25, 0.5, 0.75, 0.9} inside accepted-step callbacks; records state vs analytic and step boundaries `[x0, x1]`.

## ivp Experiment E evidence

1. `ShoEventSolOut` observes accepted steps  
2. Event bracket on step endpoints  
3. Root localized via exact step `StepInterpolant`  
4. `ControlFlag::Interrupt` from callback  
5. Restart from localized state; compared to uninterrupted reference  
6. x5 in-process + subprocess determinism on event time/state/endpoint bits  

## Evaluator (strict)

`cargo xtask evaluate --scope gate-1b0` requires:

- exactly one A–G per candidate, all `passed == true`
- required evidence fields per experiment
- `validate_candidate_report` PASS (negative tests in contract)
- A–G determinism x5 in-process + whole-report subprocess x5
- decision matrix consistent with measured evidence

## Artifact digests

| Artifact | SHA-256 |
|---|---|
| ode-solvers.json | `19ae52bebdae0a1b81c2af87b2dd2fd8e5188c2b602fd559662fe9145ee00bb5` |
| ivp.json | `c8ac650708a8f2c821f2373d5e8022e78fedfc28973d314c3af82d0cadf14e21` |

## ADR 0005

**Remain Proposed.** Measured fit favors `ivp` for accepted-step interpolation → event localization → callback stop → deterministic restart. No production ODE dependency in Gate 1B0.

## Reproduce

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo xtask evaluate --scope gate-1b0
```

Authoritative PASS requires clean worktree.
