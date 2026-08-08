# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 2D0 authoritative PASS (pending merge)

Gate 0–2C1 and R1/E0 are complete on `main` (Gate 2C1 merge `c964c74`, PR #19).
Gate 2D0 cinematic presentation evaluated **PASS** @ `e1272c5` after A1–A4
owner amendments (`presentation_frame_digest` `f8e10323…`; eval content
`be19fad2…`). Inherited Gate 2C1 `physical_color_digest` `16663188…` exact.
GPU and GUI remain deferred. E1 research remains `PAUSE_RESEARCH_WEDGE`.

![Gate evolution (diagnostic + presentation)](docs/media/blackhole-rust-evolution.gif)

Scientific channels remain authoritative; Gate 2D0 adds a display-referred
beauty PNG. Regenerate with `python3 scripts/build_evolution_gif.py` after new
gate artifacts land; keep this GIF linked from the README on subsequent commits.

See [diagnostic bolometric emission V1](docs/diagnostic-bolometric-emission-v1.md),
[diagnostic spectral emission V1](docs/diagnostic-spectral-emission-v1.md),
[physical disk emission V1](docs/physical-disk-emission-v1.md),
[physical colorimetry V1](docs/physical-colorimetry-v1.md), and
[presentation pipeline V1](docs/presentation-pipeline-v1.md).
Gate 2C1 report: [gate-2c1-final-report](docs/work-log/gate-2c1-final-report.md).
Gate 2D0 report: [gate-2d0-final-report](docs/work-log/gate-2d0-final-report.md).

Selected path remains:

- backward null-ray integration from an observer tetrad;
- a CPU `f64` oracle using canonical Hamilton equations in horizon-penetrating
  Cartesian Kerr-Schild coordinates;
- adaptive DOP853 with dense-output event localization (ADR 0005 accepted);
- physical radiometry (2C0) and colorimetry/OpenEXR (2C1);
- presentation-only SDR beauty (2D0);
- no UI or GPU dependency in the physics core.

Start with [the vision](docs/vision.md), [physical assumptions](docs/physics-assumptions.md),
[architecture](docs/architecture.md), [celestial coordinates](docs/celestial-coordinate-convention.md),
[sources](docs/research-sources.md), and [validation plan](docs/validation-plan.md).

## Workspace

```text
crates/relativity-core      # metric, coords, tetrads, ray init, frequency, physical units
crates/relativity-integrate # DOP853 geodesic integration + events
crates/relativity-trace     # outcomes, shading, celestial UV mapping
crates/relativity-render    # diagnostic + physical emission / color / presentation math
crates/relativity-oracle    # OracleFrame V1, scientific digest, crops, metrics
xtask                       # presets, tiers, evaluate scopes, PNG encode
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo run --release -p xtask -- evaluate --scope gate-2c1-colorimetry
cargo run --release -p xtask -- evaluate --scope gate-2d0-presentation
cargo run --release -p xtask -- \
  render-presentation \
  --preset presets/gargantua-physical-v1.toml \
  --presentation presets/presentation/gargantua-cinematic-v1.toml \
  --tier gate \
  --output-dir artifacts/gate-2d0-presentation \
  --execution parallel --threads 16 \
  --require-release
```

## Scope boundary

Physical output and presentation output are separate products. E1 is an
experimental sampler study on the E0 corpus; it does not claim spectra,
temperature, physical RGB, formal error guarantees, or production speedup.
