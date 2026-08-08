# Physical colorimetry V1 (Gate 2C1)

Absolute CIE 1931 2° XYZ and scene-linear Rec.709/D65 RGB from
`PhysicalDiskEmissionFrame` (Architecture **B**). OpenEXR FLOAT is a
**derived interchange** artifact; little-endian f64 + meta JSON remain
scientific authority.

## Architecture

| Path | Role |
| --- | --- |
| B — CIE 1 nm from `(F, T_eff, g)` + Planck + `g³` | **Production** |
| A — project 256-bin `PhysicalSpectralFrame` | Diagnostic A-vs-B only |
| C — new high-res spectral product | Rejected |

Provenance: XYZ is **not** integrated from the sparse 256-bin cube (~13 visible
bins). Inherited physics = Gate 2C0 Planck / `transport_i_nu` / SI Hz.

## Measure

Production (frequency):

```text
X,Y,Z = K_m ∫ I_ν(ν) {x̄,ȳ,z̄}(c/ν) dν
I_ν,obs(ν_obs) = g³ B_ν(ν_obs/g, T_eff)
K_m = 683 lm/W
```

Wavelength check uses SI Jacobian `I_λ = I_ν · (c/λ²)` (never diagnostic `C=1`).

Y is luminance in **cd/m²**. No per-frame max-normalization or exposure.

## CIE data

- Observer: CIE 1931 2° (`cie-1931-2deg-v1`)
- Table: DOI [10.25039/CIE.DS.xvudnb9b](https://doi.org/10.25039/CIE.DS.xvudnb9b)
- md5 `17cca777db64b17170f06f67ce9d3ab7`
- SHA-256 `fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1`
- Vendored path: `assets/standards/cie1931-2deg-v1.csv`
- License: **CC BY-SA 4.0** (see `assets/standards/LICENSE-CIE-CC-BY-SA-4.0.txt`).
  Repository code remains MIT OR Apache-2.0; the table is **not** re-licensed.
- Production nodes: 380–780 nm @ 1 nm (401 samples). Full official file is 360–830.

## RGB

- Space: `scene-linear-rec709-d65-v1` (IEC 61966-2-1 primaries / D65, **linear**, no OETF)
- No Bradford / CAT / creative white balance / ACEScg in V1
- Finite negatives and values > 1 allowed; never clamped in the scientific path

## Artifacts

```text
AUTHORITATIVE: physical-colorimetry-meta.json + physical-xyz-rgb.f64le + digests
DERIVED_SCIENTIFIC: physical-color.exr (FLOAT XYZ/RGB + phys.* + disk.mask)
DIAGNOSTIC: selected-pixels.csv, diagnostic-a-vs-b.json
PRESENTATION_ONLY: DEFER (no tone-map / PNG beauty)
```

## CLI

```bash
cargo run --release -p xtask -- render-physical-color \
  --preset presets/gargantua-physical-v1.toml --tier gate \
  --cie-observer cie-1931-2deg-v1 \
  --rgb-space scene-linear-rec709-d65-v1 \
  --output-dir artifacts/gate-2c1-colorimetry \
  --execution parallel --threads N --require-release

cargo run --release -p xtask -- evaluate --scope gate-2c1-colorimetry
```

## Frozen 2C0 authorities (must remain bit-identical)

| Authority | Digest |
| --- | --- |
| frequency (2B0) | `65df7b55…a875c2` |
| emission | `5e3b1502…9a383e` |
| spectral | `136b1fbc…aa8dd1` |
| `physical-spectral-grid-v1` | `ceb3db28…1162d5` |

## Non-goals

ACEScg, CAT, exposure/tone-map/OETF/PNG beauty, HALF/DWA EXR, 256 spectral EXR
channels, GPU color, E2/E3/GUI.
