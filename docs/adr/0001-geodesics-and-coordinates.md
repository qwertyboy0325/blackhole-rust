# ADR 0001: Hamiltonian geodesics in Cartesian Kerr-Schild coordinates

- Status: Accepted for Gate 1
- Date: 2026-08-03

## Context

The primary solver must survive near-horizon rays, expose numerical error, and
avoid coupling Kerr-specific shortcuts to the whole core.

## Decision

Use canonical Hamilton equations with covariant momentum in Cartesian
Kerr-Schild coordinates and Rust `f64`. Implement Carter-separated
Boyer-Lindquist null equations independently as a differential oracle.

## Consequences

The hot path needs trustworthy inverse-metric derivatives and an implicit
oblate-radius calculation. In return, the primary chart crosses the horizon and
axis regularly, `H=0` is directly measurable, and the interface can later host
other stationary metrics. Carter constants remain diagnostics rather than
constraints projected onto the state.

## Alternatives

- Second-order Christoffel equations: generic but more derivative algebra and a
  less direct null constraint; retained for optional testing.
- Carter-separated equations as primary: efficient, but Kerr-specific and
  branch-sensitive at turning points.
- Analytic elliptic functions: excellent later oracle/accelerator, but too much
  special-function branch complexity for the first production abstraction.
