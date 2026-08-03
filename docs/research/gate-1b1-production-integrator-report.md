# Gate 1B1 — Production DOP853 Adapter Report

## Scope

Production crate `relativity-integrate`: project-owned geodesic integration API
over exact-pinned `ivp = "=0.6.0"`. No disk, radiometry, image, GPU, or GUI.

## Decisions (owner)

- Gate 1B0 PASS; ADR 0005 Accepted
- Dependency: `ivp = "=0.6.0"`
- Event localization: project-owned on accepted-step `StepInterpolant`
- Caller-authoritative event = localized state; raw solver stop retained
- No Hamiltonian projection; no tangent-event claim

## Public API

- `GeodesicState`, `AffineParameter`, `Dop853Config`
- `integrate(params, y0, config, surfaces) -> IntegrationReport`
- `IntegrationOutcome::{Event, AffineLimit}`
- `EventSurface` + `OuterHorizon` + `EscapeSphere`
- `IntegrationError` (no `ivp` types)

Vector ordering: `[t, x, y, z, p_t, p_x, p_y, p_z]`.

## Tolerance model

Direct `ivp::Tolerance::Vector` for per-component rtol/atol. Position and
momentum absolute tolerances independently configurable. No state rescaling.

## Event kernel

Sign-changing surfaces with crossing-direction filters. Deterministic safeguarded
bisection on the current accepted-step interpolant. Earliest λ wins.

**Horizon f64 note:** Cartesian KS adaptive steps stall at `r → r₊⁺` (step-size
underflow) before a representable interior sample. Endpoint/stall capture within
`event_value_tolerance` still uses the physical surface `f = r − r₊` (not a
safety-offset policy).

## Exclusions

Tangent contact; identical-sign endpoints outside value tolerance; discontinuous
event functions; disk intersection; rendering.

## Commands

```bash
cargo xtask integrate-ray --preset presets/gargantua-baseline.toml --sensor-x 0 --sensor-y 0 --affine-limit 100
cargo xtask evaluate --scope gate-1b1
```
