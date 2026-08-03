# Gate 1B1 Final Report (remediation)

## 1. Commits

- Remediation: `23ac27d` — exact events vs SurfaceApproach + evidence closure
- Authoritative evaluate head: `23ac27d5b74384530676172a22374b69602fc4ac`
- Draft PR: https://github.com/qwertyboy0325/blackhole-rust/pull/3

## 2. Exact-event vs SurfaceApproach

- `Event(EventHit)`: strict sign-changing bracket or exact endpoint root (`f == 0.0`)
- `SurfaceApproach`: opt-in `HorizonProximityPolicy` for OuterHorizon only
- `event_value_tolerance` is localization convergence only — never promotes proximity to EventHit
- EscapeSphere / arbitrary surfaces never receive proximity capture

## 3. Horizon corpus

`schwarzschild_inward_horizon` → `SurfaceApproach(OuterHorizon, SolverStepSizeTooSmall)`  
with `signed_event_value > 0` and `<= approach_tolerance`.  
Horizon stall remains an unresolved f64 Cartesian-KS numerical investigation item.

## 4. Root localizer

`LocalizationTermination::{ExactEndpoint, EventValueTolerance, AffineWidthTolerance}`  
typed `EventLocalizationDidNotConverge`; interpolant bounds enforced; tests cover
value/width/exact/stagnation/exhaustion/lost-bracket/bounds.

## 5. Production errors

- Non-finite outcome interpreter → `NonFiniteState{Outcome}`
- EventDomain SolOut latch preserves EventId
- Generic solver status → `Solver` (not PhysicsDomain / EventDomain / SurfaceApproach)

## 6. Kerr 3-level convergence

loose / medium / tight; `d_medium_tight <= d_loose_medium`; H / p_t / steps recorded.

## 7. Cross-process corpus digests

`cargo xtask corpus-report --scope gate-1b1` ×3:

```text
38d4be35b61522b65599003e46ceb2248ad1987902d9b7df126b6769067629f7
```

(identical; numerical JSON, not test stdout)

## 8. Artifact digest

Convention: `content_digest_excluding_digest_field`

```text
590f40a06a647f437fed4e83412e60087f6f2dd424a18f00b0630fec98cc76ec
```

Sidecar: `artifacts/gate-1b1/evaluation.content_digest.sha256`

## 9. Authoritative evaluator

`cargo xtask evaluate --scope gate-1b1` → **PASS** (`authoritative: true`) at `23ac27d`

## 10. Artifacts

- `artifacts/gate-1b1/evaluation.json`
- `artifacts/gate-1b1/evaluation.md`
- `artifacts/gate-1b1/evaluation.content_digest.sha256`

## 11. CI

`fmt` / `clippy -D warnings` / `test --workspace` PASS under evaluator.

## 12. ADR 0005

**Accepted**; `ivp = "=0.6.0"` unchanged.

## 13. Scope

No Gate 1B2 / disk / radiometry / image / GPU / wgpu / egui / GUI introduced.
