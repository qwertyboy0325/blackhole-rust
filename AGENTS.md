# Repository operating rules

## Current gate

Gate 0, 1A, 1B0–1B2, Gate 2A0–2A2, Gate 2B0, Gate 2B1, and Gate 2B2 are complete
(Gate 2B2 merged to `main` via PR #17 @ `95c4062`).
R1/E0 is complete.
E1 physics-aware adaptive quadtree sampling is merged (PR #15).
E1 execution-performance restructuring: authoritative evaluate PASS after
owner closure `5202437486` on PR #16; pending owner merge review.

E1 is experimental:
- preserve the CPU f64 oracle;
- prevent oracle-data leakage into scheduling;
- report ray/error curves and failures;
- do not require permanent schema governance for prototype internals.
- research recommendation remains `PAUSE_RESEARCH_WEDGE` until owner authorizes estimator iteration.

**Gate 2C0** physical thin-disk emission: prior authoritative PASS @ `551f69e`
**INVALIDATED** (closure `5225301622` — Page–Thorne `F∝Q` missing `1/(B√C)` plus
non-independent numerical oracle). Root-fix in progress on
`gate-2c0-physical-emission`. Gate 2C1 is **not** authorized. Do not begin E2
ray differentials, E3 ray bundles, GPU, wgpu, egui or GUI work.

When a gate or research package produces new reviewable image artifacts,
regenerate `docs/media/blackhole-rust-evolution.gif` and keep it embedded in
`README.md` on the delivery commits.

## Authority and scope

- Modify only this repository.
- Do not push, publish, release, rewrite Git history, or merge protected branches.
- Do not claim exact reproduction of *Interstellar* or DNGR production assets.
- Prefer primary papers, official specifications, textbooks, and original
  repositories. Add every material source and its access/license note to
  `docs/research-sources.md`.
- Keep physical assumptions, numerical choices, and artistic choices visibly
  distinct.
- Never weaken checks or tolerances merely to obtain a passing result.

## Architecture invariants

- `relativity-core` (when created) must not depend on egui, wgpu, an image
  encoder, or a platform windowing crate.
- Coordinates, metrics, integrators, emitters, cameras, and artifact writers are
  separate abstractions. Render frontends depend inward; the physics core never
  depends outward.
- The CPU `f64` implementation is the reference. A GPU result is acceptable only
  inside a measured error envelope against that oracle.
- Every ray has exactly one typed termination result. Numerical failures are
  data, never silently painted as the horizon or sky.
- Diagnostic artifacts are deterministic for a pinned toolchain, preset, and
  backend. Reports include toolchain, target, feature flags, preset digest, and
  artifact digests.
- Scientific channels are written before any optional presentation transform.

## Required workflow for future gates

1. State the gate and acceptance criteria.
2. Add or update an ADR before changing a settled architecture boundary.
3. Implement the smallest reviewable slice.
4. Run format, lint, unit, integration, property, limiting-case, convergence,
   differential, and artifact checks appropriate to the slice.
5. Inspect the worst rays and pixels, not only aggregate scores.
6. Preserve failing artifacts and diagnostics long enough for review.
7. Stop at the gate boundary for owner review.

## Definition of evidence

Compilation and visual plausibility are necessary but insufficient. A result
requires reproducible configuration, numerical diagnostics, tests, reviewable
artifacts, attributable equations, explicit assumptions, and known limitations.
