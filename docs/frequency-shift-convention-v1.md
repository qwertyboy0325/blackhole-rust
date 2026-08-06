# Frequency-shift convention V1

Gate 2B0 kinematics only. Not emission, intensity, spectra, or physical RGB.

## Photon momentum orientation

The tracer stores a **past-directed** covariant momentum `p_backward`.

The equivalent future-directed photon momentum is:

```text
k_future = -p_backward
```

For metric signature `(-,+,+,+)` and a future-directed timelike observer `u`, the
positive measured frequency is:

```text
ν = p_backward_μ u^μ
  = -k_future_μ u^μ
```

Do **not** implement `-p_backward_μ u^μ` for the backward API — that reverses the
project orientation convention.

Do not call the past-directed covector itself the emitted or observed physical
photon momentum without the orientation qualifier.

## Camera unit-frequency normalization

Rectilinear camera initialization uses the observer-local past null:

```text
k̂_past^(a) = (-1, n̂)
```

Therefore:

```text
ν_obs = p_backward_μ u_obs^μ = 1
```

up to the existing tetrad/ray-initialization residual (envelope `1e-10`).

Production Gate 2B0 ratio:

```text
g = ν_obs / ν_em = 1 / ν_em
```

Source tag: `camera-local-unit-past-null`.

`ν_obs` is **not** obtained by contracting a terminal covector with an observer
at a different spacetime event. Observer verification reconstructs initial rays
only (no geodesic integration).

## Circular equatorial emitter

```text
Ω_s = s √M / (r^(3/2) + s a √M)
u^μ_BL = u^t (1, 0, 0, Ω_s)
(u^t)^-2 = -(g_tt + 2 Ω_s g_tφ + Ω_s² g_φφ)
```

evaluated at `(t=0, r, θ=π/2, φ=0)` for the metric factors, with

```text
s = +1  PositivePhi
s = -1  NegativePhi
```

Prograde policy:

```text
a > 0  → PositivePhi
a < 0  → NegativePhi
a = 0  → PositivePhi (project convention)
```

Not a ZAMO. Radius is never clamped. Invalid orbits return typed errors.

## Equatorial surface canonicalization

Disk hits are equatorial event surface samples. Emitter velocity uses:

```text
r     = localized oblate radius
θ     = π/2
t, φ  = localized event values
```

Policy id: `localized-radius-equatorial-surface-canonicalization-v1`.

Photon conserved BL `p_t` and `p_φ` come from the localized hit state after KS→BL
covector transform at the recovered BL event.

`disk_radius_residual = recovered_BL_radius - DiskHit.oblate_radius`.

## BL / KS contraction invariance

At a shared event:

```text
p_BL · u_BL = p_KS · u_KS
```

within existing coordinate-transform numerical envelopes (`1e-10`).

## Distinction from brightness

`g` is a frequency ratio. This gate does **not** implement:

- emitted intensity;
- `g³` spectral intensity transport;
- `g⁴` bolometric transport;
- temperature, emissivity, spectra, colorimetry;
- physical RGB or OpenEXR.

Diagnostic `g-factor-debug.ppm` clamps `log2(g)` for visualization only and does
not alter scientific `g` or digests.

## Geometric disk radius policy

The actual Gate 1B2 inner radius remains the geometric diagnostic radius `3M`.
Preset fields such as `inner_edge = "prograde_isco"` are not activated by this
gate. The velocity model is evaluated at the localized disk-hit radius.

## Known limitations

- Circular equatorial geodesics only; no plunging-region velocity.
- Thin opaque disk geometry unchanged; no finite thickness.
- Observer frequency fixed by camera initialization, not terminal contraction.
- No ISCO migration of the traced disk inner edge.
