# Gate 1B0 final report

## Identity

- Branch: `gate-1b0-dop853-spike`
- Base: Gate 1A tip (`f2782c3`) — PR #1 not merged to `main` at spike start
- Scope: DOP853 dependency spike only (no production integrator)

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
