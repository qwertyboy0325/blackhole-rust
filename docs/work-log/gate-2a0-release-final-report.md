# Gate 2A0-1 Final Report — Release Execution Foundation

## Status

Evidence closure complete at authoritative tip `b52a274`.

## 1. Branch, commits, base

- Base `main`: `286edce06d4234d640fa1b96674c793104b18d66` (Gate 1B2 merge)
- Branch: `gate-2a0-release-execution`
- Authoritative tip: `b52a2743c21101731bde248a5044a28b392145a2`
- Draft PR: #5

## 2. Build metadata mechanism

- `xtask/build.rs` emits `BH_CARGO_PROFILE`, `BH_OPT_LEVEL`, `BH_DEBUG`, `BH_TARGET`, `BH_TOOLCHAIN`
- `BuildExecutionMetadata::current()` + `is_optimized_release_execution()`
- Authoritative condition: `cargo_profile == "release" && !debug_assertions && opt_level != "0"`
- Worker path: adjacent `build-execution.json` written by `trace-outcome-map`, read by evaluator (not inferred)

Observed worker metadata:

- release workers: `release` / opt `3` / `debug_assertions=false`
- dev worker: `debug` / opt `0` / `debug_assertions=true`

## 3. Release guard

- `trace-outcome-map --require-release` fails before tracing on debug
- No partial PPM/PGM/JSON/`build-execution.json` on rejection

## 4. Digest projection (evidence closure)

- `DigestProjection` includes check **name+status only** (excludes `Check.detail`)
- Timing fields on `BenchmarkRun`, median/speedup top-level fields excluded
- Reference comparison historical wall-clock/note excluded from digest
- Tests cover timing-bearing `Check.detail` invariance

## 5–10. Measured benchmark matrix (`b52a274`)

| Run | Wall (s) | Rays/s |
|---|---:|---:|
| release 32×32 smoke | — | — |
| dev 64×64 | 53.657 | — |
| release 64×64 ×3 | 1.929 / 1.913 / 1.945 | — |
| median release 64×64 | 1.929 | — |
| **speedup vs dev** | **27.82×** | evidence only |
| release 128×128 | 7.687 | 2131.5 |

Historical Gate 1B2 debug 128×128 ≈ 210 s (prior run; not remeasured).

## 11. Gate 1B2 reference comparison (128×128 release)

| Channel | Status |
|---|---|
| classification | MATCH `64462a83…52c4` |
| PPM | MATCH `ac058d5a…184c` |
| PGM | MATCH `2df22639…5db5` |
| counts | MATCH (failed=0) |

## 12. Release determinism (64×64 ×3)

PASS (class/PPM/PGM/counts/steps/RHS/expensive-ray order).

## 13–14. Evaluator digest / authority

- `result: PASS`
- `authoritative: true`
- `dirty: false`
- commit: `b52a2743c21101731bde248a5044a28b392145a2`
- content digest: `8cf69d48f7e2d027e0981d6f28fab2009e3f5f2af8743ab81eb269a3a82e5673`

## 15. CI / exclusions

- Local fmt / clippy / workspace tests PASS
- No Rayon, parallelism, sky rendering, physics/tolerance/event changes
