# ADR 0002: DOP853 with dense-output event localization

- Status: Accepted for Gate 1
- Date: 2026-08-03

## Decision

Use adaptive DOP853 [Hairer1993] for the CPU `f64` oracle. Apply component-scaled local error,
curvature/domain step guards, dense output, safeguarded root location, and
subdivision when surface ordering or tangency is ambiguous.

Every ray returns one typed physical, resource, or numerical outcome. Numerical
failures are not converted into colors.

## Rationale

Near-critical trajectories need high accuracy over many steps, while ordinary
rays should not pay a uniformly tiny fixed step. Dense output localizes disk,
horizon, and sky events without reducing every accepted step to event accuracy.

## Calibration

Preset tolerances are initial run controls only. Acceptance budgets follow the
convergence and higher-precision protocol in `docs/validation-plan.md`.

## Rejected alternatives

- fixed RK4: simple and GPU-friendly, but inefficient as a CPU oracle and weak
  for event localization;
- symplectic-only integration: attractive for long Hamiltonian trajectories,
  but adaptive/event-rich null tracing and nonseparable chart Hamiltonians make
  it a later measured alternative, not an assumed win;
- endpoint sign checks: can miss multiple or tangential crossings.
