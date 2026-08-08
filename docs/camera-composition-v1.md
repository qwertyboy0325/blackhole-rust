# Camera composition V1 (Gate 2D3A)

C2 camera/composition layer over frozen Gate 2D1 scene appearance and Gate 2D0
presentation. Binding owner amendments **D3A-A1…A8**.

## Roles

| Role | Preset | Meaning |
| --- | --- | --- |
| `BASELINE_CAMERA` | `presets/camera/gargantua-baseline-v1.toml` | Exact Gate 2D1 framing; regression |
| `HERO_CAMERA` | `presets/camera/gargantua-hero-v1.toml` | Owner-selected production framing (**Phase B**) |
| `CANDIDATE_CAMERA` | search-generated | Temporary search candidates |

Physical / appearance / presentation TOML files are **never mutated**.

## Overlay allowlist (D3A-A1)

Camera preset may override only:

- `[observer]` motion / BL r / θ / φ
- `[camera]` projection / HFOV / look_at / roll

V1 freezes: `motion=zamo`, `look_at=black_hole_origin`, `roll=0` (D3A-A8).

Baseline overlay must bit-exact reproduce:

```text
presentation_frame_digest =
68b555442c277c8eb95c1562568c24746fb2489c174730350671d5567cf43cd0
```

## Authority (D3A-A2)

- **Baseline path** proves frozen 2C1 / 2D0 / 2D1 digests.
- **Hero / candidate** digests are `CAMERA_DERIVED_PRODUCTION_OUTPUT_NOT_SCIENTIFIC_AUTHORITY`.

## Search (D3A-A3…A6)

Frozen spec: `presets/camera/camera-search-spec-v1.toml` → `camera_search_spec_digest`.

- 48 candidates (4×3×2×2), fully deterministic
- Smoke 32² coarse invalidity only
- Gate 128² applies final hard filters + lex shortlist key (not unfrozen cinematic score)
- Class-fraction midpoints are `SEARCH_GUIDANCE_NOT_GATE_TRUTH`

## Workflow

```text
Phase A: camera-search-phase-a → STOP_FOR_OWNER_HERO_SELECTION
Phase B: owner picks candidate_id → freeze hero → authoritative evaluate
```

```bash
cargo run --release -p xtask -- camera-search-phase-a --threads N
cargo run --release -p xtask -- evaluate --scope gate-2d3a-camera-composition
```
