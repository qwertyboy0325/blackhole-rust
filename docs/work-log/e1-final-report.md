# E1 final report — physics-aware adaptive quadtree sampling

## Result

Pending authoritative evaluate after canonical experiment completion.

| Item | Value |
| --- | --- |
| Approved base | `86dd63dc537d5e4f41f5e798f5f30a4e3694558e` |
| Branch | `e1-physics-aware-adaptive-sampling` |
| Tracking | GitHub Issue #12 |
| E0 lock digest | `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb` |
| Baseline oracle digest | `ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383` |

## Scope

Empirical comparison of:

- `uniform-quadtree-v1`
- `intensity-only-adaptive-v1`
- `physics-aware-adaptive-v1`

on all eight E0 cases, with five physics-feature ablations on three cases.

## Oracle isolation

The sampler receives scene/domain/method/trace callback only. OracleFrame and
reference PPM are used only after reconstruction for metrics and selected-sample
parity.

## Exclusions confirmed

E2 ray differentials, E3 ray bundles, Gate 2B2, spectra, physical RGB, OpenEXR,
GPU, wgpu, egui, and GUI were not started.

## Owner review

Stop at E1 research review. Do not merge until owner accepts the hypothesis
classification and next-step recommendation.
