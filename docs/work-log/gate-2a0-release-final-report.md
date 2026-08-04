# Gate 2A0-1 Final Report — Release Execution Foundation

## Status

Pending authoritative `cargo run --release -p xtask -- evaluate --scope gate-2a0-release` on a clean tip. Fill measured timings after that run.

## 1. Branch and base

- Base `main`: `286edce06d4234d640fa1b96674c793104b18d66` (Gate 1B2 merge)
- Branch: `gate-2a0-release-execution`
- Draft PR: (open after push)

## 2. Build metadata mechanism

- `xtask/build.rs` emits `BH_CARGO_PROFILE`, `BH_OPT_LEVEL`, `BH_DEBUG`, `BH_TARGET`, `BH_TOOLCHAIN`
- `BuildExecutionMetadata::current()` + `is_optimized_release_execution()`
- Authoritative condition: `cargo_profile == "release" && !debug_assertions && opt_level != "0"`

## 3. Release guard

- `trace-outcome-map --require-release` fails before tracing when not release
- Error text includes observed profile fields
- No partial PPM/PGM/JSON on rejection
- Flag optional (Gate 1B2 commands unchanged)

## 4–9. Benchmark matrix (fill after evaluate)

| Run | Wall (s) | Rays/s | Notes |
|---|---|---|---|
| release 32×32 | TBD | TBD | smoke |
| dev 64×64 | TBD | TBD | |
| release 64×64 ×3 | TBD / TBD / TBD | TBD | median TBD |
| speedup vs dev | TBD | | evidence only |
| release 128×128 | TBD | TBD | vs Gate 1B2 refs |

Historical Gate 1B2 debug 128×128 ≈ 210 s (prior run; not remeasured).

## 10–11. Gate 1B2 reference comparison

Required matches: classification, PPM, counts, failed=0.

PGM: reported explicitly (`MATCH` or `MISMATCH_REPORTED`); difference alone is not a categorical failure.

## 12–14. Determinism / digest / authority

- 64×64 release ×3: class/PPM/PGM/counts/steps/RHS/expensive-ray order identical
- Timing/speedup excluded from `content_digest_excluding_digest_field`
- Authoritative commit: TBD

## 15. CI

- fmt / clippy / workspace tests (existing workflow)
- Full Gate 2A0 evaluate is owner/local release path (too heavy for default CI job)

## 16. Exclusions confirmed

No Rayon, thread pools, parallel iterators, SIMD, GPU, wgpu, egui, LTO, target-cpu, fast-math, celestial-sphere, disk appearance, physics/tolerance/event-semantic changes.
