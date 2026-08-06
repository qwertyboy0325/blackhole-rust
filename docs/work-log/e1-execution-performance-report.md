# E1 execution-performance report

## Result

Post-merge Workstream B (B1–B5, B7) on branch `e1-execution-performance`.
Authoritative evaluate **PASS**. Research recommendation unchanged:
`PAUSE_RESEARCH_WEDGE`. Hypothesis unchanged: `NOT_SUPPORTED_ON_E0_CORPUS`.

| Item | Value |
| --- | --- |
| Base | `origin/main` @ `705ae5ca8806ed2e0069ff4c2700bde921bad33b` (PR #15 merge) |
| Evaluated commit | `2d8b8fa2e85ba271302c6c53d6c771a62b6183cb` |
| Tracking | GitHub Issue #12 / new PR (not #15 continuation) |
| E0 lock digest | `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb` (unchanged) |
| Baseline oracle digest | `ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383` (unchanged) |
| Canonical experiment digest | `7e03da08beead5a987a8fbe4feafaf8787a4496539d1a963c9798469a25559e3` |
| Evaluate digest | `f4b93924893151da3488f3f8688353d9a1e5ac0b08fa325d15800777be2272f3` |
| Hypothesis | `NOT_SUPPORTED_ON_E0_CORPUS` |
| Recommendation | `PAUSE_RESEARCH_WEDGE` |

```text
Scientific checks                 PASS (all)
Shared reference validated        PASS
Serial/parallel smoke             PASS (digest match)
Repeat crop determinism           PASS
E0 lock / baseline oracle         UNCHANGED
Hypothesis / Pareto / classify    UNCHANGED (out of scope)
execution-profile.json            non-PASS instrumentation only
E2 / Gate 2B2                     NOT STARTED
```

Digest note: experiment / evaluate content digests **may change** vs pre-perf
runs because progressive schedules and timing-stripped profile wiring differ;
scientific metrics, indices, sample parity, and E0 lock are preserved.

## Shipped scope

| Item | Class | Status |
| --- | --- | --- |
| B1 VerifiedReferenceSession + `--reference-dir` | REQUIRED | shipped |
| B2 Progressive ladders (`--ladder progressive`, cold rollback) | REQUIRED | shipped |
| B3 `uniform_unique_ray_count` (no throwaway trace) | REQUIRED | shipped |
| B4 One Rayon pool per experiment process | REQUIRED | shipped |
| B5 Batch scientific adapt (parity-gated) | OPTIONAL | shipped (batch path) |
| B6 Case concurrency | DEFER | case-serial |
| B7 Smoke/repeat minimal I/O + `execution-profile.json` | REQUIRED | shipped |

## Execution profile (non-binding)

Wall clock for authoritative evaluate process ≈ **301 s** (release).

| Phase | Seconds |
| --- | --- |
| Shared reference generate/validate | 6.359 |
| Determinism smoke (t1 + tN) | 30.884 |
| Canonical full matrix | 154.400 |
| Repeat crops (2×) | 5.640 |

Profile fields: `ladder_mode=progressive`,
`pool_policy=one_per_experiment_process`,
`subprocess_count_experiments=5`,
`shared_reference_dir=artifacts/e1-adaptive-sampling/shared-reference`.

## Invariants confirmed

- Sampler/`TraceContext` never receives `OracleFrame`
- Nested R1/E0 integrity once; experiments load validated `--reference-dir`
- Final full-ray exact, sample parity zero, optional metric consistency PASS
- No hypothesis / Pareto / E0 lock schema edits

## Owner review

Stop for owner. Do not merge without acceptance. Do not start estimator
iteration, E2, E3, or Gate 2B2.
