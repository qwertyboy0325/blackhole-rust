# Finite celestial-boundary coordinate convention (Gate 2A1)

## Claim

Escaped-ray celestial coordinates are defined on the **finite diagnostic escape
boundary** where the localized event satisfies

```text
r_oblate = r_escape
```

Source for every escaped sample:

```text
finite-oblate-escape-boundary-position = EscapeHit.state.position
```

This is **not**:

- an asymptotic null direction at future/past null infinity;
- a Euclidean `normalize([x,y,z])` direction at nonzero spin (in general);
- a terminal-momentum / propagation direction;
- a celestial texture, radiance, or finished lensed-sky image.

Asymptotic correction is **not implemented**.

## Boundary surface

Constant **oblate Kerr–Schild radius** surface. The Gate 1B2-compatible
diagnostic scene currently resolves escape radius via

```text
resolved = min(preset.celestial_sphere.radius_m, 80)
policy   = gate-1b2-diagnostic-radius-cap
```

For `gargantua-baseline.toml`, the preset requests `1000 M` but the diagnostic
cap yields `80 M`. Artifacts record both the requested and resolved radii.

## Angular chart

Ingoing spherical Kerr–Schild `(T, r, θ, ψ)` with embedding

```text
x + i y = (r + i a) e^{iψ} sinθ
z       = r cosθ
```

Coordinate-sphere unit direction:

```text
d = [sinθ cosψ, sinθ sinψ, cosθ]
```

Canonical ranges: `θ ∈ [0, π]`, `ψ ∈ [0, 2π)`.

Handedness (Schwarzschild `a = 0`): `ψ = 0 → +x`, `π/2 → +y`, `π → −x`,
`3π/2 → −y`.

## Seam and UV

Preset seam (accepted only): `positive_x_half_plane`.

At finite nonzero spin this means the **spherical KS `ψ = 0` seam**,
asymptotically aligned with the positive-x half-plane — not a claim that the
finite oblate surface seam equals Euclidean `y = 0, x > 0` exactly.

```text
u = wrap_0_2π(ψ) / 2π
v = θ / π
```

So `ψ = 0 → u = 0`, north pole `v = 0`, south pole `v = 1`.

## Pole policy

Generic `spherical_ks_from_cartesian` continues to reject undefined pole
azimuth (`|sin θ| < AXIS_SIN_FLOOR = 1e-14`).

Celestial mapping uses `spherical_ks_direction_from_cartesian`:

- north: `θ = 0`, `ψ = 0`, `d = [0,0,1]`, status `canonicalized-north-pole`
- south: `θ = π`, `ψ = 0`, `d = [0,0,-1]`, status `canonicalized-south-pole`

## Diagnostic UV image

`celestial-uv-debug.ppm` quantizes `(u,v)` into RG with B=255 for escaped
pixels; non-escaped pixels use Gate 1B2 categorical colors.

**This image visualizes a coordinate field. It is not a celestial texture,
physical radiance image, or final lensed-sky render.**

## Known limitations

- No asymptotic handoff / null-infinity direction.
- No star field, procedural texture sampling, filtering, or radiometry.
- Escape residual ranking is diagnostic only; existing event localization
  semantics remain authoritative.
