# Gate 2A1 Final Report — Finite Celestial Boundary Coordinate Mapping

## Status

Authoritative `evaluate --scope gate-2a1-celestial-directions` **PASS** at tip `e76e806`.

## 1. Base / branch

- Base: `daaf3115d41ae0ce0f1522821c8d3699528b51c7`
- Branch: `gate-2a1-celestial-sphere-direction-mapping`
- Implementation tip: `e76e8062b4a08add5bb7742726175f7f7b741010`

## 2. Module layout

- `relativity-core`: `spherical_ks_direction_from_cartesian`, `SphericalKsDirection`, pole status; generic `spherical_ks_from_cartesian` unchanged (still rejects poles)
- `relativity-trace/src/celestial.rs`: mapping types, digest, JSON artifact, UV-debug shade, regression corpus
- `xtask`: `--emit-celestial-coordinates` on `trace-shade-many`; `evaluate --scope gate-2a1-celestial-directions`

## 3. Scientific claim

Coordinates are on the finite diagnostic escape boundary (`r_oblate = r_escape`) from `EscapeHit.state.position` (`finite-oblate-escape-boundary-position`). Not asymptotic infinity; not terminal-momentum UV.

## 4. Conventions

- Chart: ingoing spherical KS `(θ,ψ)`; direction `[sinθ cosψ, sinθ sinψ, cosθ]`
- Seam: spherical KS `ψ=0` (preset `positive_x_half_plane`)
- UV: `u = wrap(ψ)/2π`, `v = θ/π`
- Poles: canonical `ψ=0` with explicit north/south status (`AXIS_SIN_FLOOR = 1e-14`)

## 5. Position ≠ momentum

Unit test constructs +x boundary position with +y momentum; UV is `u=0, v=0.5`, not `u=0.25`.

## 6. Algebraic corpus

Schwarzschild cardinals, Euclidean reduction at `a=0`, seam ±δ, poles, Kerr round-trips (`a/M=0.5`, `0.999`) — covered by unit tests (evaluator check PASS).

## 7. Escape radius

- requested: `1000`
- resolved: `80`
- policy: `gate-1b2-diagnostic-radius-cap`

## 8. Accounting (gate 128×128)

| field | value |
|---|---:|
| escaped_count | 2442 |
| mapped_count | 2442 |
| mapping_failure_count | 0 |
| pole_count | 0 |

## 9–11. Digests

| Artifact | Digest |
|---|---|
| coordinate | `a129620fa694dcf28c8cd2074c2c87efa37fe6faa92978c209b5dfcc62d1d460` |
| coordinate JSON | `e20bc440625ac6c38f84ee795a68e8444fb053412573650a2ce893be869035c5` |
| UV-debug PPM | `4262eb4fe84937557cf3679fa390d2883151a2aaf25e9b973d6297acfe8f2107` |

## 12–13. Corpus / residuals

- 10-role regression corpus byte-identical across gate-run-0/1
- worst residual pixels (top): `(78,76)`, `(78,85)`, `(126,87)`, …

## 14. Subprocess determinism

Two `--tier gate` runs: identical coordinate / JSON / UV / corpus / Gate 1B2 channels / trace-data.

## 15–16. Compatibility

- Gate 1B2 class/PPM/PGM/counts MATCH
- Gate 2A0-4 numerical profile MATCH `af0041d388c61576e18a400a4f35a4220bd4981d34a05a42dacb6e77d97e888b`
- trace-data digest unchanged `b2c60252aea519866370774d97a8d8c1b9c7d626d3429fc2a1ae4b57a0f691a9`

## 17–18. Evaluator

- `result: PASS` / `authoritative: true` / `dirty: false`
- commit: `e76e8062b4a08add5bb7742726175f7f7b741010`
- content digest: `cefbbcb4cdf915be36f0594939a1eab13b5db27fb1710aed3426bab466673c3d`

## 19–20. CI / exclusions

- Local fmt/clippy/tests PASS (evaluator)
- Textures, radiometry, asymptotic correction, star fields, GPU/GUI — **not started**

Stop at Gate 2A1 boundary for owner review.
