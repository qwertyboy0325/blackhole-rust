# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 2A1 in progress (finite celestial-boundary coordinates)

Gate 0–1B2 and Gate 2A0-1…2A0-4 are complete. Gate 2A1 maps escaped rays to
deterministic spherical Kerr–Schild UV coordinates on the finite diagnostic
escape boundary. It does **not** sample celestial textures, compute radiometry,
or apply asymptotic-infinity corrections.

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
xtask                       # presets, tiers, evaluate scopes
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo run -p xtask -- evaluate --scope gate-2a1-celestial-directions
cargo run --release -p xtask -- \
  trace-shade-many \
  --preset presets/gargantua-baseline.toml \
  --tier gate \
  --output-dir artifacts/manual-gate \
  --execution parallel --threads 16 \
  --style gate1b2-categorical --style disk-suppressed \
  --emit-celestial-coordinates --require-release
```

## Scope boundary

Physical output and presentation output are separate products. Gate 2A1 stops at
finite-boundary celestial coordinates and a non-authoritative UV diagnostic image.

