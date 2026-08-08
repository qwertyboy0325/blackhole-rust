# Physical thin-disk emission V1 (Gate 2C0)

Authoritative physical radiometry for an equatorial Kerr thin disk. **Not**
diagnostic `SpectralFrame V1`, not CIE/RGB/OpenEXR (Gate 2C1).

## Authority boundary

| Channel | Status |
| --- | --- |
| Page–Thorne one-face flux `F(r)` | AUTHORITATIVE |
| `T_eff = (F/σ_SB)^{1/4}` | AUTHORITATIVE |
| Planck `I_ν,em = B_ν(T_eff)` | AUTHORITATIVE |
| Physical Hz grid + `g³` transport | AUTHORITATIVE |
| Raw `f64le` + meta JSON | AUTHORITATIVE |
| Diagnostic `spectral-grid-v1` ν | **not Hz** — typed reject |
| CIE XYZ / scene-linear RGB / OpenEXR | DEFERRED (2C1) |

Inherited geometry/kinematics (frozen): geodesic integration, disk bounds from
the resolved thin-disk scene, prograde circular geodesic emitter, Gate 2B0 `g`.

## Physical scale

- `mass_kg = mass_solar × (GM_☉ⁿ / G)` with pinned IAU 2015 B3 `GM_☉ⁿ` and
  CODATA 2018 `G` (`CONSTANTS_REVISION = codata-2018+iau-b3-2015-v1`).
- Baseline geometrized `M = 1` remains the metric scale; physical meters use
  `r_phys = (GM/c²) · (r/M)`.
- `Ṁ` is typed **kg/s** (authoritative). Eddington ratio is not a digest input.

Preset: `presets/gargantua-physical-v1.toml` — project demonstration knobs,
explicitly **not** film/DNGR reconstruction.

## Page–Thorne flux (one face)

Primary: Page & Thorne 1974, ApJ 191, 499 (zero torque at prograde ISCO).

```text
Q(x) = B C^{-1/2} x^{-1} [ x − x₀ − (3/2) a* ln(x/x₀) − Σ log-root terms ]
F_one_face = (3 c⁶ Ṁ) / (8 π G² M²) · Q / (B √C x⁶)
         = (3 G M Ṁ) / (8 π r_phys³) · Q / (B √C)
```

with `x = √(r/M)`, `B = 1 + a*/x³`, `C = 1 − 3/x² + 2 a*/x³` (PT74; `a*` linear
in `C`). The factor `1/(B √C)` is mandatory for this `Q` convention.

Conventions:

- **One face** (upper = lower by PT74 symmetry); digest tags `one-face`.
- Zero torque at **prograde ISCO**; `F → 0` as `r → r_isco⁺`.
- **Prograde only** (`a*/M ≥ 0`); retrograde is a typed reject.
- Outside `[r_in, r_out]` (resolved disk annulus) = absence, not clamp.
- Hits with `r ≤ r_isco` contribute no physical emission.

Independent oracles:

1. Newtonian zero-torque `F_N = (3GMṀ)/(8πr³)(1 − √(r_isco/r))` at large `r`.
2. Independent conservation-law **flux** quadrature (different code path):
   `F ∝ (−Ω_,r)/(E−ΩL)² ∫(E−ΩL)L_,r dr` with SI conversion — compare flux, not `Q`.

## Temperature and Planck

```text
T_eff = (F_one_face / σ_SB)^{1/4}
I_ν,em(ν,r) = B_ν(ν, T_eff)
π ∫_0^∞ B_ν dν = σ_SB T⁴ = F_one_face
```

Factor **π is mandatory** (isotropic Lambert emitter). Digests/tests fail if π
is dropped. `σ_SB` is derived from exact `h`, `c`, `k_B`. Finite-grid truncation
uses the analytic total `∫_0^∞ B_ν = σT⁴/π`, never another finite numerical band.

Deferred: color correction `f_col`, limb darkening, atmosphere, returning
radiation, Comptonization.

## Physical spectral grid + transport

- Frozen gate grid: `physical-spectral-grid-v1` (256 log bins, `[1e11, 1e17]` Hz).
- Explore family `physical-spectral-grid-explore-{n}` remains for ladder evidence only.
- Vacuum transport reuses `transport_i_nu`: `I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs/g)`.
- Closures (separate families): emitter `π∫B ≈ σT⁴` (truncation-aware vs analytic
  total); transport `∫I_obs ≈ g⁴ ∫I_em` on the mapped band. Absolute and relative
  maxima are tracked independently (lowest raster index on ties).

### Why freeze 256 (not 128 / 512)

Smoke 32² emitter-SB ladder after the Page–Thorne root fix:

| bins | max rel emitter SB | vs prior |
| ---: | --- | --- |
| 64 | `1.94e-3` | — |
| 128 | `4.85e-4` | ~4× |
| 256 | `1.21e-4` | ~4× |
| 512 | `3.03e-5` | ~4× |

Gate acceptance uses frozen emitter-SB rel tol `5e-4`. **128** clears that
ceiling on smoke but leaves little headroom once gate 128² geometry and
g-mapping broaden the worst pixel; **256** sits ~4× under the tol with the same
~4×/doubling convergence and keeps the gate `I_ν` cube at ~32 MiB (128²×256).
**512** halves error again but doubles spectral memory/cost without changing the
physical claim (`PT + Planck + g³` on a frozen Hz band). Freeze at 256 as the
coarsest grid that clears the calibrated gate envelope with documented margin;
explore ladders remain non-authoritative.

## Emission-frame authority

`PhysicalDiskEmissionFrame` fails closed on `try_new` / deserialize / digest:

- `inside_isco` ↔ `radius_over_m <= r_isco/M`;
- positive `F`/`T_eff` only inside resolved bounds and strictly outside ISCO;
- absence = zero `F`/`T` (no clamp);
- `radius_m = gravitational_radius_m · radius_over_m` (scale hashed);
- emitting samples obey constructor `F = σ T_eff⁴` within `1e-12` rel.

| File | Role |
| --- | --- |
| `physical-emission-meta.json` | `PhysicalDiskEmissionFrame` digests/units |
| `physical-f-teff.f64le` | magic `BHRFTEF1`, per-pixel F, T_eff, g, r |
| `physical-spectral-meta.json` | `PhysicalSpectralFrame` digests/closure |
| `physical-i-nu-obs.f64le` | magic `BHRPHYI1`, observer-frame `I_ν` cube |
| `physical-render-report.json` | run report (timing excluded from content digest) |
| `*-diagnostic.pgm` | PRESENTATION_ONLY |

## CLI

```bash
cargo run --release -p xtask -- render-physical-disk-spectrum \
  --preset presets/gargantua-physical-v1.toml --tier gate \
  --physical-emission page-thorne-blackbody-v1 \
  --physical-spectral-grid physical-spectral-grid-v1 \
  --output-dir artifacts/gate-2c0-physical-emission \
  --execution parallel --threads N --require-release

cargo run --release -p xtask -- evaluate --scope gate-2c0-physical-emission
```

## Known limitations

- Diagnostic scene `r_inner` may differ from true ISCO; emission requires
  `r > r_isco` inside the resolved annulus.
- No celestial-background physical radiometry; non-disk = absence.
- No CIE/OpenEXR in this gate.
