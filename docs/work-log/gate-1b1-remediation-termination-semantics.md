# Gate 1B1 Remediation — Termination Semantics Closure

## Taxonomy

| Outcome | Meaning |
|---|---|
| `Event(EventHit)` | Strict sign-changing bracket **or** exact endpoint root (`f == 0.0`) |
| `SurfaceApproach` | Opt-in OuterHorizon proximity only; **not** a crossing |
| `AffineLimit` | Reached affine limit without event/approach |

`event_value_tolerance` = localization convergence only.
`HorizonProximityPolicy` = separate opt-in; OuterHorizon only; does not apply to EscapeSphere.

Physical surface remains `f = r_oblate - r_plus`. Positive `f` is never an OuterHorizon EventHit.

Horizon stall (f64 Cartesian KS `StepSizeTooSmall` as `r → r₊⁺`) remains an **unresolved numerical investigation** item. Proximity policy documents the stall; it does not prove crossing.

## Corpus

`schwarzschild_inward_horizon` → `SurfaceApproach(OuterHorizon, SolverStepSizeTooSmall)` with signed residual and approach tolerance recorded.

`minkowski_escape_sphere` → true localized `EventHit`.

## Digests

- Cross-process: `cargo xtask corpus-report --scope gate-1b1` ×3; SHA-256 of canonical numerical JSON.
- Evaluation artifact: `content_digest_excluding_digest_field` (hash of report with digest field empty).
