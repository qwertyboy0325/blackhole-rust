# Gate 2C0 — Physical Thin-Disk Emission (Page–Thorne + Planck + g³)

**Status:** authoritative evaluate **PASS** on clean implementation tip — pending
owner merge review. Gate 2C1 (CIE/RGB/OpenEXR) is **not** authorized.

## Evaluated commit

| Field | Value |
| --- | --- |
| Commit | `551f69eaf932543f30698496ec379781e42de4f0` |
| Branch | `gate-2c0-physical-emission` |
| Base | `origin/main` @ `95c4062e5926e77e3e14c17ec003e7ee625cfc79` |
| Worktree at evaluate | clean (`dirty: false`) |
| Result | `PASS` (`authoritative: true`) |
| Evaluation content digest | `1d67819cdd07f0b8f75c6983cdc51d9a0412a99ba36ea2bac6c977b5a5e764db` |
| Checks | 38 / 38 PASS |
| Host | `aarch64-apple-darwin`, `rustc 1.96.0 (ac68faa20 2026-05-25)`, release |
| Authoritative threads | 16 |
| Evaluator wall (host shell) | ≈ 17.4 s (includes workspace tests + all renders) |
| Gate 128² render wall | `total_wall_clock_seconds ≈ 1.357` (trace 0.942 + spectral 0.298 + channel 0.034) |

Command:

```bash
cargo run --release -p xtask -- evaluate --scope gate-2c0-physical-emission
```

Artifact (gitignored): `artifacts/gate-2c0-physical-emission/gate-2c0-evaluate.json`.

## Physical model claim

| Knob | Authority |
| --- | --- |
| Emission model | `page-thorne-blackbody-v1` |
| Flux | Page–Thorne 1974 zero-torque at prograde ISCO, **one face** (`W m⁻²`) |
| Temperature | `T_eff = (F_one_face / σ_SB)^{1/4}` |
| Spectrum | isotropic `I_ν,em = B_ν(T_eff)` with mandatory `π ∫ B_ν = σ T⁴ = F` |
| Transport | reuse `transport_i_nu`: `I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs/g)` |
| Convention id | `physical-spectral-disk-g3-v1` |
| Constants | `codata-2018+iau-b3-2015-v1` |
| Preset | `presets/gargantua-physical-v1.toml` (`mass_solar=1e8`, `mdot_kg_s=1e18`) — project demonstration, **not** film/DNGR |
| Grid (gate) | `physical-spectral-grid-explore-256`, `[1e11, 1e17]` Hz (explore; **not frozen**) |

Frames: `PhysicalDiskEmissionFrame` + `PhysicalSpectralFrame`. Diagnostic
`SpectralFrame V1` / `spectral-grid-v1` are typed rejects (not Hz).

## Gate 2C0 scientific digests (128² gate run)

| Quantity | Digest |
| --- | --- |
| physical emission spec | `25604a7569b05b6d9e1d3f7188a579fc9f6ef4704d024621a86c42a2ae7d4e86` |
| physical emission frame (`F`, `T_eff`) | `b68490f1f00ef1795d26c5efcdb3525cb81d1648fc846c5c697be10e913ae138` |
| physical spectral grid (256) | `8592d596b56289986e6c66b0d459bbea35967fe16a2a789a284decb7b73ed820` |
| physical spectral frame (`I_ν,obs`) | `67bc17bae260dd19fbd70fbdf0c8b39d7f938568177774f817402f9203ba1dd0` |
| inherited Gate 2B0 frequency (same gate geometry) | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |

Units: `I_ν` = `W m⁻² Hz⁻¹ sr⁻¹`; `F` = `W m⁻²` (one face); `T_eff` = `K`.

## Inherited frozen authorities (verified / cited)

| Authority | Digest | This evaluate |
| --- | --- | --- |
| 2B2 `spectral-grid-v1` | `0d7e4812dfba61635aaf00f486fcc23aebc63fbb2fb9d6a51ab8a4b8ed41474e` | re-hashed PASS |
| 2B0 frequency | `65df7b55…a875c2` | gate run match PASS |
| 2B1 bolometric | `d3721de7…746b2` | reference present PASS |
| 2B2 continuum | `d2fec186…1b67f4` | frozen; not re-rendered |
| 2B2 spectral frame 128²×64 | `1271958c…fe18` | frozen; not re-rendered |

Diagnostic ν̃∈[0.25,4] is **not** SI Hz. Physical digests are a new authority.

## Closure metrics

### Gate 128² × 256 bins

| Metric | Value |
| --- | --- |
| disk hits / emission pixels | 12307 / 12307 |
| max rel emitter SB (`π∫B` vs `σT⁴(1−trunc)`) | `1.2135523489559788e-4` |
| max abs emitter SB | `5.441378977384567e5` |
| worst emitter pixel | `(90, 65)` |
| max rel g⁴ transport | `2.0635102930910416e-15` |
| max abs g⁴ transport | `3.725290298461914e-8` |
| worst transport pixel | `(16, 73)` |

Emitter SB is truncation-aware on the finite Hz band. Transport closure is at
machine-ε relative scale (same-band mapped `g³` samples).

### Smoke 32² serial ≡ parallel

| Metric | Value |
| --- | --- |
| spectral digest | `291e2375787279930b7a67c8f60723e12a3a3297a0786eb1746bd9267ab8a7f2` |
| emission digest | `d6ad7279db44ff9655258a2de7f92d530da62db1c0bafcaa1368540510b7084c` |
| emission pixels | 770 |
| max rel emitter SB | `1.9427443492103028e-3` |
| max rel g⁴ | `1.0123627179238193e-15` |
| payload byte identity | PASS (`physical-f-teff.f64le`, `physical-i-nu-obs.f64le`) |

### Grid convergence (smoke 32²)

| bins | max rel emitter SB | physical spectral digest |
| ---: | --- | --- |
| 64 | `1.9427443492103028e-3` | `291e2375…ab8a7f2` |
| 128 | `4.8547395868088304e-4` | `fdfc3d38…c2d7fe` |
| 256 | `1.213552347158568e-4` | `4f3e7779…0701315` |
| 512 | `3.033797993586108e-5` | `7124c8f2…85388a2` |

Emitter SB improves ~4× per bin doubling (64→512). Bin count remains
**explore**, not frozen.

## Hermetic physics checks (subset)

| Check | Detail |
| --- | --- |
| `π B_ν` Stefan–Boltzmann | rel ≈ `8.4e-7` |
| missing π fails SB | PASS |
| `g³` transport identity | exact match on fixture |
| PT → Newtonian asymptote | rel ≈ `4.8e-3` |
| PT algebraic vs numerical Q | `qa≈0.771`, `qn≈0.778` |
| PT vanishes near ISCO | near ≪ mid |
| `Ṁ→0` | `F=0` |
| retrograde | typed reject |
| reject diagnostic grid | CLI + hermetic PASS |
| `T_eff` SB round-trip | rel ≈ `3.3e-16` |

## Non-finite / negative / invalid-state counts

Post-hoc scan of gate `physical-i-nu-obs.f64le` and `physical-f-teff.f64le`:

| Payload | Hits | Non-finite | Negative scientific values |
| --- | ---: | ---: | ---: |
| `I_ν,obs` cube | 12307 | **0** | **0** |
| `F` / `T_eff` / `g` / `r` | 12307 | **0** | **0** (`F`, `T`) |

Absence pixels are typed (`mask=0`) with zero-filled spectra — not painted as
horizon/sky. Invalid emission / diagnostic-grid CLI paths fail closed.

## Determinism

- Smoke serial digest = smoke parallel digest.
- Smoke physical payload bytes identical across serial/parallel.
- Timing fields excluded from evaluation content digest.

## Scope exclusions (explicit)

```text
NOT IN GATE 2C0:
  CIE 1931 XYZ / CMF, scene-linear RGB, ACEScg, OpenEXR, PNG authority
  f_col, limb darkening, atmosphere, returning radiation, Comptonization
  physical celestial sky, GPU / wgpu / egui / GUI
  E2 / E3, mutating 2B0–2B2 digests, treating diagnostic ν as Hz
```

Meta tags: `colorimetry: absent-deferred-to-gate-2c1`. No `colorimetry.rs`,
CIE dataset, or `exr` crate in this branch’s scientific path.

## Owner stop

```text
Gate 2C0 implementation     DONE @ 551f69e
Gate 2C0 authoritative eval PASS (this report)
Gate 2C0 merge              NOT YET
Gate 2C1                    NOT AUTHORIZED
```

Review focus: PT one-face claim, π-normalization, Hz units/Jacobian vs
diagnostic grid, `g³` reuse, truncation-aware SB, colorimetry absence.
