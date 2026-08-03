# Gate 1A final report (remediation)

## Identity

- Branch: `gate-1a-geometry-kernel`
- Draft PR: https://github.com/qwertyboy0325/blackhole-rust/pull/1
- Local evaluate: **PASS** (`authoritative=true`, dirty=false)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)` / `aarch64-apple-darwin`

## Commit provenance

| Field | SHA / value |
|---|---|
| `reviewed_head` | `37d5e59afb974e0d5d36a5ee1481570b6951cf17` |
| authoritative evaluator commit | `25eb8e654751d533010c4cab0d725d3c49290bdf` |
| commits between | one evidence-closure commit (`docs/` + `xtask/` only) |
| between commits documentation-only | **yes** (no geometry production code) |

Evaluator JSON records the same block under `provenance`.

## Coordinate convention selected

**Ingoing Kerr–Schild** with explicit `PositionSphericalKs { T, r, θ, ψ }`:

```text
x + i y = (r + i a) e^{iψ} sinθ
z = r cosθ
t = T
dT = dt_BL + (2 M r / Δ) dr
dψ = dφ_BL + (a / Δ) dr
```

Placement gauge: `T=t`, `ψ=φ` at the BL event. Jacobians keep `∂T/∂r`, `∂ψ/∂r`.
Matched to project `ℓ_μ` signs (verified `η(ℓ,ℓ)=0` on the embedding).

Sources: GRay2 `ℓ_μ` form; BL Kerr metric [BoyerLindquist1967, Carter1968];
owner remediation note; `docs/physics-assumptions.md`.

## Independent metric-pullback evidence

`tests/coordinate_pullback.rs`: independently coded `bl_metric` + full Jacobian;
`‖Jᵀ g_KS J − g_BL‖_∞ < 1e-8` on stratified exterior points; explicit
`∂T/∂r = 2Mr/Δ` check; vector/covector pairing and round trips.

## ZAMO zero-angular-momentum evidence

- BL `|u_φ| = 1.39e-17` (evaluate baseline)
- `g_BL(u,u) ≈ −1` and KS pullback norm checked in pullback tests
- Camera look (−e₃) has BL radial component `look_r ≈ −0.95` (toward BH)

## Corpus coverage (authoritative evaluate)

| Metric | Value |
|---|---|
| expected points | 24 |
| evaluated valid | 22 |
| expected failures | 2 |
| unexpected failures | 0 |
| unexplained skips | 0 |
| derivative components | 1056 (= 22 × 3 × 16) |
| by tag | WeakField 3, NearAxis 3, NearEquatorial 3, NearOuterHorizonExterior 3, InsideHorizon 3, NearExtremalSpin 3, CancellationProneOblate 4, ExpectedDomainFailure 2 |

## Worst residuals (evaluate JSON)

| Quantity | Worst | Location / note |
|---|---|---|
| metric identity | `1.915e-15` | `(1.005, 0.05, 0.05)` |
| raw inverse asymmetry | `4.441e-16` | corpus max |
| η(ℓ,ℓ) | `3.331e-16` | corpus max |
| g(ℓ,ℓ) | `9.307e-16` | corpus max |
| \|det(g)+1\| | `3.220e-15` | corpus max |
| derivative abs | `3.816e-3` | CancellationProneOblate `(0.1,0,1e-8)` axis=2 αβ=(2,2) analytic=`−2.039e-2` fd=`−1.657e-2` scale=`2.039e-2` |
| derivative rel at worst abs | `1.872e-1` | relative residual at the abs-worst site only |
| tetrad orthonormality | `2.220e-16` | baseline ZAMO |
| ZAMO \|u_φ\| | `1.388e-17` | baseline |
| nullness | `2.741e-16` | baseline center ray |

`derivative_rel_at_worst_abs` is **not** the global worst relative residual; it is
computed only at the component where `derivative_abs` is maximal.

## Dirty-tree behavior

Porcelain including untracked files. Non-empty ⇒ `worktree_clean` FAIL ⇒ no
authoritative PASS.

## Symmetric-matrix handling

- `from_lower_triangle`: mirrors lower only (no averaging)
- `RawMatrix4` + `try_from_raw`: reject asymmetry
- Inverse oracle reports `raw_asymmetry` before conversion

## DOP853 recommendation

ADR 0005 **Proposed**. No locked crate preference. Gate **1B0** spike required
for both `ode_solvers` and `ivp` (vector tol, SolOut/callback, dense
coefficients vs sampled interpolant, guards, stats). No ODE dep in tree.

## Commands and CI

```bash
cargo fmt --all -- --check   # PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings  # PASS
cargo test --workspace --all-features  # PASS
cargo xtask evaluate --preset presets/gargantua-baseline.toml --scope gate-1a  # PASS
```

CI on `25eb8e6`: **pass** (push `30783970750`, PR `30783972950`)

## Remaining risks

- Local placement gauge vs globally integrated `T(r)`, `ψ(r)` from infinity
- `derivative_rel_at_worst_abs` can be large when the abs-worst component is
  tiny; the operative oracle gate is abs ≤ 5e-3 with rel ≤ 2e-3 per component
- No Gate 1B scope introduced
