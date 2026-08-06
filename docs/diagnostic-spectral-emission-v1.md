# Diagnostic spectral emission V1

Gate 2B2 scientific channel. Sampled spectral specific-intensity transport from
the Gate 2B1 thin-disk bolometric scale. Not temperature, Planck/blackbody,
Novikov–Thorne, physical RGB, OpenEXR, absorption, scattering, polarization,
volumetric transfer, or Interstellar/DNGR reconstruction.

## Quantity

Canonical measure:

```text
I_ν  — specific intensity per unit frequency
units: arbitrary-normalized-spectral-specific-intensity-per-unit-frequency
```

`I_λ` is a **derived-only** view (Jacobian required). Never reinterpret `I_ν`
samples as `I_λ` without conversion.

Kinematic Gate 2B0 camera unit-frequency (`ν_obs,kin ≡ 1`) is **not** the
spectral bin axis. Spectral grids use a separate observer-frame diagnostic
frequency coordinate.

## Frozen emission shape

Production continuum id: `diagnostic-lognormal-continuum-v1`

```text
I_ν,em(r, ν) = I_em,bol(r) · φ(ν)

φ(ν) ∝ (1/ν) exp( −(ln ν − μ)² / (2 σ²) )   for ν ∈ [ν_min, ν_max]
φ(ν) = 0                                      otherwise

∫_{ν_min}^{ν_max} φ(ν) dν = 1
```

Canonical parameters (dimensionless diagnostic frequency):

```text
μ = 0
σ = 0.5
ν_min = 0.25
ν_max = 4.0
```

`I_em,bol(r)` is the Gate 2B1 diagnostic radial profile
`diagnostic-radial-power-law-v1` — never recomputed from a temperature model.

Angular model: isotropic in the emitter frame (no independent beaming).

## Transport

Consume Gate 2B0 `g = ν_obs / ν_em` only. Do not recompute `g`.

```text
I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs / g)
```

Bolometric recovery (frequency-domain integral):

```text
∫ I_ν,obs dν_obs = g⁴ ∫ I_ν,em dν_em
```

Canonical arithmetic for the cubic factor:

```text
g2 = g * g
g3 = g2 * g
I_ν,obs = I_ν,em * g3
```

## Finite domain / truncation

Emitter support is exactly `[ν_min, ν_max]`. If `ν_em = ν_obs / g` lies outside
that interval, the observed spectral sample is **zero** and truncated-energy
fractions are recorded. No silent extrapolation.

## Wavelength (derived)

```text
λ = c_diag / ν     (diagnostic conversion constant; not a SI claim in digests)
λ_em = g λ_obs
I_λ,obs(λ_obs) = g⁵ I_λ,em(g λ_obs)
```

## Line fixture (tests only)

`diagnostic-gaussian-line-v1` is a hermetic narrow bump for shift/amplitude
tests. It is **not** the production diagnostic continuum.

## Optics / provenance

Disk remains zero-thickness opaque; first backward intersection terminates.
Emission provenance strings remain Gate 2B1 exact presets.

## Explicit absences

Temperature, Stefan–Boltzmann, Planck, Novikov–Thorne, Shakura–Sunyaev, GRMHD,
CIE colorimetry, physical RGB / ACEScg, OpenEXR / HDR, GPU / wgpu / egui,
polarization, optical depth, returning radiation.
