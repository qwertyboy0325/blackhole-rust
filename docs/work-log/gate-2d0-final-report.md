# Gate 2D0 final report — cinematic presentation pipeline

## Result

**Authoritative evaluate PASS** on clean worktree.

| Field | Value |
| --- | --- |
| Evaluated tip | `beb4efd0ffd419f8a81f98b6620e11ca867aa98e` |
| Evaluation content digest | `d01dcc7bbcee731fdbacb40cf156aa1fd6be53cc7b4a3a0f7fa9a13ac9a7bc29` |
| `result` | `PASS` |
| `authoritative` | `true` |
| `dirty` | `false` |
| Scope | `gate-2d0-presentation` |
| Scientific inheritance | `SCIENTIFIC_INHERITANCE_PASS` |
| Presentation pipeline | `PRESENTATION_PIPELINE_PASS` |
| Planning / merge base | `c964c746fe3819627455a170e5e46b74731c0412` (PR #19 Gate 2C1) |
| PR | draft (pending owner merge); D0-C1 closed @ `beb4efd` |

## Frozen scientific inheritance (exact)

| Authority | Digest |
| --- | --- |
| `physical_color_digest` | `16663188fad338c0fc8197dddd8268bd705f817b165a35853b16b211c7635793` |
| `payload_sha256` | `d317c517661a64f8ffdacead3dd222370056abc8eed81706d660bc4ebda81cf5` |
| CIE table SHA-256 | `fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1` |
| Gate 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |
| Gate 2C0 emission | `5e3b15023df9bf3debed9666d65a3c762cfe83fe9885e7a5c8b3565dc19a383e` |
| Gate 2C0 spectral | `136b1fbcc76beb08ea38aa24d16803d621da20bad5b7ebfecc7a13c260aa8dd1` |
| `physical-spectral-grid-v1` | `ceb3db28082bb357e50cac2635b221711bf79ea2806f2c25b60c61ca901162d5` |

## Presentation reproducibility digests

| Field | Value |
| --- | --- |
| `presentation_spec_digest` | `e6639e75d67156852f8f064e7ef9f4f2b82ab8018b707399c851522780a6dd49` |
| `presentation_frame_digest` | `f8e103239a331796bd474ff121627eecd0781f31c840f46d9f2d3a85c8d1e87b` |
| Label | `PRESENTATION_REPRODUCIBILITY_DIGEST` (not scientific authority) |

## Canonical presentation preset

| Field | Value |
| --- | --- |
| Preset | `presets/presentation/gargantua-cinematic-v1.toml` |
| `middle_gray_luminance_cd_m2` | `2411578982.805191` (Gate 2C1 median positive Y) |
| `exposure_ev` | `0.0` |
| Tone mapper | `khronos-pbr-neutral-v1` |
| Gamut mapper | `luminance-axis-desat-v1` |
| Display | sRGB IEC 61966-2-1 / RGB16 |
| PNG intent | Perceptual (0) |
| PNG gAMA | 45455 |
| cHRM / ICC | OMIT |

## EV ladder evidence (gate tier, fixed L_ref)

All five runs preserved exact Gate 2C1 `physical_color_digest`.

| EV | pre median luma | pre max RGB | gamut adj | post min | post max | code min | code max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| −2 | 0.045 | 3.92 | 0 | 0.00028 | 0.983 | 235 | 65039 |
| −1 | 0.090 | 7.85 | 0 | 0.00111 | 0.992 | 941 | 65307 |
| **0** | **0.180** | **15.69** | **0** | **0.00444** | **0.996** | **3634** | **65425** |
| +1 | 0.360 | 31.39 | 0 | 0.0178 | 0.998 | 9293 | 65481 |
| +2 | 0.720 | 62.77 | 0 | 0.0667 | 0.999 | 18767 | 65508 |

Canonical `EV=0` retained (middle-gray intent; no aesthetic retune).

## PNG / metadata verification

Gate beauty PNG: dimensions 128×128, RGB, 16-bit, sRGB Perceptual, gAMA=45455,
no cHRM, no ICC; decoded RGB16 raster exact to authored buffer; serial≡parallel
presentation digests and rasters on smoke.

## D0-C1 sRGB OETF numeric oracle

Independent hard-coded IEC 61966-2-1 vectors (not derived from production at
test time): `0`, `0.0031308`, `0.18`, `0.5`, `1.0`. Evaluator check
`hermetic_srgb_oetf_numeric_vectors` is required for `PRESENTATION_PIPELINE_PASS`.
Owner closure `5225838070`. Presentation digests unchanged vs pre-closure tip.

## Beauty review (128²)

Disk visible; hot inner structure retains rolloff under PBR Neutral; warm/cool
asymmetry present at gate resolution; no banding at RGB16; absence = display
black presentation fill. Visual review is presentation-only — not a physics claim.

## Architecture

P1 in-process: regenerate Gate 2C1 frame → present → PNG.
Pure math in `relativity-render::{presentation,tone_map,display_encoding}`.
Encoder (`png` 0.18.1, MIT OR Apache-2.0) in `xtask` only.

## Out of scope / stop

Auto exposure, Extended Reinhard, 8-bit derivative, `present-physical-color`,
bloom/glare, celestial environment, HDR10/PQ/HLG, LUT/AgX/ACES, E2/E3, GPU,
GUI. **Merge not authorized** — stop for owner review.
