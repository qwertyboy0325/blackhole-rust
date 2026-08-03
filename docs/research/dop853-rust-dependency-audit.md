# DOP853 Rust dependency audit (Gate 1A remediation)

**Status:** research only — no production ODE dependency added.  
**Date:** 2026-08-03 (updated under PR #1 owner review)  
**Related ADR:** `docs/adr/0005-dop853-dependency.md` (**Proposed**)

## Contract required by Gate 0 / ADR 0002

- Adaptive DOP853 (order 8(5,3)) in Rust `f64`
- Component-scaled absolute/relative tolerances (position vs momentum)
- Dense output suitable for **project-owned** event localization
- Accepted-step callbacks / solout-style inspection
- Access to step statistics (accepted/rejected, last step, nfcn)
- Ability to impose geometry-specific maximum-step guards
- Deterministic behavior on a pinned toolchain
- Compatible license with `MIT OR Apache-2.0`

## Distinction required by owner review

| Capability | Meaning |
|---|---|
| Mathematical dense output | Method has order-7 continuous extension coefficients in the literature/code |
| Public accepted-step dense coefficients | API exposes the coefficient vectors / interpolant state after each accepted step for an external root finder |
| Sampled output at requested points | API can evaluate `y(x*)` on a grid / continuous model without exposing coefficients |
| Callback timing | When `solout`/hooks run (accepted step only vs stage) |
| Dynamic step-guard control | Ability to shrink `h` from a callback based on geometry |
| Vector tolerance support | Per-component `atol`/`rtol` without external state rescaling |

## Candidates

### 1. `ode_solvers` 0.6.2 (Apache-2.0)

| Criterion | Finding |
|---|---|
| License / provenance | Apache-2.0; [srenevey/ode-solvers](https://github.com/srenevey/ode-solvers) |
| DOP853 | Present (order 8(5,3), dense output order 7 mathematically) |
| Tolerances | **Scalar** `rtol`/`atol` in public `Dop853::new` / `from_param` |
| Callback | `System::solout` after successful steps; can halt |
| Dense coefficients | **Not proven** for public `Dop853`: continuous-output helpers are documented primarily for Dopri5 paths; accepted-step coefficient access for an external localizer is **unproven** at this audit |
| Sampled output | Dense/`ContinuousOutputModel` style APIs exist in the crate family; Dop853 parity must be spiked |
| Step guards | `h_max`, `n_max`, safety factors via `from_param` |
| Stats | Integration result / bookkeeping via solver state |
| Event localization contract | **Not established** by public API survey alone |

### 2. `ivp` 0.6.0 (Apache-2.0)

| Criterion | Finding |
|---|---|
| License / provenance | Apache-2.0; [Ryan-D-Gast/ivp](https://github.com/Ryan-D-Gast/ivp) |
| DOP853 | Listed |
| Tolerances | Exposes **vector** tolerances |
| Callback | `SolOut` |
| Dense / interpolation | DOP853 interpolation advertised |
| Stats | Step/evaluation statistics exposed |
| Maturity | Younger ecosystem; requires Gate **1B0** source-level spike |
| Event localization contract | Promising on paper; **not adopted** until spike proves stop/restart, guards, and coefficient/interpolant access |

### 3. `diffsol` (MIT)

| Criterion | Finding |
|---|---|
| Methods | BDF/SDIRK/ERK — **no DOP853** |
| Fit | Wrong method family for ADR 0002 unless that ADR is revised |

## Recommendation (post Gate 1B0 spike)

1. ADR 0005 remains **Proposed** until owner review.
2. Measured fit favors **`ivp`** for vector tolerances and accepted-step
   `StepInterpolant` in `SolOut`.
3. **`ode_solvers`** remains usable with adapter limitations (scalar tol; dense
   via predetermined dx samples; private `rcont`).
4. From-scratch DOP853 remains unapproved.

See `docs/research/gate-1b0-dop853-spike-report.md` for experiment evidence.

## Explicit Gate 1A action

- No ODE crate in production `relativity-core` dependencies.
- Spike-only deps under `spikes/gate-1b0-*`.
