# Gate 2A1 Final Report — Finite Celestial Boundary Coordinate Mapping

## Status

Authoritative `evaluate --scope gate-2a1-celestial-directions` **PASS** at tip `adde05c`
(evidence-closure digest tags).

## 1. Base / branch

- Base: `daaf3115d41ae0ce0f1522821c8d3699528b51c7`
- Branch: `gate-2a1-celestial-sphere-direction-mapping`
- Implementation: `e76e806`
- Evidence closure tip: `adde05c673f1d8c96cdfbf1f797c1411e7c94ae2`
- Draft PR: https://github.com/qwertyboy0325/blackhole-rust/pull/9

## 2. Module layout

- `relativity-core`: `spherical_ks_direction_from_cartesian`, `SphericalKsAzimuthStatus::digest_tag`
- `relativity-trace`: `celestial.rs` mapping + `celestial_coordinate_digest` (v1 tagged schema)
- `OutcomeClass::digest_tag`, `CelestialDirectionSource::digest_tag`
- `xtask`: `--emit-celestial-coordinates`; `evaluate --scope gate-2a1-celestial-directions`

## 3. Scientific claim

Finite diagnostic escape-boundary coordinates from `EscapeHit.state.position`
(`finite-oblate-escape-boundary-position`). Not asymptotic infinity; not momentum UV.

## 4. Conventions

- Chart: ingoing spherical KS; direction `[sinθ cosψ, sinθ sinψ, cosθ]`
- Seam: spherical KS `ψ=0` (preset `positive_x_half_plane`)
- UV: `u = wrap(ψ)/2π`, `v = θ/π`
- Poles: canonical `ψ=0` with explicit status

## 5. Coordinate digest (evidence closure)

- No Debug/Display/serde-derived enum hashing
- Length-prefixed domain separators for all strings
- Full `CelestialCoordinateConvention` hashed (schema, id, source, boundary, chart,
  north axis, handedness, seam, u/v mapping, pole policy, asymptotic correction)
- Regression tests: seam/pole/u/v/chart changes alter digest; enum tags distinct;
  shade style does not alter digest

## 6–8. Accounting / radius

| field | value |
|---|---:|
| escaped / mapped / fail / pole | 2442 / 2442 / 0 / 0 |
| requested → resolved radius | 1000 → 80 (`gate-1b2-diagnostic-radius-cap`) |

## 9–11. Digests at `adde05c`

| Artifact | Digest |
|---|---|
| coordinate | `5d8df5ba007beeb3742ef9c3a684dbd86704f6b9a29271356e87d07fc2c71328` |
| coordinate JSON | `e37b8f32990aa8dd95557899ccdc80fd5d38bec5ace7fccef18541b666cb61ca` |
| UV-debug PPM | `4262eb4fe84937557cf3679fa390d2883151a2aaf25e9b973d6297acfe8f2107` |
| evaluator content | `8550653c655711f02a6832f20f5b13e55235d1d3d28e4ed5e890c6e337585e81` |

## 12–16. Compatibility

- Gate 1B2 class/PPM/PGM/counts MATCH
- Gate 2A0-4 numerical profile MATCH
- Subprocess determinism PASS
- Trace-data unchanged by coordinate mapping

## 17–20. Authority / exclusions

- `result: PASS` / `authoritative: true` / `dirty: false`
- Textures, radiometry, asymptotic correction — **not started**
- Gate 2A2 not started

Stop at Gate 2A1 boundary for owner review.
