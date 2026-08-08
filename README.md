# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 2B2 PASS (owner merge review)

Gate 0–2B2 and R1/E0 are complete at evaluated commit
`aa28e7d555d6fb0f38d64bf2d5a6d1a6bef449d4` (closure `5203577417`). Gate 2B2 is
diagnostic spectral `I_ν` transport (`g³`) on `SpectralFrame V1` — not
temperature, physical RGB, OpenEXR, GPU, wgpu, egui, or GUI. E1 research remains
`PAUSE_RESEARCH_WEDGE`. Awaiting owner merge decision on PR #17.

![Gate evolution (diagnostic channels)](docs/media/blackhole-rust-evolution.gif)

Diagnostic scientific channels only — not a beauty render. Regenerate with
`python3 scripts/build_evolution_gif.py` after new gate artifacts land; keep
this GIF linked from the README on subsequent commits.

See [diagnostic bolometric emission V1](docs/diagnostic-bolometric-emission-v1.md)
and [diagnostic spectral emission V1](docs/diagnostic-spectral-emission-v1.md).
Gate 2B2 report: [gate-2b2-final-report](docs/work-log/gate-2b2-final-report.md).

Selected path remains:

- backward null-ray integration from an observer tetrad;
- a CPU `f64` oracle using canonical Hamilton equations in horizon-penetrating
  Cartesian Kerr-Schild coordinates;
- adaptive DOP853 with dense-output event localization (ADR 0005 accepted);
- spectral radiance and OpenEXR diagnostics in later gates;
- no UI or GPU dependency in the physics core.

Start with [the vision](docs/vision.md), [physical assumptions](docs/physics-assumptions.md),
[architecture](docs/architecture.md), [celestial coordinates](docs/celestial-coordinate-convention.md),
[sources](docs/research-sources.md), and [validation plan](docs/validation-plan.md).

## Workspace

```text
crates/relativity-core      # metric, coords, tetrads, ray init, frequency kinematics
crates/relativity-integrate # DOP853 geodesic integration + events
crates/relativity-trace     # outcomes, shading, celestial UV mapping
crates/relativity-render    # procedural celestial + lensed RGB + g-factor + bolometric
crates/relativity-oracle    # OracleFrame V1, scientific digest, crops, metrics
xtask                       # presets, tiers, evaluate scopes
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo run --release -p xtask -- evaluate --scope gate-2b1-bolometric-radiance
cargo run --release -p xtask -- \
  oracle-benchmark-corpus \
  --manifest experiments/oracle-benchmark/corpus-v1.toml \
  --output-dir artifacts/r1-e0-oracle-corpus \
  --execution parallel --threads 16 \
  --require-release
cargo run --release -p xtask -- \
  render-lensed-celestial \
  --preset presets/gargantua-baseline.toml \
  --tier gate \
  --surface-set opaque-disk-horizon-escape \
  --mode opaque-disk-mask \
  --texture procedural-coordinate-grid-v1 \
  --emit-disk-frequency-shift \
  --emit-disk-bolometric-radiance \
  --output-dir artifacts/manual-bolometric-disk \
  --execution parallel --threads 16 \
  --require-release
```

## Scope boundary

Physical output and presentation output are separate products. E1 is an
experimental sampler study on the E0 corpus; it does not claim spectra,
temperature, physical RGB, formal error guarantees, or production speedup.
