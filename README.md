# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 2B1 in progress (diagnostic bolometric radiance)

Gate 0–1B2, Gate 2A0–2A2, and Gate 2B0 are complete. Gate 2B1 adds a frozen
diagnostic radial bolometric emission profile and invariant `I_obs = g⁴ I_em`
transport for opaque disk hits. It does **not** implement spectra, temperature,
physical RGB, or OpenEXR.

See [diagnostic bolometric emission V1](docs/diagnostic-bolometric-emission-v1.md).

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
xtask                       # presets, tiers, evaluate scopes
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo run --release -p xtask -- evaluate --scope gate-2b1-bolometric-radiance
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

Physical output and presentation output are separate products. Gate 2B1 produces
normalized bolometric specific intensity with `g⁴` transport. It does not claim
spectra, temperature, or physical RGB.
