# Gate 2B2 — Sampled Spectral Specific-Intensity Transport

**Status:** authoritative evaluate **PASS** — pending owner review (do not merge).

## Evaluated commit

| Field | Value |
| --- | --- |
| Commit | `07c51116db0dd35b2092fd69418e7de51ae0bce1` |
| Branch | `gate-2b2-spectral-transport` |
| Result | `PASS` (`authoritative: true`) |
| Evaluation content digest | `49a2ddbbc6716ce9745ab85659662105844b74b677d1f9f8f538d72af4526eea` |

## Scope

Diagnostic continuum `I_ν` transport with vacuum law

```text
I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs / g)
```

scaled from accepted Gate 2B1 bolometric `I_em,bol`. Introduces `SpectralFrame V1`
in `relativity-render`. Does **not** mutate `OracleFrame V1` / E0 lock / 2B0–2B1 digests.

## Inherited authorities (gate 128²)

| Authority | Digest |
| --- | --- |
| Gate 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |
| Gate 2B1 bolometric | `d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2` |

## Gate 2B2 scientific digests (gate-run-0 @ evaluated commit)

| Quantity | Digest |
| --- | --- |
| continuum `diagnostic-lognormal-continuum-v1` | `d2fec186266441c297ad41b307d0cdc0a47603c22765ed1f16dff6c6dc1b67f4` |
| `spectral-grid-v1` | `0d7e4812dfba61635aaf00f486fcc23aebc63fbb2fb9d6a51ab8a4b8ed41474e` |
| spectral frame (128² × 64) | `1271958c2f82537722f2748e8e2c09813c594539a00f911acc7da51e7b45fe18` |

## Closure (truncation-aware)

Integrals compared to Gate 2B1 bolo × captured continuum mass

```text
M_capt = ∫_{[ν_min/g, ν_max/g] ∩ domain} φ dν
dν_em = dν_obs / g   (emitted measure on observer grid)
```

| Metric (128² gate) | Value |
| --- | --- |
| disk hits | 12307 |
| max rel emitted closure | `6.324195281101354e-4` |
| max rel observed closure | `6.32419528109540e-4` |
| max abs emitted | `2.7489031555305576e-4` |
| max abs observed | `3.950266174806982e-6` |
| worst source index | 8469 |

## Convergence (smoke 32²)

| bins | grid id | max rel obs closure |
| --- | ---: | --- |
| 32 | `spectral-grid-explore-32` | `1.4729876514599062e-3` |
| 64 | `spectral-grid-v1` | `6.09991030106971e-4` |
| 128 | `spectral-grid-explore-128` | `2.4449675084819517e-4` |
| 256 | `spectral-grid-explore-256` | `1.061740022903296e-4` |

64-bin `spectral-grid-v1` retained as provisional authoritative grid.

## Artifacts / CLI

```bash
cargo run --release -p xtask -- render-disk-spectrum \
  --preset presets/gargantua-baseline.toml --tier gate \
  --spectrum diagnostic-lognormal-continuum-v1 \
  --spectral-grid spectral-grid-v1 \
  --output-dir artifacts/gate-2b2-spectral-transport \
  --execution parallel --threads N --require-release

cargo run --release -p xtask -- evaluate --scope gate-2b2-spectral-transport
```

## Explicit non-claims

No temperature/Planck, no physical RGB, no OpenEXR, no CIE, no GPU, no E2/E3.

## Owner stop

Authoritative PASS recorded. Awaiting owner acceptance. Do not merge; do not start
physical RGB, OpenEXR, or E2.
