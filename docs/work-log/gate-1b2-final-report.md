# Gate 1B2 Final Report

## 1. Branch, commits, PR

- Prerequisite: Gate 1B1 merged to `main` at `eaf273e` (tip `cce30cc`)
- Branch: `gate-1b2-ray-termination-preview` @ `e419bf7`
- Draft PR: open from https://github.com/qwertyboy0325/blackhole-rust/pull/new/gate-1b2-ray-termination-preview  
  (`gh` API auth currently invalid — create draft manually if needed)

## 2. Disk geometry

- `f = z` (Cartesian KS) + explicit `ThinDiskGeometry { r_inner, r_outer }`
- Validation: finite, `r_inner > r_+`, `r_outer > r_inner`
- Geometric radii only — not ISCO

## 3–6. Filtered events / metadata / outcomes / opaque first-hit

- `EventSurface::classify_localized_hit` + `EventArmingPolicy`
- `EventMetadata::ThinDisk { oblate_radius, crossing_side }`
- `RayOutcome` taxonomy in `relativity-trace`
- Outside-annulus plane crossings continue; accepted disk hit interrupts

## 7–9. Tests

- Analytic disk + event ordering + 12-case Kerr camera corpus (0 skips)
- Convergence probe: declared 4 candidates (Verified or Unverified; non-blocking)

## 10–14. 128×128 outcome map (serial)

Artifacts (local; gitignored):

- `artifacts/gate-1b2/outcome-map.ppm`
- `artifacts/gate-1b2/outcome-map.json`
- `artifacts/gate-1b2/rhs-evaluations.pgm`

Counts: disk=12307, escaped=2442, horizon_event=1462, horizon_approach=173, affine=0, failed=0

Digests:

- class: `64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4`
- ppm: `ac058d5af01b425e411b5c33017210bf888aa52918cfd085bb863d7ddc99184c`
- pgm: `2df226390057bb87b64d172cd258087b0ef4c1ad0ce0d4378e003b5861a75db5`
- content: `9644066230d674eafefb9edaa6a76cbe6d529075a2ce7e758425c136cbd76ec8`

## 15. Wall-clock

- ~210.6 s for 128×128 (~78 rays/s), serial CPU

## 16–18. Evaluator / CI

- `cargo xtask evaluate --scope gate-1b2` → **PASS** (`authoritative: true`) at `48a539b`
- Content digest: `80486e02ade94c3c21b5bace27cce775669084f237cc9b496945c0c4fb842f63`
- Map class digest (×3 subprocess identical): `64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4`
- fmt / clippy / workspace tests green

Root cause of earlier `PASS_NON_AUTHORITATIVE`: Gate 1B2 `.gitignore` edit dropped `/artifacts/gate-1b1/*` rules; restored in `48a539b`.

## 19. Limitations

- Accepted-step sign-change / exact endpoint only
- No radiometry / textures / OpenEXR / GPU / GUI

## 20. Gate 2A recommendation

- Celestial-sphere lookup from `Escaped`
- Keep disk opaque classification; add emission later as separate gate
