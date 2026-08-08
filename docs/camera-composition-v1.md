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

Frozen spec: `presets/camera/camera-search-spec-v1.toml` → exact
`camera_search_spec_digest` pin (D3A-A3 / D3A-C1):

```text
bc5b9257492310c612e2ac26d58926b761d31ff4acbd3fe5f2e77d98a3d9191b
```

- 48 candidates (4×3×2×2), fully deterministic
- Smoke 32² coarse invalidity only
- Gate 128² applies final hard filters + lex shortlist key (not unfrozen cinematic score)
- Class-fraction midpoints are `SEARCH_GUIDANCE_NOT_GATE_TRUTH` (D3A-A6 / D3A-C2:
  recorded as evaluator `DIAGNOSTIC` evidence; never authoritative PASS/FAIL)

## Workflow

```text
Phase A: camera-search-phase-a → STOP_FOR_OWNER_HERO_SELECTION
Phase B: owner picks candidate_id → freeze hero → authoritative evaluate
```

Owner selection (Phase B freeze): **`c024`** → `presets/camera/gargantua-hero-v1.toml`.

Frozen hero digests (gate 128²; camera-derived, not 2C1 scientific authority):

```text
camera_spec_digest =
42d3e3f8cc5d7b11950439ab46a850d6f5e2865f8e37a29ba6570f01b9ad2578

presentation_frame_digest =
fae0afdd2b16a1ff8c086303edbf633e675595f6e81620dde483e690e7266544

scene_appearance_digest =
b3c8f30afd3575215a8c75d2c5e82a0710f739e4d330a88e575de7880ccede84
```

```bash
cargo run --release -p xtask -- camera-search-phase-a --threads N
cargo run --release -p xtask -- evaluate --scope gate-2d3a-camera-composition
```
