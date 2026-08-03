# Physical assumptions and equations

This document is normative for Gate 0. Symbols use geometrized units
`G = c = M = 1` unless stated otherwise, metric signature `(-,+,+,+)`, Greek
indices for coordinates, and hatted Latin indices for a local orthonormal frame.
Equation sources are keyed to [`research-sources.md`](research-sources.md).

## Spacetime

The physical model is an isolated, stationary, axisymmetric, uncharged Kerr
black hole. It neglects self-gravity of the disk, cosmological expansion,
external bodies, plasma dispersion, and dynamical spacetime effects.

In Boyer-Lindquist notation [Carter1968, BPT1972],

```text
Sigma = r^2 + a^2 cos^2(theta)
Delta = r^2 - 2 M r + a^2
r_+/- = M +/- sqrt(M^2 - a^2)
```

`|a| <= M` is required. The outer event horizon is `r_+`. The outer stationary
limit (ergosurface) is

```text
r_ergo(theta) = M + sqrt(M^2 - a^2 cos^2(theta)).
```

The ergoregion lies between that surface and the horizon. It is not an opaque
surface and is not a ray termination event. The ring singularity is inside the
horizon and outside the renderer's integration domain.

The baseline uses `a/M = 0.999`, which is the fast-spin value used for many
published DNGR demonstrations [James2015]. The same paper says the filmmakers
used `a/M = 0.6` for a more comprehensible disk image and suppressed physically
strong brightness asymmetry. Therefore `0.999` is a published-inspired research
choice, not a claim about every final movie shot or Gargantua's story-level spin.

## Coordinates

The primary numerical chart is Cartesian Kerr-Schild `(t,x,y,z)` [GRay2,
Skylight2022]. It is regular across the outer horizon and symmetry axis. Its
oblate-spheroidal radius is the nonnegative root

```text
r^2 = 1/2 * ((rho^2 - a^2)
             + sqrt((rho^2 - a^2)^2 + 4 a^2 z^2)),
rho^2 = x^2 + y^2 + z^2.
```

Boyer-Lindquist coordinates remain an input/reporting chart and support an
independent separated-equation oracle. Their `Delta` denominators are singular
at the horizon, `phi` is ill-conditioned on the axis, and near-extremal
`r_+ - r_-` magnifies cancellation. No production ray is allowed to rely on
crossing the horizon in that chart.

## Null geodesics

The primary formulation is the canonical Hamiltonian [Carter1968, MTW1973]:

```text
H(x,p) = 1/2 g^(mu nu)(x) p_mu p_nu = 0
dx^mu/dlambda =  dH/dp_mu = g^(mu nu) p_nu
dp_mu/dlambda = -dH/dx^mu
```

Stationarity and axisymmetry imply the conserved quantities
`E = -p_t` and `L_z = p_phi` when evaluated in Boyer-Lindquist components. Kerr
separability supplies the Carter constant for null rays:

```text
Q = p_theta^2 + cos^2(theta)
    * (L_z^2 / sin^2(theta) - a^2 E^2).
```

`H`, `E`, `L_z`, and `Q` are diagnostics, not variables to project back onto
their initial values. Projection could conceal integration error.

The independent oracle uses Carter's separated first-order radial and polar
potentials. It must handle radial and polar turning points explicitly; blindly
taking a square-root sign loses branch information. Analytic elliptic-integral
codes such as YNOGK, AART, and Krang are references for later differential
testing, not copied dependencies.

## Observer and camera

An observer is a future-directed unit timelike four-velocity `u^mu` with
`g(u,u) = -1`. A right-handed orthonormal tetrad `e_(a)^mu` satisfies

```text
e_(0) = u
g_mu_nu e_(a)^mu e_(b)^nu = eta_(a b).
```

The camera maps a pixel to a unit spatial direction `n^(i)` in that local frame.
For backward tracing it constructs a past-directed null vector
`k^(a) = (-1, n^(i))`, transforms it with the tetrad, and advances a positive
backward-trace parameter. Radiometric calculations use the equivalent
future-directed vector `-k`. Tetrad normalization, handedness, nullness, and
future/past orientation are checked before integration [James2015, BPT1972].

The baseline observer is a ZAMO at a project-chosen radius and inclination. This
is a reproducible diagnostic camera, not a published movie camera.

## Disk and emission

The first disk is a zero-thickness equatorial surface with configurable inner
and outer Boyer-Lindquist radii. The default inner edge is the prograde Kerr ISCO
computed from Bardeen, Press, and Teukolsky [BPT1972], not a hand-tuned ring. It
is optically thick: the nearest backward intersection terminates the ray.

Geometry, material velocity, and emission are independent interfaces:

- geometry answers whether and where a ray intersects;
- velocity supplies a normalized emitter four-velocity;
- emission supplies local-frame spectral specific intensity.

The baseline emission is a labeled diagnostic profile, not a Novikov-Thorne or
GRMHD claim. The published *Interstellar* disk was art-directed and intentionally
anemic [James2015]. Gate 0 does not infer its unavailable textures or parameters.

## Frequency shift and radiance

For future-directed photon momentum `k` and observer/emitter velocities, the
frequency ratio is [Younsi2012]

```text
g = nu_obs / nu_em = (k_mu u_obs^mu) / (k_mu u_em^mu).
```

Specific intensity obeys the invariant `I_nu / nu^3`; consequently

```text
I_obs(nu_obs) = g^3 I_em(nu_obs / g),
I_obs,bolometric = g^4 I_em,bolometric.
```

This combines gravitational and kinematic Doppler shifts. Beaming is not an
independent arbitrary multiplier. Absorption, scattering, polarization, plasma
dispersion, and time-dependent transfer are deferred.

## Photon regions, shadows, and critical curves

Kerr has a family of unstable spherical photon orbits rather than one spherical
"photon sphere" at general spin. Their projection separates captured and
escaping directions and forms the ideal critical curve for a distant observer
[Bardeen1973, Gralla2020]. The dark shadow additionally depends on illumination
and absorbing boundaries. These terms must not be used interchangeably.

## Known numerical failure modes

- horizon and axis coordinate singularities in Boyer-Lindquist coordinates;
- cancellation near extremal spin and near radial/polar turning points;
- rays lingering exponentially near unstable photon orbits;
- missed disk or horizon events when a large step crosses multiple surfaces;
- tangential disk contact without a sign change;
- tetrad handedness or time-orientation mistakes;
- subtractive error when deriving `r` in Cartesian Kerr-Schild coordinates;
- non-finite metric derivatives or momenta;
- false confidence from small average error while critical pixels are wrong;
- `f32` branch divergence close to the critical curve.

Every such condition must produce diagnostics or a typed failure, never a
plausible fallback color.
