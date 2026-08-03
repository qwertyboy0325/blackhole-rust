# DOP853 Rust dependency audit (Gate 1A)

**Status:** research only — no production ODE dependency added in Gate 1A.  
**Date:** 2026-08-03  
**Related ADR:** `docs/adr/0005-dop853-dependency.md` (Proposed)

## Contract required by Gate 0 / ADR 0002

- Adaptive DOP853 (order 8(5,3)) in Rust `f64`
- Component-scaled absolute/relative tolerances (position vs momentum)
- Dense output suitable for event localization (order-7 continuous extension)
- Accepted-step callbacks / solout-style inspection
- Access to step statistics (accepted/rejected, last step, nfcn)
- Ability to impose geometry-specific maximum-step guards
- Deterministic behavior on a pinned toolchain
- Compatible license with `MIT OR Apache-2.0`
- Prefer `forbid(unsafe_code)` in our integrator crate, or audited unsafe

## Candidates

### 1. `ode_solvers` 0.6.2

| Criterion | Finding |
|---|---|
| License | Apache-2.0 ([crates.io](https://crates.io/crates/ode_solvers)) |
| Provenance | [srenevey/ode-solvers](https://github.com/srenevey/ode-solvers); Hairer-style DOP853 |
| Maintenance | Active; 0.6.2 published 2026-06-07; long history since 2018 |
| Unsafe | Depends on `nalgebra`; crate itself is mostly safe Rust (verify at pin) |
| Vector/component tolerances | Scalar `rtol`/`atol` in constructors; per-component scales need wrapper/state scaling |
| Accepted-step callback | `System::solout` after successful steps; can halt |
| Dense output | DOP853 dense output order 7; continuous output model APIs present for Dopri5 and related paths — confirm Dop853 continuous-output parity at pin |
| Event localization | No first-class event API; suitable as dense-output primitive under our bracket/root layer |
| Determinism | Pure `f64` RK; deterministic given identical steps/RHS |
| Stats | Exposes integration result / step bookkeeping via solver state |
| `h_max` / domain guards | `from_param` exposes `h_max`, `n_max`, safety factors |
| API stability | 0.x; nalgebra coupling; adaptation cost moderate |
| Fit | **Best direct DOP853 match** among surveyed crates |

### 2. `ivp` 0.6.0

| Criterion | Finding |
|---|---|
| License | Apache-2.0 ([crates.io](https://crates.io/crates/ivp)) |
| Provenance | [Ryan-D-Gast/ivp](https://github.com/Ryan-D-Gast/ivp); SciPy-like `solve_ivp` port |
| Maintenance | Young (2025–2026) but actively versioned; DOP853 listed |
| Unsafe | Pure-Rust intent; audit at pin |
| Tolerances | Builder `rtol`/`atol`; vector atol support should be verified against our 8-component state |
| Callbacks / dense output | Dense output advertised for DOP853/DOPRI5 |
| Events | SciPy-style events evolving; symplectic path currently lacks events |
| `h_max` | Likely via builder; confirm at pin |
| API stability | Early 0.x; lower download/ecosystem mileage than `ode_solvers` |
| Fit | Strong feature surface; higher adoption risk for Gate 1B |

### 3. `diffsol` (MIT)

| Criterion | Finding |
|---|---|
| License | MIT ([crates.io](https://crates.io/crates/diffsol)) |
| Provenance | [martinjrobins/diffsol](https://github.com/martinjrobins/diffsol); JOSS 2026 |
| Maintenance | Active scientific ODE/DAE library |
| Methods | BDF, SDIRK/ESDIRK, ERK (TSIT45), etc. — **no DOP853** |
| Dense output / events | Yes (interpolation + event stop) |
| Fit | Excellent general ODE toolkit, **wrong method family** for ADR 0002’s DOP853 oracle |

## Recommendation (Proposed, not adopted)

1. Prefer **`ode_solvers`** as the first Gate 1B integration candidate: native DOP853, Apache-2.0, `h_max`, solout, dense output, mature downloads.
2. Plan an adapter that:
   - maps our `RayState` ↔ `SVector<f64, 8>` / `DVector<f64>`;
   - applies component scales for position/momentum tolerances;
   - wraps solout for metric-domain `h` guards and invariant sampling;
   - feeds dense output into our bracket/root event localizer (owned by us).
3. Keep **`ivp`** as a backup if `ode_solvers` dense-output/event ergonomics fail a spike.
4. Do **not** choose `diffsol` for the primary null-geodesic oracle unless ADR 0002 is revised away from DOP853.
5. A from-scratch DOP853 is **not** justified by this audit: existing crates can meet the contract with an adapter layer.

## Explicit Gate 1A action

- No ODE crate added to workspace dependencies.
- ADR 0005 left **Proposed** pending owner review and a Gate 1B spike.
