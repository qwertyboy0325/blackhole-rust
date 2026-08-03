# Gate 1B0 final report (lifecycle closure)

## Identity

- Branch: `gate-1b0-dop853-spike`
- Base: Gate 1A merge `dc38619` on `main` (PR #1 merged)
- PR #2: Gate 1B0-only delta
- Schema: `gate-1b0-v3`
- Authoritative commit: `d072e32d5fc3a62d80ca45961ca37ce899661b93`
- Authoritative evaluate: **PASS**

| Artifact | SHA-256 |
|---|---|
| ode-solvers.json | `f2c5bb1ecefca8bd3c97cbf8717d81ee98bded0f619d31725bdacd63987d7eb9` |
| ivp.json | `2404ccc73a4fe90943f9db01671828023fda4f2f712dab23d96d8897fc851123` |

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
