# Gate 1B0 DOP853 spike report

**Status:** lifecycle/error-propagation closure (non-production)  
**Date:** 2026-08-03  
**Branch:** `gate-1b0-dop853-spike` (rebased on Gate 1A `main` @ `dc38619`)  
**PR:** #2 (Gate 1B0 only)  
**Authoritative evaluate commit:** `bd561cbcc4f1d0307df8c5a7e88b24bc9cdad840`

| Artifact | SHA-256 |
|---|---|
| ode-solvers.json | `cd92ea430f751f82a7a65dcbc2a422075acbba41f88116fe376ac1583b45e41a` |
| ivp.json | `2961eba63cb9aa1900d5d90dbaef7db93361b9cc0d929146ea21ba24d3d70d9c` |

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
