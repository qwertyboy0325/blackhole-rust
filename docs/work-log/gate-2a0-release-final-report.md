# Gate 2A0-1 Final Report — Release Execution Foundation

## 1. Branch, commits, base

- Base `main`: `286edce06d4234d640fa1b96674c793104b18d66` (Gate 1B2 merge)
- Branch: `gate-2a0-release-execution`
- Prior measured implementation: `d8e04af`
- Tip docs commit (re-evaluated for authority): see HEAD after this file lands

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

Numbers from tip evaluate at `39ae37e` (docs commit that recorded `d8e04af`); final tip re-evaluate follows this report update.

| Run | Wall (s) | Rays/s |
|---|---:|---:|
| release 32×32 smoke | 0.470 | — |
| dev 64×64 | 53.883 | 76.0 |
| release 64×64 run-0 | 1.889 | 2167.9 |
| release 64×64 run-1 | 1.891 | 2165.6 |
| release 64×64 run-2 | 1.890 | 2167.3 |
| median release 64×64 | 1.890 | — |
| **speedup vs dev** | **28.51×** | evidence only |
| release 128×128 | 7.547 | 2170.9 |

Historical Gate 1B2 debug 128×128 ≈ 210 s (prior run; not remeasured here).

## 10–11. Gate 1B2 reference comparison (128×128 release)

| Channel | Status |
|---|---|
| classification | MATCH `64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4` |
| PPM | MATCH `ac058d5af01b425e411b5c33017210bf888aa52918cfd085bb863d7ddc99184c` |
| PGM | MATCH `2df226390057bb87b64d172cd258087b0ef4c1ad0ce0d4378e003b5861a75db5` |
| counts | MATCH disk 12307 / escaped 2442 / horizon_event 1462 / horizon_approach 173 / affine 0 / failed 0 |

## 12. Release determinism (64×64 ×3)

Exact equality of classification, PPM, PGM, counts, accepted/rejected steps, RHS totals, expensive-ray ordering. PASS.

Dev vs release 64×64 classifications/PPM/counts agree; PGM identical across profiles on this host.

## 13–14. Evaluator digest / authority

At `39ae37e`:

- `result: PASS`
- `authoritative: true`
- `dirty: false`
- content digest: `84afcc67f0a0b36fed9986aa07deab2aa0c93b0aa8a8084499d528a9a7b7111c`

Final tip digest after this docs commit is filled by the subsequent release evaluate (must remain PASS / authoritative).

## 15. CI

- Local: `fmt` / `clippy -D warnings` / `cargo test --workspace --all-features` PASS
- Full Gate 2A0 evaluate remains owner/local release path (heavy serial map matrix)

## 16. Exclusions confirmed

No Rayon, thread pools, parallel iterators, SIMD, GPU, wgpu, egui, LTO, target-cpu, fast-math, celestial-sphere, disk appearance, physics/tolerance/event-semantic changes.
