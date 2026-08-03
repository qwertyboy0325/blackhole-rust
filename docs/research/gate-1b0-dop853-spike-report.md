# Gate 1B0 DOP853 spike report

**Status:** evidence from executable spike (non-production)  
**Date:** 2026-08-03  
**Branch:** `gate-1b0-dop853-spike`

## Candidates (pinned)

| Crate | Version | License |
|---|---|---|
| `ode_solvers` | `=0.6.1` | Apache-2.0 |
| `ivp` | `=0.6.0` | Apache-2.0 |

## Experiment matrix

Both candidates ran experiments A–G under `spikes/` with shared JSON schema
(`gate-1b0-contract`, schema `gate-1b0-v1`).

| ID | Finding (summary) |
|---|---|
| A | Both integrate `y'=λy`; endpoint + dense probes; determinism x5 in-process |
| B | SHO endpoint/energy; stop+restart exercised |
| C | 8D mixed scales; ivp direct `Tolerance::Vector`; ode scalar + adapter rescale |
| D | ivp: `SolOut` + `StepInterpolant` at accepted step; ode: `solout` without interpolant, dx-grid dense |
| E | Known zero crossing localized; ivp via `sol(t)`; ode via dense sample bracket |
| F | Static `h_max`; domain NaN RHS; callback stop |
| G | Short Kerr Hamiltonian probe (weak-field ZAMO ray); no geodesic claims |

## Decision matrix (enum labels)

See `artifacts/gate-1b0/comparison.json` after authoritative evaluate.

Key differentiators observed:

- **Vector tolerance:** ivp `Supported`; ode_solvers `Unsupported` (adapter rescale only)
- **Accepted-step dense interpolation:** ivp `Supported` (`StepInterpolant` in `SolOut`); ode_solvers `SupportedWithAdapter` (fixed dx grid; private `rcont`)
- **Event localization:** ivp `Supported` (dense `sol(t)`); ode_solvers `SupportedWithAdapter` (dense grid bracket)

## ADR 0005 recommendation

**Remain `Proposed`.** Spike favors `ivp` on contract fit (vector tol + accepted-step interpolant), but owner acceptance requires review of spike JSON digests and Gate 1B1 adapter scope. Do not add production ODE dependency in Gate 1B0.

## Reproduce

```bash
cargo xtask spike-dop853 --candidate ode-solvers
cargo xtask spike-dop853 --candidate ivp
cargo xtask evaluate --scope gate-1b0
```

Authoritative PASS requires clean worktree (porcelain including untracked).

## Artifact schema

- `artifacts/gate-1b0/ode-solvers.json` — `CandidateReport`
- `artifacts/gate-1b0/ivp.json` — `CandidateReport`
- `artifacts/gate-1b0/comparison.json` — `ComparisonReport`
- `artifacts/gate-1b0/evaluation.json` — gate orchestration + digests

Digests recorded in evaluation JSON after authoritative run.

## Gate 1B1 scope (recommended)

- Production adapter spike on chosen candidate only after ADR 0005 owner acceptance
- Prove component-scaled tolerances against stratified Hamiltonian corpus
- Wire accepted-step event localizer to project-owned root finder
- No image rendering or full ray tracing
