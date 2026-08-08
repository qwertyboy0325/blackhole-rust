# Gate 2B2 — Sampled Spectral Specific-Intensity Transport

**Status:** authoritative evaluate **PASS** after owner closures `5203577417` +
`5224835191` — pending final merge review.

## Evaluated commit

| Field | Value |
| --- | --- |
| Commit | `3499d8e51535bf809967c975bfccc7c6fbd45dfc` |
| Branch | `gate-2b2-spectral-transport` |
| Result | `PASS` (`authoritative: true`) |
| Evaluation content digest | `2a1b4d0f1ef75b45ad235b2e8f27859f80853600c308dee2020c48dd4722cac2` |
| Owner closures | `5203577417`, `5224835191` |

## Scope

```text
I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs / g)
dν_em = dν_obs / g
closure target = I_bolometric × M_capt
```

`SpectralFrame V1` only. OracleFrame V1 / E0 / 2B0–2B1 digests unchanged.
Spectral digest unchanged across F1–F3.

## Inherited authorities (gate 128²)

| Authority | Digest |
| --- | --- |
| Gate 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |
| Gate 2B1 bolometric | `d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2` |

## Gate 2B2 scientific digests

| Quantity | Digest |
| --- | --- |
| continuum | `d2fec186266441c297ad41b307d0cdc0a47603c22765ed1f16dff6c6dc1b67f4` |
| `spectral-grid-v1` (64, frozen) | `0d7e4812dfba61635aaf00f486fcc23aebc63fbb2fb9d6a51ab8a4b8ed41474e` |
| spectral frame 128²×64 | `1271958c2f82537722f2748e8e2c09813c594539a00f911acc7da51e7b45fe18` |

## Closure metrics (gate 128²)

| Metric | Value |
| --- | --- |
| disk hits | 12307 |
| max rel emitted / observed | `6.324195281101354e-4` / `6.32419528109540e-4` |
| max abs emitted / observed | `4.4941654586516666e-4` / `1.988171424682328e-3` |
| worst abs emitted / rel emitted | `12601` / `8469` |
| worst abs observed / rel observed | `8282` / `8469` |
| frozen `CLOSURE_REL_TOL` / `CLOSURE_ABS_TOL` | `2e-3` / `2e-3` |

Ties use strict `>` updates → **lowest source index** wins.

## Absolute tolerance calibration (F3)

Measured gate max abs observed = `1.988171424682328e-3`.

```text
CLOSURE_ABS_TOL = 2e-3
headroom = (2e-3 − 1.9881714e-3) / 2e-3 ≈ 0.59%
```

Rationale: absolute and relative budgets share one frozen ceiling so a single
owner-visible envelope covers both authorities. The absolute ceiling is set
just above the measured peak (not loosened for float noise). Relative peak
`≈6.32e-4` retains ~3× margin under the same `2e-3` number. Do not raise the
ceiling without new measurement evidence.

## Convergence ladders (smoke 32²; emitted + observed both freeze)

| bins | max rel obs | max rel em |
| --- | --- | --- |
| 32 | `1.4729876514599062e-3` | `1.4729876514595849e-3` |
| 64 | `6.09991030106971e-4` | `6.099910301073684e-4` |
| 128 | `2.4449675084819517e-4` | `2.444967508481222e-4` |
| 256 | `1.061740022903296e-4` | `1.0617400229039688e-4` |

Both ladders pass ≥2× (32→64), improve + non-accelerate (64→128, 128→256), and
`e64 ≤ 2e-3`.

## 64 / 128 / 256 cost trade-off (F3)

Smoke 32² payload + spectral-channel wall times from this evaluate:

| bins | `spectral-i-nu-obs.f64le` | spectral wall (s) | vs 64 |
| --- | ---: | ---: | --- |
| 64 | 525 336 B (~0.50 MiB) | 0.00133 | 1× |
| 128 | 1 049 624 B (~1.00 MiB) | 0.00262 | ~2.0× size, ~2.0× spectral time |
| 256 | 2 098 200 B (~2.00 MiB) | 0.00617 | ~4.0× size, ~4.6× spectral time |

Gate 128² × 64 payload = 8 405 016 B (~8.0 MiB); spectral wall ≈ 0.022 s
(host-local; excluded from digests).

Memory scaling for full observer cube ≈ `W·H·n_bins·8` bytes (plus mask byte):
128²×64 ≈ 8 MiB, ×128 ≈ 16 MiB, ×256 ≈ 32 MiB.

**Freeze at 64:** meets both ladder freezes and both error budgets; 128/256
halve relative error again but double/quadruple spectral cost and artifact
size without changing scientific claim (`g³` + `M_capt`). 64 is the coarsest
grid that clears the frozen acceptance rule.

## Authority closures

| ID | Status |
| --- | --- |
| `5203577417` (abs⊥rel, validated types, artifacts, provisional→freeze) | closed |
| `5224835191` F1 four worst indices + lowest-index tie | closed |
| `5224835191` F2 emitted+observed freeze ladders | closed |
| `5224835191` F3 abs calibration + cost rationale | closed (this report) |

## Explicit non-claims

No temperature/Planck, physical RGB, OpenEXR, CIE, GPU, E2/E3.

## Owner stop

Awaiting final merge decision on PR #17. Do not start RGB / OpenEXR / E2.
