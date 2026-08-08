# Gate 2C1 final report — physical colorimetry

## Result

**Authoritative evaluate PASS** on clean worktree.

| Field | Value |
| --- | --- |
| Evaluated tip | `7e9fe829e64024e1dd50e8c20cb86157d32f9976` |
| Evaluation content digest | `a09ae8a8a3ad6b4260551a95201598ae27212a83cb357ae82af91194f08266a4` |
| `result` | `PASS` |
| `authoritative` | `true` |
| `dirty` | `false` |
| Scope | `gate-2c1-colorimetry` |
| Planning / merge base | `57659c6202b8d8642891b5d0d88bce7d8f82f470` (PR #18 Gate 2C0) |

Implementation commits on branch `gate-2c1-physical-colorimetry`:

1. `226bda3` — Arch B colorimetry + vendored CIE CC BY-SA table + CLI/evaluator
2. `7e9fe82` — RGB matrix roundtrip check uses relative L1 on absolute photometric scale

## Architecture B provenance (production)

```text
PhysicalDiskEmissionFrame (F, T_eff, g)
  → B_ν(ν_obs/g, T_eff) at official CIE 1 nm nodes (λ→ν = c/λ)
  → g³ via transport_i_nu / independent_physical_i_nu_obs
  → absolute CIE 1931 2° XYZ with K_m = 683 lm/W (Y in cd/m²)
  → scene-linear Rec.709/D65 RGB (IEC matrix, no OETF)
```

XYZ is **not** integrated from the Gate 2C0 frozen 256-bin `PhysicalSpectralFrame`.
That cube is **DIAGNOSTIC_ONLY** (Architecture A comparison).

## Data authority — CIE table

| Item | Value |
| --- | --- |
| DOI | [10.25039/CIE.DS.xvudnb9b](https://doi.org/10.25039/CIE.DS.xvudnb9b) |
| md5 | `17cca777db64b17170f06f67ce9d3ab7` |
| SHA-256 | `fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1` |
| License | **CC BY-SA 4.0** (`assets/standards/LICENSE-CIE-CC-BY-SA-4.0.txt` + `NOTICE`) |
| Modifications | **NONE** (byte-identical to official `CIE_xyz_1931_2deg.csv`) |
| Repo code license | MIT OR Apache-2.0 (unchanged; table not re-licensed) |
| Acquisition | `VENDOR_CC_BY_SA_DATA` (owner 2026-08-08) |

## Gate digests (128², gargantua-physical-v1)

| Digest | Value |
| --- | --- |
| `physical_color_digest` | `2a4ae7143fd59f25fcdde0efea3afc49fce6e83480091b4404875c5f51640504` |
| CIE table SHA-256 | `fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1` |
| RGB matrix digest | `e6bbfa40f33759e93bc9cf5d29a6b73d5e5a82fe8f66c0e6a7e7c4c95fad015e` |
| Gate 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |
| Gate 2C0 emission | `5e3b15023df9bf3debed9666d65a3c762cfe83fe9885e7a5c8b3565dc19a383e` |
| Gate 2C0 spectral | `136b1fbcc76beb08ea38aa24d16803d621da20bad5b7ebfecc7a13c260aa8dd1` |
| `physical-spectral-grid-v1` | `ceb3db28082bb357e50cac2635b221711bf79ea2806f2c25b60c61ca901162d5` |

All four inherited 2C0/2B0 digests **exact-match** frozen authorities.

## Numerical / hermetic evidence

| Check | Evidence |
| --- | --- |
| ν↔λ agreement | max rel Y ≈ `3.27e-6` (tol `1e-5`) |
| Sampling ladder 10 nm | rel vs 1 nm ≈ `3.35e-4` (tol `5e-2`); 5/2/1 nm improves |
| Blackbody Y(T) | Y(3k) < Y(6.5k) < Y(10k) |
| RGB matrix roundtrip | rel L1 ≈ `2.8e-16` |
| Serial ≡ parallel | smoke color digests + `physical-xyz-rgb.f64le` byte-identical |
| Negative RGB | diagnostic count `0` on gate (allowed; not a failure) |
| Non-finite XYZ/RGB | fail-closed in `PhysicalColorFrame` / EXR f64→f32 checks; gate run finite |
| Clamp / CAT / exposure | banned (`NO_SCIENTIFIC_CLAMP_*`, no Bradford, `DEFER_NO_TONE_MAP`) |

## Interchange

| Item | Policy / result |
| --- | --- |
| Scientific authority | raw LE f64 `physical-xyz-rgb.f64le` + `physical-colorimetry-meta.json` |
| EXR role | `DERIVED_INTERCHANGE_ARTIFACT` (`exr` 1.74.2, BSD-3) |
| EXR policy | FLOAT XYZ/RGB + phys.*; UINT `disk.mask`/`outcome`; **uncompressed**; no HALF/DWA |
| EXR roundtrip | f64→checked f32→write→decode→**exact f32** (render-time verify + unit test) |
| Preview PNG | **DEFER** (no tone-map policy in 2C1) |

## A-vs-B diagnostic (not authority)

Gate comparison of Arch A (256-bin cube projection) vs Arch B (CIE 1 nm from emission):

- compared disk hits: `12307`
- max rel Y error ≈ `2.43e-3`
- max Δu′v′ ≈ `1.13e-3`

Sparse-cube under-sampling is expected; Arch B remains production colorimetry.

## Out of scope / stop

ACEScg, CAT, exposure/tone-map/OETF/PNG beauty, E2/E3, GPU/GUI, Issue #12
resume beyond dual-track note. **Merge not authorized** in this report.
