# Gate 1B0 final report (remediation)

## Identity

- Branch: `gate-1b0-dop853-spike`
- Rebased base: Gate 1A merge `dc38619` on `main` (PR #1 merged)
- PR #2: Gate 1B0-only delta (7 commits on top of `main`)
- Authoritative commit: `5180cfe42db6f344603c6cba0d0d388da555ea88`
- Authoritative evaluate: **PASS** (`authoritative=true`)

## Artifact digests

| Artifact | SHA-256 |
|---|---|
| `ode-solvers.json` | `19ae52bebdae0a1b81c2af87b2dd2fd8e5188c2b602fd559662fe9145ee00bb5` |
| `ivp.json` | `c8ac650708a8f2c821f2373d5e8022e78fedfc28973d314c3af82d0cadf14e21` |

## Remediation addressed

1. Strict evaluator + `validate_candidate_report` + 4 negative tests
2. Split event evidence; no synthetic stop/restart in root finder
3. ivp D: StepInterpolant probes with analytic errors
4. ivp E: real SolOut event loop, interrupt, restart, x5 determinism
5. ode_solvers: Unsupported for preferred dense/event/stop architecture
6. F: callback stop + typed domain latch (both candidates)
7. A–G: per-experiment determinism x5 + subprocess x5
8. Executable dependency audit via `cargo metadata` + source scan

## ADR 0005

Status remains **Proposed**. Favor `ivp` on measured contract fit.

## Commands

```bash
cargo xtask evaluate --scope gate-1b0
```

## Gate boundary

No Gate 1B1. No ADR acceptance. PR #2 remains draft.
