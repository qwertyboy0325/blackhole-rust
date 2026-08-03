# Gargantua Relativistic Renderer

An offline-first Rust project for scientifically defensible images of light in a
Kerr spacetime. The visual target is inspired by the phenomena shown around
Gargantua in *Interstellar*, not an exact reconstruction of Double Negative's
proprietary assets, renderer, grading, camera, or undocumented production
parameters.

## Status: Gate 1A complete (geometry kernel); Gate 1B not started

Gate 0 architecture is accepted. Gate 1A implements and verifies the Cartesian
Kerr–Schild geometry kernel, Hamiltonian RHS evaluation at a state, ZAMO
tetrads, and null-ray initialization. It does **not** integrate geodesics or
render images.

Selected path remains:

- backward null-ray integration from an observer tetrad;
- a CPU `f64` oracle using canonical Hamilton equations in horizon-penetrating
  Cartesian Kerr-Schild coordinates;
- adaptive DOP853 with dense-output event localization (dependency **Proposed**
  in ADR 0005; not linked in Gate 1A);
- spectral radiance and OpenEXR diagnostics in later gates;
- no UI or GPU dependency in the physics core.

Start with [the vision](docs/vision.md), [physical assumptions](docs/physics-assumptions.md),
[architecture](docs/architecture.md), [sources](docs/research-sources.md), and
[validation plan](docs/validation-plan.md).

## Workspace

```text
crates/relativity-core   # metric, coords, tetrads, ray init (no I/O)
xtask                    # inspect-point, inspect-initial-ray, evaluate
```

License: `MIT OR Apache-2.0`.

## Commands

```bash
cargo xtask inspect-point --mass 1 --spin 0.999 --x 4 --y 1 --z 2
cargo xtask inspect-initial-ray \
  --preset presets/gargantua-baseline.toml \
  --sensor-x 0 --sensor-y 0
cargo xtask evaluate \
  --preset presets/gargantua-baseline.toml \
  --scope gate-1a
```

Alias via Cargo:

```bash
cargo run -p xtask -- evaluate --preset presets/gargantua-baseline.toml --scope gate-1a
```

## Scope boundary

Physical output and presentation output are separate products. Gate 1A stops at
initialized null rays and typed geometry diagnostics.
