# Gate 2B2 — Sampled Spectral Specific-Intensity Transport

**Status:** implementation complete; authoritative evaluate pending clean worktree commit.

## Scope

Diagnostic continuum `I_ν` transport with vacuum law

```text
I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs / g)
```

scaled from accepted Gate 2B1 bolometric `I_em,bol`. Introduces `SpectralFrame V1`
in `relativity-render`. Does **not** mutate `OracleFrame V1` / E0 lock / 2B0–2B1 digests.

## Inherited authorities (verified on gate 128²)

| Authority | Digest |
| --- | --- |
| Gate 2B0 frequency | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |
| Gate 2B1 bolometric | `d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2` |

## Gate 2B2 scientific digests (gate-run-0, dirty evaluate)

| Quantity | Digest |
| --- | --- |
| continuum `diagnostic-lognormal-continuum-v1` | `d2fec186266441c297ad41b307d0cdc0a47603c22765ed1f16dff6c6dc1b67f4` |
| `spectral-grid-v1` | `0d7e4812dfba61635aaf00f486fcc23aebc63fbb2fb9d6a51ab8a4b8ed41474e` |
| spectral frame (128² × 64) | `1271958c2f82537722f2748e8e2c09813c594539a00f911acc7da51e7b45fe18` |

Re-pin after the first clean-worktree release evaluate.

## Closure (truncation-aware)

Integrals compared to Gate 2B1 bolo × captured continuum mass
`M_capt = ∫_{[ν_min/g,ν_max/g]∩domain} φ dν`. Emitted uses observer-grid Jacobian
`dν_em = dν_obs / g`.

Gate 128²: max relative closure ≈ `6.3e-4` (both emitted and observed).

## Convergence (smoke 32²)

| bins | max rel obs closure |
| --- | --- |
| 32 | 1.47e-3 |
| 64 | 6.10e-4 |
| 128 | 2.44e-4 |
| 256 | 1.06e-4 |

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

Ready for commit + clean release evaluate. Do not merge until owner accepts digests
and claim boundary.
