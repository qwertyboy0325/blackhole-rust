# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 2A2 in progress (first lensed celestial diagnostic)

Gate 0–1B2, Gate 2A0-1…2A0-4, and Gate 2A1 are complete. Gate 2A2 samples a
deterministic procedural celestial field through Gate 2A1 finite-boundary
coordinates to produce the first lensed diagnostic RGB image. It does **not**
claim physical radiometry, redshift, disk emission, or asymptotic-infinity
directions. Disk omission is a separate surface-set diagnostic, not a
transparent physical disk.

See [procedural celestial texture V1](docs/procedural-celestial-texture-v1.md).

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
crates/relativity-core      # metric, coords, tetrads, ray init (no I/O)
crates/relativity-integrate # DOP853 geodesic integration + events
crates/relativity-trace     # outcomes, shading, celestial UV mapping
crates/relativity-render    # procedural celestial + lensed diagnostic RGB
xtask                       # presets, tiers, evaluate scopes
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo run --release -p xtask -- evaluate --scope gate-2a2-lensed-celestial
cargo run --release -p xtask -- \
  render-lensed-celestial \
  --preset presets/gargantua-baseline.toml \
  --tier gate \
  --surface-set horizon-escape-only \
  --mode disk-omitted-diagnostic \
  --texture procedural-coordinate-grid-v1 \
  --output-dir artifacts/manual-lensed-sky \
  --execution parallel --threads 16 \
  --require-release
```

## Scope boundary

Physical output and presentation output are separate products. Gate 2A2 produces
a diagnostic lensed celestial image from finite-boundary coordinates and a
procedural texture. It does not claim radiometry or asymptotic infinity.
