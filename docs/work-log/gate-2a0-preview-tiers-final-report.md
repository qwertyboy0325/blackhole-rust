# Gate 2A0-4 Final Report — Named Preview Quality Tiers

## Status

Authoritative `evaluate --scope gate-2a0-preview-tiers` **PASS** at tip `8f39b41`.

## 1. Base / branch

- Base: `980046ae21a4d1be50f6abcf1e0212eb1e63893c` (Gate 2A0-3 merge)
- Branch: `gate-2a0-preview-quality-tiers`
- Implementation tip: `8f39b414cad1044ae759acdc5aad541629f5c645`

## 2. Tier table

| Tier | Dimensions | Authority |
|---|---:|---|
| `smoke` | 32×32 | non-authoritative |
| `preview` | 64×64 | non-authoritative |
| `gate` | 128×128 | authoritative candidate |
| `showcase` | 256×256 | non-authoritative |
| custom | explicit W×H | always non-authoritative |
| legacy-default (no args) | 128×128 | non-authoritative |

## 3. CLI compatibility

- `--tier NAME` derives dimensions; rejects `--width`/`--height`
- `--width` / `--height` remain custom; partial override keeps other axis at 128
- No tier and no dimensions → legacy 128×128 (`legacy-default`, non-authoritative)
- Only explicit `--tier gate` is an authoritative candidate
- Safety limit 4096×4096; zero / overflow rejected before allocation

## 4. Numerical profile

- `profile_id`: `gate-1b2-diagnostic-v1`
- Built by `build_diagnostic_trace_scene` (single scene builder for all tiers)
- Digest (bit-pattern): `af0041d388c61576e18a400a4f35a4220bd4981d34a05a42dacb6e77d97e888b`
- Shared by smoke / preview / gate / showcase / custom

## 5. Only grid dimensions vary

Integrator tolerances, arming, horizon proximity, camera, disk, escape radius, and one sample per pixel center are identical across tiers. Tier selection changes `TraceGrid.width/height` only.

## 6. Authority classification (observed)

| Run | `authority_class` |
|---|---|
| smoke | non-authoritative |
| preview | non-authoritative |
| gate ×2 | authoritative-candidate |
| showcase | non-authoritative |
| custom 128×128 | non-authoritative |

## 7. Timing (16 threads; informational)

| Tier | Rays | Trace (s) | Shade (s) | Rays/s |
|---|---:|---:|---:|---:|
| smoke | 1024 | 0.058 | 0.000021 | ~17800 |
| preview | 4096 | 0.178 | 0.000063 | ~23000 |
| gate | 16384 | 0.832 | 0.000233 | ~19700 |
| showcase | 65536 | 2.982 | 0.001115 | ~22000 |
| custom-128 | 16384 | 0.720 | 0.000228 | ~22800 |

Monotonic wall-clock: smoke < preview < gate < showcase.

## 8. Gate 1B2 reference (`--tier gate` 128×128)

| Channel | Status |
|---|---|
| classification `64462a83…52c4` | MATCH |
| categorical PPM `ac058d5a…184c` | MATCH |
| RHS PGM `2df22639…5db5` | MATCH |
| counts (disk 12307 / escaped 2442 / hz_event 1462 / hz_approach 173 / affine 0 / failed 0) | MATCH |
| disk-suppressed Δ pixels | 12307 |

## 9. Gate subprocess determinism

Two independent `--tier gate` subprocesses: identical trace-data / class / PPM / PGM / counts / step totals.

## 10. Custom-128 authority negative

`--width 128 --height 128` → `resolution_source=custom-dimensions`, `authority_class=non-authoritative`, `render_tier=null`.

## 11–12. Evaluator digest / authority

- `result: PASS`
- `authoritative: true`
- `dirty: false`
- commit: `8f39b414cad1044ae759acdc5aad541629f5c645`
- content digest: `3734bd742bfa9adc943d06e09f78bc994f14068d11088b0da0a01f89e29ebffc`

## 13–14. CI / exclusions

- Local fmt / clippy / workspace tests PASS (evaluator)
- No reduced-tolerance preview mode; no celestial-sphere / radiometry / GPU / AA / multi-sample

Stop at Gate 2A0-4 boundary for owner review.
