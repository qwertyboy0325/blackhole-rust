# Gate 2C0 — Physical Thin-Disk Emission (Page–Thorne + Planck + g³)

**Status:** authoritative evaluate **PASS** after owner closure `5225301622`
physics root fix — pending owner merge review. Gate 2C1 **not** authorized.

Prior PASS @ `551f69e` remains **INVALIDATED** (wrong `F∝Q`, non-independent
oracle). Corrected digests differ as required.

## Evaluated commit

| Field | Value |
| --- | --- |
| Commit | `a760427239edf177d3ed775769c17ccd776d0b0c` |
| Branch | `gate-2c0-physical-emission` |
| Base | `origin/main` @ `95c4062e5926e77e3e14c17ec003e7ee625cfc79` |
| Worktree at evaluate | clean (`dirty: false`) |
| Result | `PASS` (`authoritative: true`) |
| Evaluation content digest | `209bddb1dc7cbfc251cab2a09473e7e6318d62f9393a4d630d6a2bdef18c7097` |
| Checks | 40 / 40 PASS |
| Owner closure addressed | `5225301622` |
| Host | `aarch64-apple-darwin`, `rustc 1.96.0`, release, 16 threads |
| Evaluator wall (host) | ≈ 26.5 s |
| Gate 128² render wall | `total ≈ 0.845 s` |

## Closure `5225301622` fixes

| Item | Resolution |
| --- | --- |
| P0 `F∝Q` missing `1/(B√C)` | `F = (3GMṀ)/(8πr³)·Q/(B√C)` |
| P0b numerical oracle | conservation-law **flux** with `−Ω_,r` in numerator; compare flux not `Q` |
| C1 truncation | captured / (`σT⁴/π`); no finite-band total |
| C2 frames / closure | validated `Deserialize` + `try_new`; independent abs/rel maxima + lowest-index ties |
| C3 freeze | `physical-spectral-grid-v1` (256); smoke tol `6e-3`; gate tol `5e-4`; PT flux tol `5e-3` |

## Physical model claim

| Knob | Authority |
| --- | --- |
| Emission model | `page-thorne-blackbody-v1` |
| Flux | Page–Thorne 1974 zero-torque, **one face**, `F∝Q/(B√C)` |
| Temperature | `T_eff = (F/σ_SB)^{1/4}` |
| Spectrum | `I_ν,em = B_ν(T)` with mandatory `π∫B = σT⁴` |
| Transport | `transport_i_nu`: `I_ν,obs = g³ I_ν,em(ν_obs/g)` |
| Grid | **frozen** `physical-spectral-grid-v1` (256 log bins, `[1e11,1e17]` Hz) |
| Preset | `gargantua-physical-v1.toml` (`mass_solar=1e8`, `mdot_kg_s=1e18`) |

## Gate 2C0 scientific digests (128² × `physical-spectral-grid-v1`)

| Quantity | Digest |
| --- | --- |
| physical emission spec | `25604a7569b05b6d9e1d3f7188a579fc9f6ef4704d024621a86c42a2ae7d4e86` |
| physical emission frame | `5c0bc7c2f93893069712301e6ba1f9883764a11ffdeaa98f859738ee22df6a6f` |
| physical spectral grid v1 | `ceb3db28082bb357e50cac2635b221711bf79ea2806f2c25b60c61ca901162d5` |
| physical spectral frame | `685cdb830670c983369be9808628334314f13930671354f0eadd8f6fd5508614` |
| inherited 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |

**Digest change vs invalidated `551f69e`:** emission `b68490f1…` → `5c0bc7c2…`;
spectral `67bc17ba…` → `685cdb83…` (expected after flux root fix).

## Inherited frozen authorities

| Authority | Digest | This evaluate |
| --- | --- | --- |
| 2B2 `spectral-grid-v1` | `0d7e4812…41474e` | re-hash PASS |
| 2B0 frequency | `65df7b55…a875c2` | gate match PASS |
| 2B1 bolometric | `d3721de7…746b2` | reference present |
| 2B2 continuum / spectral frame | frozen | not re-rendered |

## Closure metrics (gate 128² × 256)

| Metric | Value |
| --- | --- |
| emission pixels | 12307 |
| max rel / abs emitter SB | `1.21355e-4` / `7.38774e5` |
| worst rel / abs emitter | `(90,65)` / `(80,85)` **independent** |
| max rel / abs g⁴ | `2.04e-15` / `8.58e-6` |
| worst rel / abs transport | `(111,69)` / `(93,64)` **independent** |
| frozen gate SB tol | `5e-4` |
| algebraic vs numerical PT flux (a*=0.999 domain) | worst rel ≈ `1.9e-8` |

## Convergence (smoke 32² explore ladder)

| bins | max rel emitter SB |
| ---: | --- |
| 64 | `1.9427e-3` (smoke tol `6e-3`) |
| 128 | `4.8547e-4` |
| 256 | `1.2136e-4` |
| 512 | `3.0338e-5` |

## Determinism / validity

- Smoke serial ≡ parallel spectral digest `1a107993…d20e5e` + payload byte identity
- Gate `I_ν` / `F,T` payloads: **0** non-finite, **0** negative scientific values
- Colorimetry: `absent-deferred-to-gate-2c1` (no CIE/EXR in path)

## Scope exclusions

```text
NOT IN GATE 2C0: CIE/XYZ/RGB/OpenEXR, f_col, limb darkening, returning radiation,
physical celestial sky, GPU/GUI, E2/E3, mutating 2B0–2B2 digests
```

## Owner stop

```text
Gate 2C0 previous PASS @ 551f69e   INVALIDATED
Gate 2C0 physics root fix           DONE @ a760427
Gate 2C0 authoritative eval         PASS
Gate 2C0 merge                      NOT YET
Gate 2C1                            NOT AUTHORIZED
```
