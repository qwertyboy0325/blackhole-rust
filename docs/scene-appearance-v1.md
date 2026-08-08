# Scene appearance V1 (Gate 2D1)

Production appearance layer over frozen scientific and presentation authorities.

## Authority taxonomy

| Layer | Role |
| --- | --- |
| `SCIENTIFIC_AUTHORITY` | Gate 2C0 / 2C1 unchanged |
| `PHYSICALLY_MOTIVATED_APPEARANCE` | derived disk flux modulation |
| `ARTISTIC_ENVIRONMENT` | procedural finite-boundary sky |
| `DISPLAY_PRESENTATION` | Gate 2D0 gamut / tone / OETF |

## Disk appearance (D1-B)

`PhysicalDiskEmissionFrame` is never mutated. Derived:

```text
m(r,φ) = 1 + A(r) Σ w_j cos(m_j φ + k_j ln(r/r_ref) + phase_j)
F_app = F_base · m
T_app = (F_app / σ)^(1/4)
```

then existing Planck → g³ → CIE path.

Radial envelope `raised-cosine-radial-envelope-v1` (A2):

```text
u = clamp((r − r_inner)/(r_outer − r_inner), 0, 1)
A(r) = A_max · sin²(π u)
```

so `A(r_inner) = A(r_outer) = 0`.

Claim label: **`ANNULAR_APPEARANCE_MEAN_PRESERVING`** (A3). Not energy /
luminosity / observer-frame flux conservation.

## Celestial environment (E1-B)

Finite-boundary `unit_coordinate_direction` sampling
(`finite-oblate-ks-boundary-uv-v1`). Not null infinity.

Stars use fixed **`angular_sigma_rad`** (A1) — never pixel-scaled.

Milky-Way-like band is labeled `MILKY_WAY_LIKE_PROCEDURAL_APPEARANCE`.

No external HDRI. No sky × lensing magnification. No disk `g` on sky.
`NO_ADDITIONAL_ENVIRONMENT_FREQUENCY_SHIFT`.

## Scene composition (S2)

```text
disk_ev0 = RGB_abs × 0.18 / L_middle_gray
env_ev0  = authored dimensionless linear RGB
scene    = ev0 × 2^EV
→ present_exposed_linear_rgb (A4)
→ gamut → PBR Neutral → sRGB OETF → RGB16
```

Never re-enter `present_physical_color_frame` exposure.

Identity scene (`identity_modulation` + `identity_black`, EV=0) must match Gate
2D0 RGB16 and `presentation_frame_digest` bit-exactly (A5).

`AffineLimit` / `Failed` → `SCENE_NUMERICAL_FAILURE`; no beauty PASS (A6).

## Presets

- `presets/appearance/gargantua-scene-v1.toml` — canonical beauty
- `presets/appearance/gargantua-scene-identity-v1.toml` — A5 differential

Do not modify `gargantua-physical-v1.toml` or `gargantua-cinematic-v1.toml`.

## CLI

```bash
cargo run --release -p xtask -- render-scene-appearance \
  --preset presets/gargantua-physical-v1.toml \
  --appearance presets/appearance/gargantua-scene-v1.toml \
  --presentation presets/presentation/gargantua-cinematic-v1.toml \
  --tier gate \
  --output-dir artifacts/gate-2d1-scene-appearance \
  --execution parallel --threads N --require-release

cargo run --release -p xtask -- evaluate --scope gate-2d1-scene-appearance
```
