# Gate 1B0 final report

## Identity

- Branch: `gate-1b0-dop853-spike`
- Base: Gate 1A tip (`f2782c3`) — PR #1 not merged to `main` at spike start
- Scope: DOP853 dependency spike only (no production integrator)
- Authoritative commit: `08eeecd060a0927aba68e75ee7b3f96878e4b706`
- Authoritative evaluate: **PASS** (`authoritative=true`)

## Artifact digests (authoritative run)

| Artifact | SHA-256 |
|---|---|
| `ode-solvers.json` | `c606074982ca96d2a8706a00501eaa5c952b6cb20e2fe2b7e723e118698b47c4` |
| `ivp.json` | `58c40733afd1afbf8ed9950b7315e585fe13074323794d370345a2eaaa4e48f2` |

Regenerate: `cargo xtask evaluate --scope gate-1b0` (clean worktree required).

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo xtask spike-dop853 --candidate ode-solvers
cargo xtask spike-dop853 --candidate ivp
cargo xtask evaluate --scope gate-1b0
```

## ADR 0005

Status remains **Proposed**. Spike evidence documented in
`docs/research/gate-1b0-dop853-spike-report.md`. Favor `ivp` on measured
contract fit; no production dependency added.

## Risks

- `ivp` is younger; API stability requires Gate 1B1 adapter hardening
- `ode_solvers` lacks public Dop853 dense coefficients for external localizer
- Kerr probe G is diagnostic only — not a physical acceptance threshold
