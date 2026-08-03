# Gate 1A execution memo

**Gate:** 1A — Geometry kernel and null-ray initialization  
**Base:** `25a9e72c73e33a13eda2f03819820b5492d09e43`  
**Branch:** `gate-1a-geometry-kernel`  
**Date:** 2026-08-03

## Preflight

| Check | Result |
|---|---|
| Branch before start | `main` @ `25a9e72` = approved base |
| Worktree | clean |
| Remote | `origin` → `https://github.com/qwertyboy0325/blackhole-rust.git` |
| Toolchain | `rustc 1.96.0`, `cargo 1.96.0`, `stable-aarch64-apple-darwin` |
| GitHub Actions | query forbidden from this environment; CI added in-repo |
| Feature branch | created from approved base |

## Proposed file and module map

```text
Cargo.toml                          workspace root
rust-toolchain.toml                 pin 1.96.0
LICENSE-MIT / LICENSE-APACHE
.gitignore
.github/workflows/ci.yml

crates/relativity-core/
  Cargo.toml
  src/
    lib.rs
    error.rs                        typed domain/conditioning failures
    types.rs                        PositionKS, Covector, Vector, LocalFrame
    kerr.rs                         checked KerrParams
    radius.rs                       stable oblate-spheroidal r
    metric/
      mod.rs
      kerr_schild.rs                g, g^{-1}, H, ℓ (analytic)
      minkowski.rs                  η test metric
      derivatives.rs                analytic ∂_i g^{αβ}
    coords/
      mod.rs
      boyer_lindquist.rs            BL ↔ KS position / vector / covector
    hamiltonian.rs                  H, dx/dλ, dp/dλ at a state
    observer.rs                     ZAMO + Minkowski observer, tetrad
    ray_init.rs                     rectilinear null-ray initialization
    corpus.rs                       stratified point corpus (pub for xtask)
  tests/
    metric_identity.rs
    derivatives_oracle.rs           central-FD oracle (test-only)
    transforms.rs
    tetrad_and_rays.rs
    domain_errors.rs

xtask/
  Cargo.toml
  src/main.rs                       inspect-point, inspect-initial-ray, evaluate

presets/gargantua-baseline.toml     Gate-0 preset (schema-validated; unused fields preserved)
docs/adr/0005-dop853-dependency.md  Proposed
docs/research/dop853-rust-dependency-audit.md
docs/research-sources.md            Gate-1A sources appended
docs/work-log/                      this memo + layer notes + failures
artifacts/gate-1a/                  evaluate reports (gitignored contents)
```

No speculative crates. No renderer/integrator production code.

## Formulas and primary sources (first slice → full 1A)

Signature `(-,+,+,+)`, geometrized units.

1. **Kerr params:** `|a| ≤ M`, `M > 0`, finite; extremal `|a|=M` allowed explicitly [Carter1968, BPT1972].
2. **Oblate radius:** `A = ρ² − a²`, `D = √(A² + 4 a² z²)`.
   - `A ≥ 0`: `r² = ½(A + D)`  
   - `A < 0`: `r² = 2 a² z² / (D − A)` (rationalized; cancels `A+|A|`)  
   Ring/domain exclusion when `r = 0` or non-finite [physics-assumptions, GRay2].
3. **Cartesian KS metric** [Kerr1963 / Kerr–Schild; GRay2; Wikipedia KS form]:
   ```text
   g_μν = η_μν + 2 H ℓ_μ ℓ_ν
   g^μν = η^μν − 2 H ℓ^μ ℓ^ν
   H = M r³ / (r⁴ + a² z²)
   ℓ_μ = (1, (r x + a y)/(r²+a²), (r y − a x)/(r²+a²), z/r)
   ℓ^μ = η^{μν} ℓ_ν  ⇒  ℓ^t = −1, ℓ^i = ℓ_i
   ```
4. **Hamiltonian RHS** [Carter1968, MTW1973]:
   ```text
   H = ½ g^{μν} p_μ p_ν
   dx^μ/dλ = g^{μν} p_ν
   dp_μ/dλ = −½ (∂_μ g^{αβ}) p_α p_β
   ```
5. **ZAMO / tetrad** [BPT1972, James2015]; past-directed local null `k̂ = (−1, n̂)`, radiometry uses `−k`.

## Independent validation oracle

| Quantity | Production | Oracle |
|---|---|---|
| `g^{μν}` | KS closed form | `g_μν` matrix inverse (nalgebra LU) + identity residual |
| `∂_i g^{αβ}` | analytic KS differentiation | test-only central finite differences of production `g^{αβ}` (independent code path; not the analytic ∂) |
| Minkowski | `a=0,M→0` / dedicated η | exact η |
| BL↔KS | closed transforms | round-trip in valid region |
| Null rays | init algebra | `g(k,k)=0`, orientation signs |

## Conditioning hazards

- Oblate `r` cancellation for `A < 0`, `z → 0`
- `z/r` and `H` near ring singularity
- Near-extremal `r₊ − r₋` (BL reporting only in 1A)
- BL axis `φ` / `sin θ → 0`
- Near-horizon BL `Δ → 0` (KS chart remains primary)
- Tetrad Gram–Schmidt near ergoregion / horizon
- Covector vs vector index confusion
- Past-ray vs future-radiometry sign flip

## Exact tests for this slice (Gate 1A minimum)

Layer L0 governance compiles. Then:

- **L1 params:** reject `M≤0`, `|a|>M`, non-finite; accept extremal
- **L2 radius:** stable branch corpus; ring rejection; non-finite rejection
- **L3 metric:** symmetry; `g g^{-1} ≈ I`; Lorentzian signature; large-`r` → η; `a=0` Schwarzschild KS; axis finite; across `r₊` finite
- **L4 derivatives:** analytic vs FD oracle on stratified corpus; record worst abs/rel
- **L5 transforms:** BL↔KS position RT; vector ≠ covector paths; singular typed errors
- **L6 Hamiltonian:** finite RHS; `dp_t/dλ ≈ 0` envelope; no H-projection
- **L7 tetrad:** `g(u,u)=−1`; orthonormality; right-handed; future-directed
- **L8 rays:** local+chart null; past init / future radiometry opposite
- **L9 xtask/evaluate:** deterministic JSON; `evaluate --scope gate-1a` PASS/FAIL

Tolerance provenance labeled per test (fp / FD-oracle / provisional smoke).

## Planned commit boundaries

1. `chore: workspace, licenses, toolchain, CI scaffolding`
2. `feat(core): Kerr params and stable oblate radius`
3. `feat(core): Cartesian Kerr–Schild metric and Minkowski oracle`
4. `feat(core): analytic inverse-metric derivatives + FD oracle tests`
5. `feat(core): BL↔KS transforms and Hamiltonian RHS`
6. `feat(core): ZAMO tetrad and null-ray initialization`
7. `feat(xtask): inspect-point, inspect-initial-ray, evaluate gate-1a`
8. `docs: DOP853 audit, ADR-0005 Proposed, sources, Gate 1A report`

Commits land as layers pass; final push + draft PR at gate boundary.

## Mathematical layers (execution order)

| Layer | Scope | Gate to next |
|---|---|---|
| L0 | Workspace + governance | `cargo check` workspace |
| L1 | `KerrParams` | param unit tests |
| L2 | Stable `r` + domain | radius tests |
| L3 | Metric / inverse | metric corpus tests |
| L4 | Analytic ∂ + FD oracle | derivative corpus |
| L5 | Coordinate maps | transform tests |
| L6 | Hamiltonian RHS | hamiltonian tests |
| L7 | Observer / tetrad | tetrad tests |
| L8 | Ray init | ray tests |
| L9 | xtask + evaluate + audit | full evaluate PASS |

Do not start layer *N+1* until layer *N* focused tests pass.
