# ADR 0005: DOP853 Rust dependency selection

- Status: **Accepted**
- Decision: `ivp = "=0.6.0"` (exact pin)
- Accepted: 2026-08-03 (owner review after Gate 1B0 PASS)
- Evidence commit: `bd561cbcc4f1d0307df8c5a7e88b24bc9cdad840`

## Context

ADR 0002 selects adaptive DOP853 with dense-output event localization for the
CPU `f64` geodesic oracle. Gate 1B0 executed executable spikes for
`ode_solvers::Dop853` and `ivp` DOP853 against a frozen checklist.

## Decision

**Accept `ivp = "=0.6.0"`** as the initial production DOP853 dependency, isolated
behind `relativity-integrate`. No `ivp` type may appear in that crate’s public API.

Gate 1B0 measured capabilities:

| Requirement | ode_solvers 0.6.1 | ivp 0.6.0 |
|---|---|---|
| DOP853 f64 | Supported | Supported |
| 8D state | Supported | Supported |
| Vector tolerance direct | Unsupported | Supported |
| Accepted-step dense interpolant | Unsupported | Supported |
| Event localization (preferred arch) | Unsupported | Supported |
| Stop/restart semantics | Unsupported | Supported |
| Typed domain error | Supported | Supported |

### Why `ivp`

- Direct `Tolerance::Vector` for the 8D state
- `SolOut` exposes current accepted-step `StepInterpolant`
- Demonstrated localize → interrupt → adapter-owned Event outcome → restart
- Apache-2.0; no native deps in the locked spike graph

### Why not `ode_solvers` for the preferred architecture

- Public API exposes predetermined dx-grid dense samples only
- Dop853 continuous-output coefficients (`rcont`) are private
- Fixed-grid / external linear interpolation does not satisfy accepted-step
  event localization owned by this project

### Evidence digests (authoritative Gate 1B0 evaluate)

| Artifact | SHA-256 |
|---|---|
| ode-solvers.json | `cd92ea430f751f82a7a65dcbc2a422075acbba41f88116fe376ac1583b45e41a` |
| ivp.json | `2961eba63cb9aa1900d5d90dbaef7db93361b9cc0d929146ea21ba24d3d70d9c` |

### Stability measure

Exact-version pin `ivp = "=0.6.0"` until a later ADR revises the dependency.
Bump only with measured adapter re-validation.

## Alternatives

- From-scratch DOP853: rejected while `ivp` meets the contract
- Fork/upstream contribution to `ode_solvers`: open if coefficient API is needed later

## Consequences

- Gate 1B1 implements `relativity-integrate` with project-owned types and events
- Physics RHS remains in `relativity-core`
- Hamiltonian projection remains prohibited
- Tangent / no-sign-change event detection is not claimed

## References

- `docs/research/gate-1b0-dop853-spike-report.md`
- `docs/research/dop853-rust-dependency-audit.md`
- `docs/work-log/gate-1b0-final-report.md`
- ADR 0002
