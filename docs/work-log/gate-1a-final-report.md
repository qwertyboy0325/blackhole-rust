# Gate 1A final report

## 1. Branch, commit, PR

- Branch: `gate-1a-geometry-kernel`
- Base: `25a9e72c73e33a13eda2f03819820b5492d09e43`
- Draft PR: opened against `main` after push (see PR URL in chat)

## 2. Files changed (summary)

- Governance: `LICENSE-*`, `rust-toolchain.toml`, `.github/workflows/ci.yml`, `.gitignore`, `.cargo/config.toml`, workspace `Cargo.toml`
- `crates/relativity-core/**`: Kerr params, stable oblate radius, KS metric/inverse/derivatives, BL↔KS maps, Hamiltonian RHS, ZAMO tetrad, ray init, corpus, tests
- `xtask/**`: preset schema, inspect-point, inspect-initial-ray, evaluate
- Docs: ADR 0005 (Proposed), DOP853 audit, research-sources, work-log, README/AGENTS

## 3. Equations and sources

- Signature `(-,+,+,+)`; `g = η + 2H ℓ⊗ℓ`, `g^{-1} = η − 2H ℓ⊗ℓ`
- `H = M r³/(r⁴+a²z²)`, `ℓ_μ = (1,(rx+ay)/(r²+a²),(ry−ax)/(r²+a²),z/r)`
- Hamiltonian: `H=½ g^{μν}p_μp_ν`, RHS as ADR 0001 / Carter / MTW
- ZAMO: BPT1972 LNRF formulas in BL, pushed to KS
- Sources recorded in `docs/research-sources.md`

## 4. Stable oblate radius

- Direct: `r²=½(A+D)` when `A≥0`
- Stable: `r²=2a²z²/(D−A)` when `A<0`
- Evidence: naive collapses at `(0.05,0,1e-9)`, `a=0.999`; stable matches implicit spheroid
- Domain: `r=0` → typed ring/excluded-disk error (never success)

## 5. Metric / derivative strategy

- Production: closed-form KS metric + analytic `∂_i g^{αβ}`
- Oracles: Gauss–Jordan inverse of `g_μν`; test-only adaptive central FD of `g^{αβ}`
- Paths do not share derivative code

## 6. Worst residuals (evaluate PASS)

| Quantity | Worst | Location |
|---|---|---|
| `g g^{-1} − I` | `1.9e-15` | inside-horizon corpus point |
| analytic vs FD `∂g^{-1}` | `3.8e-3` abs | cancellation-prone `(0.1,0,1e-8)`, `a=0.999` |
| tetrad orthonormality | `2.2e-16` | baseline ZAMO |
| nullness / `|H|` | `1.8e-16` | baseline center ray |

FD bound provenance: oracle comparison (provisional), not geodesic acceptance.

## 7. Transforms / tetrad

- BL↔KS position round-trip off-axis; axis → typed singular
- Vector vs covector Jacobians independently tested
- ZAMO: `g(u,u)=−1`, orthonormal, right-handed, future-directed

## 8. Initial-ray orientation

- Local past null `k̂=(−1,n̂)`; chart `H≈0`; future momentum `−p`
- Sign-reversal test between backward init and radiometry momentum

## 9. DOP853 audit recommendation

Prefer `ode_solvers` (Apache-2.0) behind an adapter in Gate 1B; `ivp` backup;
`diffsol` lacks DOP853. ADR 0005 **Proposed**. No ODE dep in Gate 1A.

## 10. Commands run

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo xtask inspect-point --mass 1 --spin 0.999 --x 4 --y 1 --z 2 --format json
cargo xtask inspect-initial-ray --preset presets/gargantua-baseline.toml --sensor-x 0 --sensor-y 0 --format json
cargo xtask evaluate --preset presets/gargantua-baseline.toml --scope gate-1a
```

All PASS (evaluate `result: PASS`).

## 11. CI status

Workflow `.github/workflows/ci.yml` added; remote Actions status depends on push.

## 12. Unresolved risks

- Analytic `∂g^{-1}` vs FD disagreement grows in deep cancellation; deeper `|z|` validated by radius tests only
- ZAMO currently refused for `Δ≤0` (exterior baseline only)
- Component-scaled DOP853 adapter not yet spiked
- Camera/disk/spectrum preset fields provisional

## 13. Acceptance criteria evidence

| Criterion | Evidence |
|---|---|
| Workspace + governance | licenses, toolchain, CI, gitignore |
| Typed domain handling | `CoreError` / domain tests |
| Stable radius | unit tests + work-log |
| Metric/inverse corpus | `metric_corpus` + evaluate |
| Analytic vs FD derivatives | `derivatives_oracle` + evaluate worst |
| Hamiltonian RHS | unit tests; `dp_t=0` |
| Transforms | coords tests |
| ZAMO tetrad | observer + evaluate |
| Null rays + orientation | ray_init tests + evaluate |
| Diagnostics | inspect-* JSON |
| evaluate PASS | `artifacts/gate-1a/evaluation.*` |
| No renderer/integrator | dependency/audit only |

## 14. Recommended Gate 1B scope

- Adopt ODE adapter per ADR 0005 after owner approval
- Adaptive DOP853 stepping with domain `h` guards
- Dense-output event localization (horizon/disk/sky)
- Typed `RayOutcome` taxonomy
- Invariant drift diagnostics (`H,E,L_z,Q`) without projection
- Still no image renderer / GPU / egui

## 15. Diff summary

Adds minimal Cargo workspace (`relativity-core`, `xtask`) implementing Gate 1A
geometry kernel end-to-end with deterministic evaluation and Proposed DOP853 ADR.
