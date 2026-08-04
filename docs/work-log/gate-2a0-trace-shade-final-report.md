# Gate 2A0-3 Final Report — Trace-Once / Shade-Many

## Status

Authoritative `evaluate --scope gate-2a0-trace-shade` **PASS** at tip `7e26d2b`.

## 1. Base / branch

- Base: `85d11379705914c1cfbea657d386b82b142dd3e0` (Gate 2A0-2 merge)
- Branch: `gate-2a0-trace-shade-separation`
- Implementation tip: `7e26d2b1df0a775f163e65d052ce6d6d2c79d0c4`

## 2. Module / type layout

- `crates/relativity-trace/src/shade.rs`
  - `RgbFrame` (validated length)
  - `DiagnosticShadeStyle::{Gate1b2Categorical, DiskSuppressed}`
  - `shade_trace_bundle`, `shade_diagnostic`, `shade_many`, `ShadedFrame`
- `crates/relativity-trace/src/image.rs`
  - `encode_ppm(&RgbFrame)`
  - `write_outcome_ppm` = compatibility wrapper over legacy categorical shade
  - `write_rhs_pgm` remains trace-data-derived
- `crates/relativity-trace/src/trace_digest.rs`
  - `trace_data_digest(&TraceBundle)` — shading-independent
- `xtask/src/trace_shade_many.rs` — one-trace / many-shade worker + `TraceShadeReport`
- `xtask/src/evaluate_gate2a0_trace_shade.rs` — scope `gate-2a0-trace-shade`

## 3. Compatibility

- `trace_grid` / `trace_grid_with_execution` unchanged
- `TraceBundle` remains canonical; docs state it holds outcomes, not display colors, and may be shaded repeatedly
- `write_outcome_ppm` stays byte-identical via single categorical mapping (no duplicated legend)

## 4. Trace-data digest

- Row-major per-pixel: outcome class, λ, state `[f64;8]` via `to_bits()`, accepted/rejected steps, RHS count, outcome-specific metadata
- Includes width/height/pixel index
- Excludes shading style, RGB, PPM, wall-clock, paths
- Verified: both styles share digest `b2c60252aea519866370774d97a8d8c1b9c7d626d3429fc2a1ae4b57a0f691a9` at 128×128

## 5. Proof tracing runs once

Worker report fields (not inferred from filenames):

| Run | `trace_invocations` | `shade_passes` |
|---|---:|---:|
| smoke 32×32 | 1 | 2 |
| auth-128 run-0 | 1 | 2 |
| auth-128 run-1 | 1 | 2 |

Code path: resolve execution → `trace_grid_with_execution` once → finite check → digest → PGM → `shade_many` loop → encode PPMs.

## 6. Styles / order

Requested and recorded order:

1. `gate1b2-categorical`
2. `disk-suppressed`

CLI rejects duplicate styles. Disk-suppressed is diagnostic-only (DiskHit → black); not a physical shadow.

## 7. Timing (informational; excluded from digests)

| Run | Trace (s) | Shade (s) |
|---|---:|---:|
| smoke 32×32 | 0.285 | 0.000023 |
| auth-128 run-0 | 0.782 | 0.000229 |
| auth-128 run-1 | 0.729 | 0.000221 |

`trace >> shade` observed.

## 8. Legacy Gate 1B2 reference (128×128)

| Channel | Digest / value | Status |
|---|---|---|
| classification | `64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4` | MATCH |
| legacy PPM | `ac058d5af01b425e411b5c33017210bf888aa52918cfd085bb863d7ddc99184c` | MATCH |
| RHS PGM | `2df226390057bb87b64d172cd258087b0ef4c1ad0ce0d4378e003b5861a75db5` | MATCH |
| counts | disk 12307 / escaped 2442 / horizon_event 1462 / horizon_approach 173 / affine 0 / failed 0 | MATCH |

## 9. Disk-suppressed differential

- Changed pixels: **12307** (= `disk_hit`)
- Every non-`DiskHit` pixel byte-identical
- Disk-suppressed PPM digest: `1c98a08a5d5018cb80fb6df85d281bf6ff2cc6f537b0805ddaacfffc0bf23f58`

## 10. Subprocess determinism

Two independent 128×128 parallel (16-thread) subprocesses: identical `trace_data_digest`, class, PPM (both styles), PGM, counts, step/RHS totals.

## 11. Authoritative evaluator digest

- `result: PASS`
- `authoritative: true`
- `dirty: false`
- commit: `7e26d2b1df0a775f163e65d052ce6d6d2c79d0c4`
- content digest: `82e537b68a9e6e878409ec8444339864d13b6810318f08ba42d3af40a837a1b3`
- `authoritative_threads: 16`

## 12–13. CI / authority

- Local fmt / clippy (`-D warnings`) / workspace tests PASS (evaluator)
- Release authority + clean worktree held

## 14. Exclusions confirmed

Celestial-sphere mapping, escape-direction sky, star fields, physical disk emission, redshift/Doppler/beaming, radiative transfer, PNG/EXR/HDR, GPU/wgpu/egui, integrator/event/metric/camera/tolerance changes — **not started**.

Stop at Gate 2A0-3 boundary for owner review.
