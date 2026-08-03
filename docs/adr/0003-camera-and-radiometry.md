# ADR 0003: Tetrad camera and spectral invariant radiometry

- Status: Accepted for Gate 1 interfaces
- Date: 2026-08-03

## Decision

Construct rays in an observer's local orthonormal tetrad, then transform into the
integration chart. Keep projection separate from observer motion.

Represent emission as local-frame spectral specific intensity in `f64`, apply
`g = nu_obs/nu_em` through the invariant `I_nu/nu^3`, and only then integrate to
scene-linear color. Disk geometry, emitter velocity, and emission are distinct.

## Consequences

Moving observers and coordinate charts cannot silently redefine a lens. Spectral
sampling costs more than direct RGB, so its grid is configurable and validated by
convergence. A cheap diagnostic emitter can precede an astrophysical disk model
without changing geometry or geodesics.

## Rejected alternatives

- coordinate-component camera rays: fragile near curved-coordinate axes and
  ambiguous for moving observers;
- direct sRGB emission: cannot apply frequency shifts consistently;
- arbitrary Doppler brightness multiplier: duplicates or contradicts invariant
  transfer.
