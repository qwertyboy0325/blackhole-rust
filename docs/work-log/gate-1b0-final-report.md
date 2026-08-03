# Gate 1B0 final report (lifecycle closure)

## Identity

- Branch: `gate-1b0-dop853-spike`
- Base: Gate 1A merge `dc38619` on `main` (PR #1 merged)
- PR #2: Gate 1B0-only delta
- Schema: `gate-1b0-v3`
- Authoritative evaluate commit: `bd561cbcc4f1d0307df8c5a7e88b24bc9cdad840`
- Authoritative evaluate: **PASS**

| Artifact | SHA-256 |
|---|---|
| ode-solvers.json | `cd92ea430f751f82a7a65dcbc2a422075acbba41f88116fe376ac1583b45e41a` |
| ivp.json | `2961eba63cb9aa1900d5d90dbaef7db93361b9cc0d929146ea21ba24d3d70d9c` |

## Closure items

1. Raw solver stop vs adapter-localized `AdapterOutcome::Event`
2. `SpikeAdapterError::Domain` caller-visible typed errors (both candidates)
3. Non-finite nominal success → `NonFiniteResult`
4. Strict F validation + negative tests
5. Shallow probe wording: `shallow_sign_changing_crossing` only

## ADR 0005

**Proposed** — favor `ivp` on measured contract fit; no production dep.

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo xtask evaluate --scope gate-1b0
```

## Gate boundary

No Gate 1B1. No ADR acceptance. PR #2 remains draft for owner review.
