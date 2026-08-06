# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 2B0 in progress (frequency-shift kinematics)

Gate 0–1B2, Gate 2A0–2A2 are complete. Gate 2B0 establishes measured-frequency
kinematics for opaque thin-disk hits (`g = ν_obs/ν_em`) with circular equatorial
emitter velocity. It does **not** implement emission, intensity transport,
spectra, physical RGB, or OpenEXR.

See [frequency-shift convention V1](docs/frequency-shift-convention-v1.md).

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
crates/relativity-render    # procedural celestial + lensed RGB + disk g-factor
xtask                       # presets, tiers, evaluate scopes
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo run --release -p xtask -- evaluate --scope gate-2b0-frequency-shift
cargo run --release -p xtask -- \
  render-lensed-celestial \
  --preset presets/gargantua-baseline.toml \
  --tier gate \
  --surface-set opaque-disk-horizon-escape \
  --mode opaque-disk-mask \
  --texture procedural-coordinate-grid-v1 \
  --emit-disk-frequency-shift \
  --output-dir artifacts/manual-frequency-shift \
  --execution parallel --threads 16 \
  --require-release
```

## Scope boundary

Physical output and presentation output are separate products. Gate 2B0 produces
frequency-ratio kinematics for opaque disk hits. It does not claim disk
brightness, spectral transport, or physical radiometry.
