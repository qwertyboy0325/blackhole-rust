# Presentation pipeline V1 (Gate 2D0)

Display-referred cinematic SDR presentation over immutable Gate 2C1
`PhysicalColorFrame` scientific authority.

## Boundary

```text
SCIENTIFIC AUTHORITY  = PhysicalColorFrame / raw f64le / physical_color_digest
PRESENTATION STATE    = PresentationSpec + DisplayEncodedRgb16 + beauty PNG
```

Presentation never mutates scientific digests. Beauty ≠ radiance authority.
Absence → display black is a presentation fill only.

## Pipeline

```text
PhysicalColorFrame (absolute photometric Rec.709/D65 f64)
  → exposure (A1)
  → luminance-axis-desat-v1 (A2)
  → khronos-pbr-neutral-v1
  → strict [0,1]±ε validation
  → sRGB OETF (IEC 61966-2-1)
  → u16 quantization
  → RGB16 PNG (A3/A4)
```

### A1 exposure

```text
RGB_exposed =
  RGB_absolute × 0.18 × 2^exposure_ev / middle_gray_luminance_cd_m2
```

`EV = 0` maps `middle_gray_luminance_cd_m2` → linear **0.18**.
Canonical preset freezes median Gate 2C1 Y and `exposure_ev = 0`.

### A2 gamut

`luminance-axis-desat-v1` resolves negative / OOG chromaticity along the equal
Rec.709-luminance neutral axis. HDR `RGB > 1` is valid and unchanged.
Post–tone-map significant `<0` or `>1` is a pipeline failure (ε-only
endpoint canonicalize).

### Tone mapper

`khronos-pbr-neutral-v1` — official Khronos PBR Neutral analytic operator
(CC-BY-4.0). No LUT / AgX / ACES / Hable / Reinhard in V1.

### A3 / A4 PNG

```text
RGB16
sRGB chunk intent = Perceptual (0)   # fixed, not preset-selectable
gAMA = 45455
cHRM = OMIT
ICC  = OMIT
```

Authority for reproducibility: decoded RGB16 raster digest
(`PRESENTATION_REPRODUCIBILITY_DIGEST`), not DEFLATE bytes.

## Preset

`presets/presentation/gargantua-cinematic-v1.toml` — separate from scientific
`presets/gargantua-physical-v1.toml`.

## CLI

```bash
cargo run --release -p xtask -- render-presentation \
  --preset presets/gargantua-physical-v1.toml \
  --presentation presets/presentation/gargantua-cinematic-v1.toml \
  --tier gate \
  --output-dir artifacts/gate-2d0-presentation \
  --execution parallel --threads N --require-release

cargo run --release -p xtask -- evaluate --scope gate-2d0-presentation
```

## Artifacts

```text
presentation-meta.json
beauty-srgb16.png
presentation-report.json
```

## Non-goals

Auto exposure, 8-bit derivative, standalone payload postprocess, bloom/glare,
environment assets, HDR10/PQ/HLG, grading, LUT, GPU, GUI, E2/E3.
