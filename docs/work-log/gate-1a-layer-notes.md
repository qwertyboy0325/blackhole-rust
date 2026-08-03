# Gate 1A layer notes and failed experiments

## Preflight

- Base `25a9e72` confirmed; clean tree; branch `gate-1a-geometry-kernel`.
- Toolchain: rustc/cargo 1.96.0 (`stable-aarch64-apple-darwin`).

## L2 — oblate radius

- Naive `½(A+D)` collapses to `0` for `(x,y,z)=(0.05,0,1e-9)`, `a=0.999`.
- Stable branch returns `r² > 0` matching the implicit spheroid residual.
- Fixture `z=1e-300` is **invalid**: `z²` underflows to `0` in `f64`, so both
  formulas yield a domain `r=0` rejection. Documented; test uses `z=1e-9`.

## L4 — derivative FD oracle

- First failure: `CancellationProneOblate` at `z=1e-8` with fixed `h=1e-6`.
  Analytic `∂_z g^{tt} ≈ -2.03`, bad FD ≈ `-0.020` (factor ~`h/|z|`).
- Root cause: stencil exits the locally linear neighborhood; **not** an analytic
  `∂r` bug (`∂r²` itself matched FD even at `h=1e-6`).
- Fix: adaptive `h = clamp(1e-6 * max(|x_i|, r, 1e-16), 1e-14, 1e-4)`.
- Preserved test `fixed_h_invalid_in_cancellation_regime` as evidence.
- Corpus FD points use `|z| ≥ 1e-8`; deeper cancellation (`1e-9`–`1e-16`) is
  validated by stable-radius tests because `g^{μν}` FD underflows to 0 there.
- Oracle bounds used in tests: `abs_tol=5e-3`, `rel_tol=2e-3` (provisional
  oracle-comparison, not geodesic acceptance).

## L7 — tetrad

- First failure: orthonormality residual from wrong Gram–Schmidt projection on
  the timelike leg (`v ← v − g(v,u)u` instead of `v ← v + g(v,u)u` for
  `g(u,u)=−1`). Fixed; ZAMO + ray init tests pass.

## Remediation (PR #1 owner review)

- Replaced √(r²+a²) BL embedding with ingoing KS via `PositionSphericalKs` and
  `dT/dt`, `dψ/dφ` radial terms; pullback tests added.
- Corpus outcomes totalized; dirty porcelain blocks PASS.
- `MetricTensor` no longer silently averages; KS `η(ℓ,ℓ)/g(ℓ,ℓ)/det` invariants.
- DOP853 audit: no locked preference; ADR 0005 remains Proposed.
