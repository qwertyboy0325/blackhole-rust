# Spectral coordinate convention V1

Gate 2B2. Canonical spectral measure and grid for diagnostic `I_ν` transport.

## Measure

```text
SpectralMeasure::FrequencySpecificIntensity  (canonical)
SpectralMeasure::WavelengthSpecificIntensity (derived view only)
```

Mismatched measure reinterpretation without Jacobian is a typed reject.

## Diagnostic frequency domain

Observer-frame dimensionless frequency interval:

```text
ν ∈ [ν_min, ν_max] = [0.25, 4.0]
```

This axis is independent of Gate 2B0 kinematic `ν_obs,kin = 1`.

## Authoritative grid

Id: `spectral-grid-v1` (**frozen**)

```text
spacing: logarithmic in frequency
bins: 64 (authoritative; Gate 2B2 owner closure 5203577417)
edges: n+1 log-spaced points on [ν_min, ν_max]
centers: geometric midpoints of adjacent edges
weights: Δν = edge[i+1] − edge[i]  (rectangle rule on centers)
```

### Grid-selection / error-budget rule

Freeze `spectral-grid-v1` at 64 bins when **both** observed and emitted relative
ladders satisfy (smoke corpus):

```text
e64 ≤ e32 / 2                 (≥2× improvement 32→64)
e64 ≤ CLOSURE_REL_TOL         (error budget; frozen 2e-3)
e128 ≤ e64                    (64→128 improves)
(e64/e128) ≤ 1.25 (e32/e64)   (64→128 not accelerating)
e256 ≤ e128                   (128→256 improves)
(e128/e256) ≤ 1.25 (e64/e128) (128→256 not accelerating)
```

Absolute budget `CLOSURE_ABS_TOL = 2e-3` is calibrated to the measured gate
max abs observed ≈ `1.988e-3` (see final report cost/rationale section).

Exploratory grids (32 / 128 / 256) remain non-authoritative convergence probes only.

## Transport evaluation

```text
fixed observer-frame ν grid
for each disk-hit pixel and bin ν_obs:
  ν_em = ν_obs / g
  evaluate analytic φ(ν_em) (or zero if out of domain)
  I_ν,obs = g³ · I_em,bol · φ(ν_em)
```

Bolometric recovery on the observer grid:

```text
∫ I_ν,obs dν_obs           ≈ g⁴ I_em,bol · M_capt
∫ I_ν,em (dν_obs / g)      ≈ I_em,bol · M_capt
M_capt = ∫_{ [ν_min/g, ν_max/g] ∩ domain } φ dν
truncation fraction = 1 − M_capt
```

Closure compares spectral integrals to Gate 2B1 bolometric × `M_capt`
(not the untruncated bolo when `|ln g|` is large).

No table interpolation in V1 for the production continuum.

## Layout

Pixel-major storage: `[pixel][bin]`. Non-disk outcomes store absence, not a
zero spectrum.

## Known limitations

- Finite-band truncation under large redshift/blueshift.
- Rectangle-rule quadrature error (documented via convergence).
- Diagnostic units only; not SI radiometric calibration.
- No colorimetry metadata (Gate 2B3 boundary).
