# Gate 2D1 final report — production scene appearance

## Result

**Authoritative evaluate PASS** on clean worktree after D1-V1 visual-semantic
closure. Merge still requires owner final review (not authorized here).

| Field | Value |
| --- | --- |
| D1-V1 implementation / evaluated tip | `345c560ab978b7ac8e00a57af0519441579c2068` |
| Evaluation content digest | `c146210d5914a9d8b3b429d03b4d5ca1936ff7b7d65682cc0355ca404b4b9e67` |
| Prior implementation tip (pre–D1-V1) | `8d7e13ac2dd8c7eeb7f12026aa942e746ff68334` |
| `result` | `PASS` |
| `authoritative` | `true` |
| `dirty` | `false` |
| Scope | `gate-2d1-scene-appearance` |
| Scientific inheritance | `SCIENTIFIC_INHERITANCE_PASS` |
| Presentation inheritance | `PRESENTATION_INHERITANCE_PASS` |
| Appearance pipeline | `APPEARANCE_PIPELINE_PASS` (includes 6× `d1_v1_*`) |
| Planning / merge base | `b832e4778cdfad9f061970c71dbb1b82fdb31188` |
| Architecture | D1-B + E1-B + S2 (one package) + D1-V1 diagnostics |
| Owner amendments | A1–A6 binding |

## Frozen inheritance (exact)

| Authority | Digest |
| --- | --- |
| 2C1 `physical_color_digest` | `16663188fad338c0fc8197dddd8268bd705f817b165a35853b16b211c7635793` |
| 2C1 `payload_sha256` | `d317c517661a64f8ffdacead3dd222370056abc8eed81706d660bc4ebda81cf5` |
| 2D0 `presentation_spec_digest` | `e6639e75d67156852f8f064e7ef9f4f2b82ab8018b707399c851522780a6dd49` |
| 2D0 identity `presentation_frame_digest` | `f8e103239a331796bd474ff121627eecd0781f31c840f46d9f2d3a85c8d1e87b` |

## A1–A6 evidence

| ID | Evidence |
| --- | --- |
| A1 | Stars use fixed `angular_sigma_rad = 0.015` (~0.86°); not pixel-scaled |
| A2 | `A(r) = A_max · sin²(π u)`, `u = clamp((r−r_in)/(r_out−r_in),0,1)`; zero at disk edges |
| A3 | Claim `ANNULAR_APPEARANCE_MEAN_PRESERVING`; diagnostic observer/display Δluma ≈ `+3.505%` (not required = 0) |
| A4 | Shared `present_exposed_linear_rgb`; identity path does not re-apply absolute→0.18 exposure |
| A5 | Identity RGB16 bit-exact vs Gate 2D0; `presentation_frame_digest = f8e10323…` |
| A6 | Canonical beauty path rejects `AffineLimit`/`Failed` (`SCENE_NUMERICAL_FAILURE`); gate beauty has zero numerical failures |

## Canonical scene beauty (gate-run-0)

| Field | Value |
| --- | --- |
| Artifact | `artifacts/gate-2d1-scene-appearance/gate-run-0/beauty-scene-srgb16.png` |
| `scene_appearance_digest` | `9107d571f999eade38052eb55ed66852382b6d9f6e43eb4a80e18c45049c2f28` |
| scene `presentation_frame_digest` | `68b555442c277c8eb95c1562568c24746fb2489c174730350671d5567cf43cd0` |
| `disk_appearance_spec_digest` | `c225cff70ca24fd9744545673a91becfeea10c75206f75c4deb3b787a7dc143e` |
| `environment_spec_digest` | `4ebc941b35c7d472c354074ab994af848ab81c6a6c4452034eeca3ee0aadc759` |
| serial ≡ parallel | PASS |

`APPEARANCE_REPRODUCIBILITY_DIGEST` — not scientific authority. Unchanged by D1-V1.

## D1-V1 visual-semantic closure

Owner product-review evidence; **not** beauty/scientific authority. Six checks
are required members of `APPEARANCE_PIPELINE_PASS`.

| Check | Result (gate 128) |
| --- | --- |
| `d1_v1_diagnostic_artifacts_present` | PASS |
| `d1_v1_source_mask_matches_bundle` | PASS |
| `d1_v1_disk_dominates_pixel_fraction` | PASS — DiskHit **0.7512**, Escaped **0.1490**, Horizon **0.0998** |
| `d1_v1_modulation_visible_on_disk` | PASS — `std≈0.0628`, `|m−1|>0.05` on **37.49%** lit disk |
| `d1_v1_bright_arc_candidates_recorded` | PASS — top hit `(90,65)` `g≈1.529`, `m≈1.000` (Doppler / inner-disk) |
| `d1_v1_resolution_consistency_evidence` | PASS — 128↔512 box MSE recorded (diagnostic; no frozen IQ threshold) |

Counts (gate 128 / 16384 px): DiskHit **12307**, Escaped **2442**, Horizon **1635**,
AffineLimit/Failed **0**.

Conclusion for prior visual blocker: large gray mass is opaque DiskHit
occlusion (~¾ frame), not environment/composition domain mix-up.

## Scope exclusions honored

Bloom / glare / E2 / E3 / GPU / wgpu / egui / GUI / 2D2 / 2D3 — not started.
Merge — **not authorized** pending owner final PR review.
