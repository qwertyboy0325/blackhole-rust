# Gate 2B2 — Sampled Spectral Specific-Intensity Transport

**Status:** authoritative evaluate **PASS** after owner closure `5203577417` — pending merge decision.

## Evaluated commit

| Field | Value |
| --- | --- |
| Commit | `aa28e7d555d6fb0f38d64bf2d5a6d1a6bef449d4` |
| Closure implementation | `aff07ef88fb2e3be3e71c098ae53436be3a47150` |
| Branch | `gate-2b2-spectral-transport` |
| Result | `PASS` (`authoritative: true`) |
| Evaluation content digest | `5401924a38a70c7cce39785144a612dbfba4f8dd094029ef287ec86999971336` |
| Owner closure | `5203577417` |

## Scope

Diagnostic continuum `I_ν` transport with vacuum law

```text
I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs / g)
dν_em = dν_obs / g
closure target = I_bolometric × M_capt
```

`SpectralFrame V1` in `relativity-render`. Does **not** mutate `OracleFrame V1` /
E0 lock / 2B0–2B1 digests.

## Inherited authorities (gate 128²)

| Authority | Digest |
| --- | --- |
| Gate 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |
| Gate 2B1 bolometric | `d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2` |

## Gate 2B2 scientific digests (gate-run-0)

| Quantity | Digest |
| --- | --- |
| continuum `diagnostic-lognormal-continuum-v1` | `d2fec186266441c297ad41b307d0cdc0a47603c22765ed1f16dff6c6dc1b67f4` |
| `spectral-grid-v1` (64 bins, **frozen**) | `0d7e4812dfba61635aaf00f486fcc23aebc63fbb2fb9d6a51ab8a4b8ed41474e` |
| spectral frame (128² × 64) | `1271958c2f82537722f2748e8e2c09813c594539a00f911acc7da51e7b45fe18` |

## Closure metrics (truncation-aware; abs ⊥ rel)

| Metric (128² gate) | Value |
| --- | --- |
| disk hits | 12307 |
| max rel emitted | `6.324195281101354e-4` |
| max rel observed | `6.32419528109540e-4` |
| max abs emitted | `4.4941654586516666e-4` |
| max abs observed | `1.988171424682328e-3` |
| worst rel source index | 8469 |
| frozen `CLOSURE_REL_TOL` / `CLOSURE_ABS_TOL` | `2e-3` / `2e-3` |

`max_abs_*` and `max_rel_*` are tracked independently (closure `5203577417`).

## Convergence + grid freeze (smoke 32²)

| bins | grid id | max rel obs |
| --- | ---: | --- |
| 32 | explore | `1.4729876514599062e-3` |
| 64 | **spectral-grid-v1** | `6.09991030106971e-4` |
| 128 | explore | `2.4449675084819517e-4` |
| 256 | explore | `1.061740022903296e-4` |

Selection rule (frozen): `e64 ≤ e32/2`, `e64 ≤ 2e-3`, `e128 ≤ e64`,
`(e64/e128) ≤ 1.25(e32/e64)`, `e256 ≤ e128`, `(e128/e256) ≤ 1.25(e64/e128)`.

## Authority closures addressed (`5203577417`)

1. Closure metric: independent `max_abs_*` / `max_rel_*`.
2. Validated spectral types: `Frequency`/`Wavelength`/`SpectralGrid` deser runs
   validation; grid rechecks bounds, monotonic edges, weight/center consistency.
3. Artifacts: `bolometric-relative-error.pgm` vs `I_obs_bol × M_capt`; CSV includes
   `m_capt`, `nu_*`, `i_nu_obs_*`.
4. Grid/tolerance freeze: 64-bin `spectral-grid-v1` + `2e-3` budget + 128→256 checks.

## Explicit non-claims

No temperature/Planck, no physical RGB, no OpenEXR, no CIE, no GPU, no E2/E3.

## Owner stop

Awaiting merge decision on PR #17. Do not start physical RGB, OpenEXR, or E2.
