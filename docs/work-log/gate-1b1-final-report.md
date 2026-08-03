# Gate 1B1 Final Report (remediation)

## 1. Commits

Remediation commits on `gate-1b1-production-integrator` (see git log / PR #3).

## 2. Exact-event vs SurfaceApproach

- Exact Event: sign change or `f == 0.0` endpoint only.
- SurfaceApproach: opt-in `HorizonProximityPolicy` for OuterHorizon stall/proximity.
- Never serialize proximity/stall as `EventHit`.

## 3. Horizon corpus

Actual: `SurfaceApproach { event_id: OuterHorizon, reason: SolverStepSizeTooSmall }` with `signed_event_value > 0` and `<= approach_tolerance`.

## 4. Root localizer

`LocalizationTermination::{ExactEndpoint, EventValueTolerance, AffineWidthTolerance}`; typed `EventLocalizationDidNotConverge`; bounds enforced.

## 5. Error evidence

Backend tests: non-finite outcome → `NonFiniteState{Outcome}`; EventDomain via SolOut latch preserves EventId; generic solver status → `Solver`.

## 6. Kerr 3-level convergence

`d_medium_tight <= d_loose_medium` with H / p_t / steps recorded.

## 7. Cross-process corpus digests

`cargo xtask corpus-report --scope gate-1b1` ×3; identical SHA-256 of numerical JSON.

## 8. Artifact digest

`content_digest_excluding_digest_field` + sidecar `evaluation.content_digest.sha256`.

## 9–13

Authoritative evaluate commit, artifact digests, CI, ADR 0005 Accepted, no Gate 1B2 scope — filled after evaluate.
