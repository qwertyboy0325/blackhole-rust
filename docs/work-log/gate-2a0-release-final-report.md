# Gate 2A0-1 Final Report — Release Execution Foundation

## 1. Branch, commits, base

- Base `main`: `286edce06d4234d640fa1b96674c793104b18d66` (Gate 1B2 merge)
- Branch: `gate-2a0-release-execution`
- Implementation tip evaluated: `d8e04af`
- Draft PR: (opened after push)

## 2. Build metadata mechanism

- `xtask/build.rs` emits `BH_CARGO_PROFILE`, `BH_OPT_LEVEL`, `BH_DEBUG`, `BH_TARGET`, `BH_TOOLCHAIN`
- `BuildExecutionMetadata::current()` + `is_optimized_release_execution()`
- Authoritative condition: `cargo_profile == "release" && !debug_assertions && opt_level != "0"`
- Observed evaluator build: `release` / opt `3` / `debug_assertions=false` / `aarch64-apple-darwin`

## 3. Release guard

- `trace-outcome-map --require-release` fails before tracing on debug
- Observed: `cargo_profile=debug opt_level=0 debug_assertions=true`
- No partial PPM/PGM/JSON written
- Flag optional (Gate 1B2 commands unchanged)

## 4–9. Measured benchmark matrix

| Run | Wall (s) | Rays/s |
|---|---:|---:|
| release 32×32 smoke | 0.475 | — |
| dev 64×64 | 53.610 | 76.4 |
| release 64×64 run-0 | 1.924 | 2128.6 |
| release 64×64 run-1 | 1.940 | 2111.8 |
| release 64×64 run-2 | 1.917 | 2136.5 |
| median release 64×64 | 1.924 | — |
| **speedup vs dev** | **27.86×** | evidence only |
| release 128×128 | 7.586 | 2159.7 |

Historical Gate 1B2 debug 128×128 ≈ 210 s (prior run; not remeasured here).

## 10–11. Gate 1B2 reference comparison (128×128 release)

| Channel | Status |
|---|---|
| classification | MATCH `64462a83…52c4` |
| PPM | MATCH `ac058d5a…184c` |
| PGM | MATCH `2df22639…5db5` |
| counts | MATCH disk 12307 / escaped 2442 / horizon_event 1462 / horizon_approach 173 / affine 0 / failed 0 |

## 12. Release determinism (64×64 ×3)

Exact equality of classification, PPM, PGM, counts, accepted/rejected steps, RHS totals, expensive-ray ordering. PASS.

Dev vs release 64×64 classifications/PPM/counts agree; PGM identical across profiles on this host.

## 13–14. Evaluator digest / authority

- `result: PASS`
- `authoritative: true`
- `dirty: false`
- commit: `d8e04afeeefa3e813f6272f96544e5ba8d5ecf53`
- content digest: `7cfc798d6f44ee8a6749d2d4aec8162f6a9b70faf59f38bc124cad6f197d147b`

## 15. CI

- Local: `fmt` / `clippy -D warnings` / `cargo test --workspace --all-features` PASS
- Full Gate 2A0 evaluate remains owner/local release path (heavy serial map matrix)

## 16. Exclusions confirmed

No Rayon, thread pools, parallel iterators, SIMD, GPU, wgpu, egui, LTO, target-cpu, fast-math, celestial-sphere, disk appearance, physics/tolerance/event-semantic changes.
