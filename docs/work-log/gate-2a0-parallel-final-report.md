# Gate 2A0-2 Final Report — Deterministic CPU Parallelism

## 1. Base / branch / authority

- Base `main`: `d4325f71f884a389fe35c4fbddfc155edcebbe69` (Gate 2A0-1 merge)
- Branch: `gate-2a0-deterministic-parallelism`
- Authoritative evaluate commit: `11d42c9ee666c06046e3d9cf16297ef05fb022d4`
- Draft PR: (opened after push)

## 2. Parallel library / pool ownership

- Dependency: `rayon` in `relativity-trace` only
- Local `ThreadPoolBuilder::new().num_threads(n).build()` + `pool.install(|| …)`
- No Rayon global pool mutation; no shared mutable numerical state

## 3–5. Scheduling / errors / reduction

- Indexed domain `index = row*width+col` via `(0..n).into_par_iter()`
- Scheduler id: `rayon-indexed-work-stealing`
- Collect `Vec<Result<RayOutcome, IntegrationError>>` then `fold_indexed_results` (first error by ascending index)
- Counts / percentiles / PPM / PGM / digests remain single-threaded on ordered `TraceBundle`

## 6. Worker metadata

- Adjacent `trace-execution.json` (`mode`, `thread_count`, `scheduler`)
- Adjacent `build-execution.json` (Gate 2A0-1)
- Evaluator reads worker reports; does not infer execution mode

## 7–12. Measured matrix (`11d42c9`)

| Quantity | Value |
|---|---|
| available / authoritative threads | 16 / 16 |
| serial 64 wall | 1.904 s |
| parallel 64 ×3 | 0.205 / 0.204 / 0.200 s |
| median parallel 64 | 0.204 s |
| parallel speedup 64 | 9.35× |
| serial 128 wall | 7.641 s |
| parallel 128 ×3 | 0.772 / 0.760 / 0.710 s |
| median parallel 128 | 0.760 s |
| parallel speedup 128 | 10.05× |
| performance status | Verified |

## 13–15. Equivalence / Gate 1B2

- serial↔parallel 64/128: byte-identical class/PPM/PGM/counts/steps/RHS/expensive-ray order
- cross-thread-count 32 (1/2/16): identical
- Gate 1B2 128 parallel: class/PPM/PGM/counts MATCH, failed=0
  - class `64462a83…52c4`
  - ppm `ac058d5a…184c`
  - pgm `2df22639…5db5`

## 16. Evaluator

- `result: PASS`
- `authoritative: true`
- `dirty: false`
- content digest: `dce8afa8c51d4f6eb43c8c1182ad9f0d332a48adb0f099baed37f418bc2765cd`

## 17–19. CI / exclusions

- Local fmt / clippy / workspace tests PASS
- No celestial-sphere, sky, SIMD, GPU, LTO, tolerance/event/physics changes
- Stopped at Gate 2A0-2 boundary for owner review
