# Gate 1B2 Final Report (in progress)

## 1. Branch / merge prerequisite

- PR #3 landed on `main` via merge commit `eaf273e` (Gate 1B1 tip `cce30cc`).
- Working branch: `gate-1b2-ray-termination-preview`

## 2. Disk geometry

- Surface: `f = z` (Cartesian Kerr–Schild)
- Annulus: explicit `ThinDiskGeometry { r_inner, r_outer }` with `r_inner > r_+`, `r_outer > r_inner`
- Geometric scene radii only — not ISCO (preset `inner_edge = "prograde_isco"` is ignored)

## 3. Filtered-event API

- `EventSurface::classify_localized_hit` → `Ok(None)` rejects; `Ok(Some(metadata))` accepts
- `EventArmingPolicy { minimum_affine_parameter }` on `Dop853Config` (no geometry mutation)

## 4–6. Metadata / outcomes / opaque first-hit

- `EventId::ThinDisk`, `EventMetadata::ThinDisk { oblate_radius, crossing_side }`
- `RayOutcome::{DiskHit, Escaped, HorizonEvent, HorizonApproach, AffineLimit, Failed}`
- First accepted disk hit interrupts; outside-annulus plane crossings continue

## 7–9. Tests

- Analytic disk + event ordering + Kerr camera corpus (≥10 cases, 0 skips)
- Longer Kerr convergence probe: declared candidates; status Verified or Unverified (non-blocking)

## 10–18. Artifacts / evaluator

- `cargo xtask trace-outcome-map …` → PPM / PGM / JSON under `artifacts/gate-1b2/`
- `cargo xtask evaluate --scope gate-1b2`
- Fixed categorical legend (black/orange/blue/purple/red)

## 19. Limitations

- Accepted-step sign-change / exact endpoint roots only
- No even-in-step multiple plane crossings, tangent contact, radiometry, textures, GPU, GUI

## 20. Recommended Gate 2A

- Celestial-sphere lookup from `Escaped` states
- Physical disk radiometry deferred
