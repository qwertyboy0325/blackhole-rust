# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 0 complete, Gate 1 not started

This repository currently contains research, architecture decisions, a
validation plan, and a reproducible baseline configuration. It deliberately
contains no production renderer. Owner review is required before Gate 1.

The selected path is:

- backward null-ray integration from an observer tetrad;
- a CPU `f64` oracle using canonical Hamilton equations in horizon-penetrating
  Cartesian Kerr-Schild coordinates;
- an independent Carter-separated Boyer-Lindquist solver as a differential
  oracle, not the production integration path;
- adaptive DOP853 integration with dense-output event localization;
- a thin, equatorial, optically thick disk interface kept separate from its
  emission law;
- spectral radiance internally and scene-linear OpenEXR diagnostics at the
  artifact boundary;
- no UI or GPU dependency in the physics core.

Start with [the vision](docs/vision.md), [physical assumptions](docs/physics-assumptions.md),
[architecture](docs/architecture.md), [sources](docs/research-sources.md), and
[validation plan](docs/validation-plan.md). Decisions are recorded in
[`docs/adr`](docs/adr), and the Gate 0 configuration is
[`presets/gargantua-baseline.toml`](presets/gargantua-baseline.toml).

## Intended future interface

These commands are contracts for Gate 1 and later; they are not implemented in
Gate 0:

```bash
cargo xtask evaluate --preset gargantua-baseline
cargo xtask render --preset gargantua-baseline
cargo xtask inspect-ray --x <x> --y <y>
cargo xtask compare-renders <old.exr> <new.exr>
cargo xtask benchmark
```

The evaluation contract and threshold-calibration policy are specified in
[`docs/validation-plan.md`](docs/validation-plan.md).

## Scope boundary

Physical output and presentation output are separate products. Physics produces
ray outcomes, invariants, redshift, and radiance. Presentation may later apply
exposure, tone mapping, bloom, glare, lens flare, or artistic color decisions,
but those operations must be optional, labeled, and unable to alter scientific
diagnostic channels.
