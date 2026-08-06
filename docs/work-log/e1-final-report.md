# E1 final report — physics-aware adaptive quadtree sampling

## Result

Canonical experiment completed on branch `e1-physics-aware-adaptive-sampling`.

| Item | Value |
| --- | --- |
| Approved base | `86dd63dc537d5e4f41f5e798f5f30a4e3694558e` |
| Implementation commit | `a7b5c73` |
| Tracking | GitHub Issue #12 |
| E0 lock digest | `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb` |
| Baseline oracle digest | `ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383` |
| Config digest | `4b29b3837a756b74b8923f6a1182c0252396370bd5217b2a060468e1e4a2f666` |
| Experiment content digest | `ec96534facd4e39484ef3306908dfc19f98b766581fd02b7684f0cfcc027dd1a` |
| Hypothesis classification | `MIXED_ON_E0_CORPUS` |
| Recommendation | `ITERATE_E1_ESTIMATOR` |

## Matrix

- 8 E0 cases × 3 primary methods × 5 budget ladder points
- 5 physics-feature ablations × 3 cases
- Final full-coverage reconstructions are RGB/scientific exact
- Selected-sample oracle parity mismatches: 0

## Oracle isolation

Sampler APIs take scene/domain/method/trace callback only. `OracleFrame` and
reference PPM are consumed only after reconstruction for metrics and
selected-sample parity.

## Uniform observed final ray counts

- Source 128² finals: `16384` unique rays
- Crop 64² finals: `4096` unique rays

Adaptive budgets are derived from the corresponding uniform ladder counts.

## Claim boundary

E1 PASS does not require hypothesis support. Observed corpus-bounded result is
`MIXED_ON_E0_CORPUS`: physics-aware is not a consistent Pareto winner over both
baselines across the committed boundary crops and ≥3 sources under the stated
evidence rule.

## Exclusions confirmed

E2 ray differentials, E3 ray bundles, Gate 2B2, spectra, physical RGB, OpenEXR,
GPU, wgpu, egui, and GUI were not started.

## Owner review

Stop at E1 research review. Do not merge until owner accepts classification and
next-step recommendation. Do not start the recommended package without explicit
authorization.
