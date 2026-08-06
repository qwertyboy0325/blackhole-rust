# E1 execution-performance report

## Result

Post-merge Workstream B on branch `e1-execution-performance`, closed against
owner comment `5202437486`. Authoritative evaluate **PASS**. Research
recommendation unchanged: `PAUSE_RESEARCH_WEDGE`. Hypothesis unchanged:
`NOT_SUPPORTED_ON_E0_CORPUS`.

| Item | Value |
| --- | --- |
| Base | `origin/main` @ `705ae5ca8806ed2e0069ff4c2700bde921bad33b` (PR #15 merge) |
| Evaluated commit | `de035b2ce0c1129fa337188e2573a6572159ca37` |
| Tracking | GitHub Issue #12 / PR #16 (draft) |
| Owner closure | `5202437486` |
| E0 lock digest | `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb` (unchanged) |
| Baseline oracle digest | `ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383` (unchanged) |
| Canonical experiment digest | `fa947adb84f97bba65a126716f4180df6ad54568c27630e24d5cc74ebbb1faff` |
| Evaluate digest | `0f85e76f95d55f776b6f923bc18cfba56a010ec4f6bdfc4db11ee6de84728b54` |
| Hypothesis | `NOT_SUPPORTED_ON_E0_CORPUS` |
| Recommendation | `PAUSE_RESEARCH_WEDGE` |

```text
Scientific checks                 PASS (all)
Shared reference authority        PASS (marker + all oracle/PPM digests)
Progressive↔cold equivalence      PASS (opaque + 2 crops × 3 methods × 5 budgets)
Serial/parallel smoke             PASS
Repeat crop determinism           PASS (minimal evidence bundle)
Minimal I/O                       REAL (skips reconstruction.ppm)
Timing semantics                  RESTORED (cumulative/per-budget + phases)
E0 lock / baseline oracle         UNCHANGED
Hypothesis / Pareto / classify    UNCHANGED
PR #16                            DRAFT / awaiting owner
E2 / Gate 2B2                     NOT STARTED
```

## Closure of `5202437486`

1. **Shared reference** — `validate_existing` checks marker content against the
   exact lock digest, every source/crop `OracleFrame` (validate + scientific
   digest), and every `reference.ppm` against lock `reference_image_digest`.
   Tamper tests cover marker, missing case, oracle frame, source PPM, crop PPM.
   `load_cases` also enforces PPM digests for filtered `--reference-dir` runs.
2. **Progressive↔cold** — evaluator runs both ladders on
   `kerr0999-edge-opaque` + both boundary crops × 3 methods × full budgets;
   compares unique rays, scientific/RGB/parity, maps/digests, and adaptive split
   sequences (ignoring only `requested_target`/`overshoot`/`step`). Uniform
   progressive stencil nesting is unit-tested.
3. **Minimal I/O** — `--write-artifacts minimal` writes the determinism evidence
   bundle only (`scientific-error-summary.json`, `schedule-summary.json`,
   `sample-mask.pgm`, `leaf-depth.pgm`, `outcome-disagreement.pgm`,
   `reconstruction.digest`). Full mode also writes `reconstruction.ppm`.
4. **Timing** — `wall_clock_seconds` includes tracing/scheduling through
   artifacts. Progressive = cumulative-to-budget; cold = per-budget-from-zero.
   Phase fields: `tracing_and_schedule_*`, `reconstruction_*`, `metric_*`,
   `artifact_*` (stripped from scientific digests).

## CI follow-up (`5202853029`)

Unit-test fixture no longer materializes the E0 corpus and does not read
ignored `artifacts/`. Tamper tests use a hermetic 1-source + 1-crop synthetic
session via `validate_session_tree`. Production `validate_existing` still
requires the pinned lock digest and validates all eight reference cases in the
authoritative evaluator.

## Execution profile (non-binding)

Wall clock for authoritative evaluate process ≈ **383 s** (release).

| Phase | Seconds |
| --- | --- |
| Shared reference validate | 0.158 |
| Determinism smoke (t1 + tN) | 31.561 |
| Progressive↔cold equivalence | 72.094 |
| Canonical full matrix | 157.855 |
| Repeat crops (2×) | 6.005 |

## Owner review

Stop for owner. Do not merge without acceptance. Do not start estimator
iteration, E2, E3, or Gate 2B2.
