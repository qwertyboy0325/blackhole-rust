# Gate 1B0 DOP853 spike report

**Status:** lifecycle/error-propagation closure (non-production)  
**Date:** 2026-08-03  
**Branch:** `gate-1b0-dop853-spike` (rebased on Gate 1A `main` @ `dc38619`)  
**PR:** #2 (Gate 1B0 only)  
**Authoritative commit:** `d072e32d5fc3a62d80ca45961ca37ce899661b93`

| Artifact | SHA-256 |
|---|---|
| ode-solvers.json | `f2c5bb1ecefca8bd3c97cbf8717d81ee98bded0f619d31725bdacd63987d7eb9` |
| ivp.json | `2404ccc73a4fe90943f9db01671828023fda4f2f712dab23d96d8897fc851123` |

## Candidates (pinned)

| Crate | Version | License |
|---|---|---|
| `ode_solvers` | `=0.6.1` | Apache-2.0 |
| `ivp` | `=0.6.0` | Apache-2.0 |

## Schema (`gate-1b0-v3`)

- `RootLocalizationEvidence` — pure root finder (no lifecycle claims)
- `SolverStopEvidence` records separately:
  - `raw_solver_stop_time/state`
  - `localized_event_time/state`
  - `adapter_returned_time/state`
  - `adapter_matches_localized`
- Preferred adapter contract: `AdapterOutcome::Event { time, state, raw_solver_stop }`
- `SpikeAdapterError::{Domain, NonFiniteResult, Solver}` — pattern-matchable
- Shallow probe: `shallow_sign_changing_crossing` only (tangent not claimed)

## Decision matrix

| Requirement | ode_solvers | ivp |
|---|---|---|
| vector_tolerance_direct | Unsupported | Supported |
| accepted_step_dense_interpolation | Unsupported | Supported |
| event_localization_fit | Unsupported | Supported |
| stop_restart_semantics | Unsupported | Supported |
| typed domain error | Supported | Supported |

## ivp lifecycle (Exp E)

Caller receives adapter-localized Event outcome; raw solver stop is the accepted-step endpoint and is recorded separately. Restart uses adapter-returned state. x5 in-process + subprocess.

## Typed domain errors (Exp F)

Both candidates: latch → `SpikeAdapterError::Domain { code: DOMAIN_X_EXCEEDED }`.  
Non-finite nominal success → `NonFiniteResult`. NaN is not the public error identity.

## ADR 0005

**Remain Proposed.** Measured fit favors `ivp`. No production ODE dependency.

## Reproduce

```bash
cargo xtask evaluate --scope gate-1b0
```
