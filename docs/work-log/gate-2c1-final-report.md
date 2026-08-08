# Gate 2C1 final report — physical colorimetry

## Result

**Authoritative evaluate PASS** on clean worktree after owner closure
`5225581548` (C1–C4).

| Field | Value |
| --- | --- |
| Evaluated tip | `480e6bfb2dfcc5cff797b4780c14ed68b93d2736` |
| Evaluation content digest | `b5727f26e56bbbe084bbda6f03f03cf0e1ad61dea9e101bbe9d41b4a81c19b96` |
| `result` | `PASS` |
| `authoritative` | `true` |
| `dirty` | `false` |
| Scope | `gate-2c1-colorimetry` |
| Planning / merge base | `57659c6202b8d8642891b5d0d88bce7d8f82f470` (PR #18 Gate 2C0) |
| PR | [#19](https://github.com/qwertyboy0325/blackhole-rust/pull/19) (draft) |

Implementation commits on branch `gate-2c1-physical-colorimetry`:

1. `226bda3` — Arch B colorimetry + vendored CIE CC BY-SA table + CLI/evaluator
2. `7e9fe82` — RGB matrix roundtrip check uses relative L1 on absolute photometric scale
3. `8cac462` — report-only PASS @ pre-closure tip (superseded digests)
4. `480e6bf` — closure C1–C4: runtime CIE, 360–830 band, raw/EXR authority

## Closure package (owner `5225581548`)

| ID | Issue | Resolution |
| --- | --- | --- |
| C1 | CIE licensed as separate CC BY-SA CSV, but `include_str!` baked table into binary | Runtime load from `assets/standards/cie1931-2deg-v1.csv` + SHA-256 pin; unit tests use synthetic CMFs |
| C2 | Production band was abridged 380–780 nm | Full official **360–830 nm @ 1 nm / 471 samples** (`cie-1931-360-830-1nm-v1`); Planckian xy direction check added |
| C3 | Digest hashed `OutcomeClass` but f64le lacked typed outcome | Schema-2 payload `BHRXYZR2`: presence + outcome u8 + XYZRGB; `payload_sha256` in meta; self-consistency check |
| C4 | EXR “exact roundtrip” only verified XYZRGB+mask | Roundtrip verifies all `phys.*` + `outcome` channels; frame `validate()` before digest |

`physical_color_digest` **changed** vs pre-closure (expected: band + digest tag v2 + outcome encoding).

## Architecture B provenance (production)

```text
PhysicalDiskEmissionFrame (F, T_eff, g)
  → B_ν(ν_obs/g, T_eff) at official CIE 1 nm nodes (λ→ν = c/λ), 360–830 nm
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
| Load mode | `runtime-vendored-asset` (not embedded) |
| Production band | 360–830 nm @ 1 nm (471 samples) |
| Repo code license | MIT OR Apache-2.0 (unchanged; table not re-licensed) |
| Acquisition | `VENDOR_CC_BY_SA_DATA` (owner 2026-08-08) |

## Gate digests (128², gargantua-physical-v1)

| Digest | Value |
| --- | --- |
| `physical_color_digest` | `16663188fad338c0fc8197dddd8268bd705f817b165a35853b16b211c7635793` |
| `payload_sha256` (`physical-xyz-rgb.f64le`) | `d317c517661a64f8ffdacead3dd222370056abc8eed81706d660bc4ebda81cf5` |
| EXR SHA-256 | `151035e5db938c863ca6463eda3f0b5c4cf7abb04e775b01185ef4329f6eb10a` |
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
| Runtime CIE load (not `include_str!`) | `hermetic_cie_runtime_load_not_include_str` + meta `cie_load_mode` |
| Production band 360–830 / 471 | `hermetic_production_471`, `hermetic_production_band_360_830` |
| ν↔λ agreement | hermetic envelope (tol `1e-5`) |
| Sampling ladder 10→1 nm | hermetic ladder improves toward 1 nm |
| Blackbody Y(T) | Y(3k) < Y(6.5k) < Y(10k) |
| Blackbody Planckian xy | `hermetic_blackbody_planckian_direction` |
| RGB matrix roundtrip | relative L1 hermetic |
| Schema-2 raw authority | `BHRXYZR2` + outcome; `raw_payload_self_consistent`; meta `payload_schema=2` |
| Serial ≡ parallel | smoke color digests + f64le byte-identical |
| Negative RGB | diagnostic count `0` on gate (allowed; not a failure) |
| Non-finite XYZ/RGB | fail-closed in `PhysicalColorFrame` / EXR f64→f32 checks; gate run finite |
| Clamp / CAT / exposure | banned (`NO_SCIENTIFIC_CLAMP_*`, no Bradford, `DEFER_NO_TONE_MAP`) |

## Interchange

| Item | Policy / result |
| --- | --- |
| Scientific authority | raw LE f64 schema-2 `physical-xyz-rgb.f64le` + `physical-colorimetry-meta.json` (`payload_sha256`) |
| EXR role | `DERIVED_INTERCHANGE_ARTIFACT` (`exr` 1.74.2, BSD-3) |
| EXR policy | FLOAT XYZ/RGB + phys.*; UINT `disk.mask`/`outcome`; **uncompressed**; no HALF/DWA |
| EXR roundtrip | f64→checked f32→write→decode→**exact f32** for XYZRGB **and** `phys.g/F/T/r_over_m` + `outcome` |
| Preview PNG | **DEFER** (no tone-map policy in 2C1) |

## A-vs-B diagnostic (not authority)

Gate comparison of Arch A (256-bin cube projection) vs Arch B (CIE 1 nm from emission):

- compared disk hits: `12307`
- max rel Y error ≈ `2.44e-3`
- max Δu′v′ ≈ `1.08e-3`

Sparse-cube under-sampling is expected; Arch B remains production colorimetry.

## Out of scope / stop

ACEScg, CAT, exposure/tone-map/OETF/PNG beauty, E2/E3, GPU/GUI, Issue #12
resume beyond dual-track note. **Merge not authorized** in this report — stop for owner review.
