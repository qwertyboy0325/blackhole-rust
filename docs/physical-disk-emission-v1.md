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
F_one_face = (3 c⁶ Ṁ) / (8 π G² M²) · Q / x⁶
         = (3 G M Ṁ) / (8 π r_phys³) · Q
```

with `x = √(r/M)`, `B = 1 + a*/x³`, `C = 1 − 3/x² + 2 a*/x³` (PT74; `a*` linear
in `C`).

Conventions:

- **One face** (upper = lower by PT74 symmetry); digest tags `one-face`.
- Zero torque at **prograde ISCO**; `F → 0` as `r → r_isco⁺`.
- **Prograde only** (`a*/M ≥ 0`); retrograde is a typed reject.
- Outside `[r_in, r_out]` (resolved disk annulus) = absence, not clamp.
- Hits with `r ≤ r_isco` contribute no physical emission.

Independent oracles:

1. Newtonian zero-torque `F_N = (3GMṀ)/(8πr³)(1 − √(r_isco/r))` at large `r`.
2. Numerical integral of the PT conservation-law integrand (different code path),
   converted to `Q` and the same SI prefactor.

## Temperature and Planck

```text
T_eff = (F_one_face / σ_SB)^{1/4}
I_ν,em(ν,r) = B_ν(ν, T_eff)
π ∫_0^∞ B_ν dν = σ_SB T⁴ = F_one_face
```

Factor **π is mandatory** (isotropic Lambert emitter). Digests/tests fail if π
is dropped. `σ_SB` is derived from exact `h`, `c`, `k_B`.

Deferred: color correction `f_col`, limb darkening, atmosphere, returning
radiation, Comptonization.

## Physical spectral grid + transport

- New grid family `physical-spectral-grid-explore-{n}` (Hz, log-spaced).
- Provisional band `[1e11, 1e17]` Hz for Gate 2C0 calibration — **bin count not
  frozen** without convergence evidence.
- Vacuum transport reuses `transport_i_nu`: `I_ν,obs(ν_obs) = g³ I_ν,em(ν_obs/g)`.
- Closures (separate families): emitter `π∫B ≈ σT⁴` (truncation-aware);
  transport `∫I_obs ≈ g⁴ ∫I_em` on the mapped band.

## Artifacts (raw authority)

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
  --physical-spectral-grid physical-spectral-grid-explore-256 \
  --output-dir artifacts/gate-2c0-physical-emission \
  --execution parallel --threads N --require-release

cargo run --release -p xtask -- evaluate --scope gate-2c0-physical-emission
```

## Known limitations

- Diagnostic scene `r_inner` may differ from true ISCO; emission requires
  `r > r_isco` inside the resolved annulus.
- Physical grid bin count is exploratory pending ladder freeze.
- No celestial-background physical radiometry; non-disk = absence.
- No CIE/OpenEXR in this gate.
