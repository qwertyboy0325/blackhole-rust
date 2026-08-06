# Diagnostic bolometric emission V1

Gate 2B1 scientific channel. Not spectra, temperature, physical RGB, OpenEXR,
Novikov–Thorne, Shakura–Sunyaev, GRMHD, or Interstellar film reconstruction.

## Quantity

Normalized **bolometric specific intensity** in arbitrary project units:

```text
arbitrary-normalized-bolometric-specific-intensity
```

Not total luminosity, not flux per proper area, not observer flux density, not
spectral radiance, not display brightness.

## Frozen emission profile

```text
I_em(r) = normalization × (r_inner / r)^radial_exponent
```

Canonical V1 (`diagnostic-radial-power-law-v1`):

```text
normalization = 1
radial_exponent = 3
I_em(r) = (r_inner / r)^3
```

Normalization radius source: **resolved trace-scene thin-disk inner radius**.

Angular model: **isotropic in the emitter frame** (no independent beaming factor).

At current Gate diagnostic bounds `r_inner = 3 M`, `r_outer = 20 M`:

```text
I_em(3 M)  = 1
I_em(20 M) = (3/20)^3 = 0.003375
```

The actual Gate 1B2 / Gate 2B scene disk still begins at the geometric diagnostic
radius `3 M`. Preset `inner_edge = "prograde_isco"` is **not** activated. Do not
claim the traced disk inner edge is ISCO.

Disk-bounds source tag: `resolved-trace-scene-thin-disk-v1`.

`ResolvedDiskBounds` is a closed typed invariant: finite, `inner > 0`,
`outer > inner`. Construction and deserialization go through `new()` /
`validate()`; public sampling and digest entry points re-validate. Illegal
bounds are rejected — never clamped.

## Emission provenance

Gate 2B1 accepts only the exact preset strings:

```text
emission_model = "diagnostic_radial_profile"
emission_claim = "project diagnostic, not astrophysical or film-asset reconstruction"
```

These are validated before tracing. The accepted model and claim are recorded in
the bolometric map artifact, worker report, and scientific digest. An altered
claim or unsupported model rejects with no artifacts.

Convention kebab claim
(`project-diagnostic-not-astrophysical-or-film-reconstruction`) is a separate
convention-id field and is not a substitute for the preset provenance string.

## Transport

Consume Gate 2B0 frequency ratio `g = ν_obs / ν_em` only. Do not recompute `g`
from geodesic state.

Bolometric specific-intensity transport:

```text
I_obs = g⁴ I_em
```

Canonical arithmetic:

```text
g2 = g * g
g4 = g2 * g2
I_obs = I_em * g4
```

No independent Doppler-beaming multiplier, no left/right brightness factor, no
`g³`, no `1/r²` attenuation for specific intensity, no separate lensing
magnification. Ray-to-pixel mapping already encodes lensing.

## Optics

The disk remains zero-thickness, opaque; the first backward intersection
terminates the ray. No transparent or volumetric transfer.

## Display

`fixed-log2-grayscale-v1` maps positive intensity to grayscale via clamped
`log2(I / reference)` stops. Clamping is visualization-only and does not alter
scientific scalars or digests.

The disk–celestial composite combines the Gate 2A2 procedural sky for escaped
pixels with grayscale `I_obs` for disk hits. It is **not** physical RGB or a
film-equivalent image.

## Explicit absences

Wavelength grids, `I_ν`, blackbody/Planck, temperature, Stefan–Boltzmann flux,
physical RGB / ACEScg / OpenEXR / HDR, GPU / wgpu / egui are out of scope.
