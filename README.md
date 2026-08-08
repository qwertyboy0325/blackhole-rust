# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 2C0 authoritative PASS after 5225301622 fix (pending merge)

Gate 0–2B2 and R1/E0 are complete on `main` (Gate 2B2 merge `95c4062`, PR #17).
Gate 2C0 physical Page–Thorne emission re-evaluated **PASS** @ `a760427` after
closure `5225301622` (prior `551f69e` PASS invalidated). Gate 2C1
(CIE/RGB/OpenEXR), GPU, and GUI are not authorized. E1 research remains
`PAUSE_RESEARCH_WEDGE`.

![Gate evolution (diagnostic channels)](docs/media/blackhole-rust-evolution.gif)

Diagnostic scientific channels only — not a beauty render. Regenerate with
`python3 scripts/build_evolution_gif.py` after new gate artifacts land; keep
this GIF linked from the README on subsequent commits.

See [diagnostic bolometric emission V1](docs/diagnostic-bolometric-emission-v1.md),
[diagnostic spectral emission V1](docs/diagnostic-spectral-emission-v1.md), and
[physical disk emission V1](docs/physical-disk-emission-v1.md).
Gate 2B2 report: [gate-2b2-final-report](docs/work-log/gate-2b2-final-report.md).
Gate 2C0 report: [gate-2c0-final-report](docs/work-log/gate-2c0-final-report.md).

Selected path remains:

- backward null-ray integration from an observer tetrad;
- a CPU `f64` oracle using canonical Hamilton equations in horizon-penetrating
  Cartesian Kerr-Schild coordinates;
- adaptive DOP853 with dense-output event localization (ADR 0005 accepted);
- physical radiometry (2C0) before colorimetry/OpenEXR (2C1);
- no UI or GPU dependency in the physics core.

Start with [the vision](docs/vision.md), [physical assumptions](docs/physics-assumptions.md),
[architecture](docs/architecture.md), [celestial coordinates](docs/celestial-coordinate-convention.md),
[sources](docs/research-sources.md), and [validation plan](docs/validation-plan.md).

## Workspace

```text
crates/relativity-core      # metric, coords, tetrads, ray init, frequency, physical units
crates/relativity-integrate # DOP853 geodesic integration + events
crates/relativity-trace     # outcomes, shading, celestial UV mapping
crates/relativity-render    # diagnostic + physical disk emission / spectral frames
crates/relativity-oracle    # OracleFrame V1, scientific digest, crops, metrics
xtask                       # presets, tiers, evaluate scopes
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo run --release -p xtask -- evaluate --scope gate-2b2-spectral-transport
cargo run --release -p xtask -- evaluate --scope gate-2c0-physical-emission
cargo run --release -p xtask -- \
  render-physical-disk-spectrum \
  --preset presets/gargantua-physical-v1.toml \
  --tier gate \
  --physical-emission page-thorne-blackbody-v1 \
  --physical-spectral-grid physical-spectral-grid-v1 \
  --output-dir artifacts/gate-2c0-physical-emission \
  --execution parallel --threads 16 \
  --require-release
```

## Scope boundary

Physical output and presentation output are separate products. E1 is an
experimental sampler study on the E0 corpus; it does not claim spectra,
temperature, physical RGB, formal error guarantees, or production speedup.
