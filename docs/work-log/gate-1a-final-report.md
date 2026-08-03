# Gate 1A final report (remediation)

## Identity

- Branch: `gate-1a-geometry-kernel`
- Tip / evaluate commit: `2b6c075fdac86e61b2342797284246c7f2ccd3d0`
- Draft PR: https://github.com/qwertyboy0325/blackhole-rust/pull/1
- Local evaluate: **PASS** (`authoritative=true`, dirty=false)
- CI: **pass** (push + pull_request checks on tip)
- Toolchain: `rustc 1.96.0 (ac68faa20 2026-05-25)` / `aarch64-apple-darwin`

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
| derivative abs | `3.816e-3` | CancellationProneOblate `(0.1,0,1e-8)` axis=2 αβ=(2,2) |
| derivative rel | `1.872e-1` | same site (abs still ≤ 5e-3 oracle bound) |
| tetrad orthonormality | `2.220e-16` | baseline ZAMO |
| ZAMO \|u_φ\| | `1.388e-17` | baseline |
| nullness | `2.741e-16` | baseline center ray |

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

CI on tip: Actions `check` **pass** for push `30783306153` and PR `30783308613`.

## Remaining risks

- Local placement gauge vs globally integrated `T(r)`, `ψ(r)` from infinity
- Derivative relative residual can be large when the component itself is tiny;
  abs bound is the operative oracle check there
- No Gate 1B scope introduced
