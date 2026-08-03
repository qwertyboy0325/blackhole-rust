# Gate 1B1 Final Report (evidence closure)

## 1. Commits

- Remediation (architecture): `23ac27d` — exact events vs SurfaceApproach
- Evidence closure (this tip): localizer non-convergence self-check + Kerr evidence serialization
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

## 4. Root localizer non-convergence

Executable self-check: `localization_nonconvergence_self_check()` — evaluator PASSes only
when this returns structured evidence (no unconditional PASS).

| Path | EventId | iterations | residual | bracket_width |
|---|---|---|---|---|
| Midpoint stagnation | OuterHorizon | 1 | 1.0 | 2.0 |
| Iteration exhaustion | EscapeSphere | 80 | 1.0 | ≈ 8.272e-25 |

Stagnation success path returns mutually consistent `(lambda, state, residual)` from the
last sample (never a prior residual under a different λ).

Retained: value-tol, affine-width, exact endpoint, lost bracket, interpolant bounds.

## 5. Production errors

- Non-finite outcome interpreter → `NonFiniteState{Outcome}`
- EventDomain SolOut latch preserves EventId
- Generic solver status → `Solver` (not PhysicsDomain / EventDomain / SurfaceApproach)

## 6. Kerr 3-level convergence (serialized)

Criterion: `d_medium_tight <= d_loose_medium + 1e-15` (documented slack).

| Level | accepted/rejected | rhs | H_max | p_t drift |
|---|---|---|---|---|
| loose | 6 / 0 | 92 | 2.602e-16 | 0 |
| medium | 6 / 0 | 92 | 2.602e-16 | 0 |
| tight | 6 / 0 | 92 | 2.602e-16 | 0 |

Measured distances (fixture affine window `0.5`):

```text
d_loose_medium = 0
d_medium_tight = 0
passed = true
```

Full per-run evidence (tolerances, endpoint_bits, H, p_t, steps) is serialized in
`artifacts/gate-1b1/evaluation.json` under `kerr_convergence`.

## 7. Cross-process corpus digests

`cargo xtask corpus-report --scope gate-1b1` ×3:

```text
38d4be35b61522b65599003e46ceb2248ad1987902d9b7df126b6769067629f7
```

(identical; numerical JSON, not test stdout)

## 8. Artifact digest

Convention: `content_digest_excluding_digest_field`

Recorded by authoritative `cargo xtask evaluate --scope gate-1b1` on a clean tip;
see `artifacts/gate-1b1/evaluation.content_digest.sha256` and PR #3 body for the
commit-bound value (digest input includes `commit`).

## 9. Authoritative evaluator

`cargo xtask evaluate --scope gate-1b1` → **PASS** (`authoritative: true`) on the
evidence-closure tip (clean worktree). Commit and content digest: PR #3.

## 10. Artifacts

- `artifacts/gate-1b1/evaluation.json`
- `artifacts/gate-1b1/evaluation.md`
- `artifacts/gate-1b1/evaluation.content_digest.sha256`

## 11. Scope

No Gate 1B2 / disk / radiometry / image / GPU / wgpu / egui / GUI introduced.

## 12. ADR 0005

**Accepted**; `ivp = "=0.6.0"` unchanged.
