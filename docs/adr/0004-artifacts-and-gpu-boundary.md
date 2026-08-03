# ADR 0004: Deterministic artifacts and a deferred GPU backend

- Status: Accepted
- Date: 2026-08-03

## Decision

Use multi-channel float OpenEXR plus canonical JSON reports and versioned TOML
presets as the future authoritative artifact set. Keep presentation exports
non-authoritative.

Defer GPU implementation. A future backend must return the same typed outcomes
and diagnostics as the CPU path and prove its precision envelope ray-by-ray.
Portable WGSL `f32`, mixed precision, and native GPU `f64` are candidates, not
assumptions.

## Consequences

Artifacts are larger than PNGs but preserve HDR values and scientific channels.
Bitwise determinism is required only for a pinned CPU environment; cross-platform
validation is numerical and classification-aware. Critical-curve rays outside a
GPU's demonstrated envelope must fall back to CPU or be marked unresolved.

## Rejected alternatives

- GPU-first architecture: performance before a reference would make precision
  failures hard to diagnose;
- PNG-only regressions: tone mapping destroys physical evidence;
- a wgpu dependency in the physics core: reverses the intended dependency
  direction and mistakes one backend for the model.
