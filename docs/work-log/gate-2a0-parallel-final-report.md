# Gate 2A0-2 Final Report — Deterministic CPU Parallelism

## Status

Pending authoritative `cargo run --release -p xtask -- evaluate --scope gate-2a0-parallel` on a clean tip.

## 1. Base / branch

- Base `main`: `d4325f71f884a389fe35c4fbddfc155edcebbe69` (Gate 2A0-1 merge)
- Branch: `gate-2a0-deterministic-parallelism`

## 2–6. Design (filled after evaluate)

- Library: `rayon` local `ThreadPoolBuilder` (no global pool mutation)
- Scheduling: indexed `0..n` work-stealing (`rayon-indexed-work-stealing`)
- Error selection: ordered `Vec<Result<…>>` then first error by pixel index
- Reductions: serial after collect (`TraceBundle` → reports/PPM/PGM)
- Worker metadata: adjacent `trace-execution.json` + `build-execution.json`

## 7–15. Measured results

TBD after release evaluate.

## 16. Exclusions

No celestial-sphere, sky, SIMD, GPU, LTO, tolerance/event/physics changes.
