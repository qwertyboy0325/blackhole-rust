# E1 final report — physics-aware adaptive quadtree sampling

## Result

Owner closure `5201409295` complete. Authoritative evaluate **PASS** with full
repeat determinism, semantic evaluator checks, and scientific Pareto/failure
dimensions. Hypothesis classification updated under those semantics.

| Item | Value |
| --- | --- |
| Approved base | `86dd63dc537d5e4f41f5e798f5f30a4e3694558e` |
| Evaluated commit | `815b1780447491e7085e8045f2b5706e533f0101` |
| Tracking | GitHub Issue #12 / Draft PR #15 |
| E0 lock digest | `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb` |
| Baseline oracle digest | `ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383` |
| Experiment digest | `8f335b68cfcdade8d42b87fff88af8c6a2eb13dfc281d6bd09492801a1a39688` |
| Evaluate digest | `fcabe78e0374df45d60d0f397683faee3bc5140cace30d34b4311c9fed582a40` |
| Hypothesis classification | `NOT_SUPPORTED_ON_E0_CORPUS` |
| Recommendation | `PAUSE_RESEARCH_WEDGE` |

```text
E1 core implementation          PASS
E1 hypothesis result            NOT_SUPPORTED_ON_E0_CORPUS
Repeat determinism evidence     PASS
Evaluator semantic closure      PASS
Pareto/failure semantics        PASS
PR #15                          DRAFT / NOT MERGED
E2 / Gate 2B2                   NOT STARTED
```

## Closure checks (owner blockers)

1. **Repeat determinism** — both boundary crops re-run physics-aware; sample
   schedules, PPM/PGM, metrics JSON, and curve digests (points only) match the
   canonical publish.
2. **Evaluator semantics** — validates 8×3×5 matrix + artifacts, ablation
   3×5×5, sample parity = 0, finite metrics, final full-coverage exactness,
   scope exclusions via Cargo deps + real import lines (not comment/string
   mentions).
3. **Pareto / failure** — primary dimensions include ray count, outcome
   disagreement, RGB MSE, celestial angular RMSE, and `log2(I_obs)` RMSE when
   present. Failure records include target/provenance coordinates, leaf
   rectangle/depth, and feature vector at last split when applicable.

## Matrix

- 8 E0 cases × 3 primary methods × 5 budget ladder points
- 5 physics-feature ablations × 3 cases
- Final full-coverage reconstructions are RGB/scientific exact
- Selected-sample oracle parity mismatches: 0
- Serial/parallel determinism smoke: PASS

## Oracle isolation

Sampler APIs take scene/domain/method/trace callback only. `OracleFrame` and
reference PPM are consumed only after reconstruction for metrics and
selected-sample parity.

## Uniform observed final ray counts

- Source 128² finals: `16384` unique rays
- Crop 64² finals: `4096` unique rays

Adaptive budgets are derived from the corresponding uniform ladder counts.

## Claim boundary

E1 evaluate PASS does not require hypothesis support. Under full scientific
Pareto dimensions, physics-aware does not Pareto-beat both baselines on the
committed boundary crops or on ≥1 source case under the stated evidence rule
(`min_points=2`). Prior provisional `MIXED_ON_E0_CORPUS` (RGB/outcome-only) is
superseded; owner allowed classification to change after semantic closure.

## Exclusions confirmed

E2 ray differentials, E3 ray bundles, Gate 2B2, spectra, physical RGB, OpenEXR,
GPU, wgpu, egui, and GUI were not started.

## Owner review

Stop at E1 research review. Do not merge until owner accepts classification and
`PAUSE_RESEARCH_WEDGE`. Do not start E2 / Gate 2B2 without explicit
authorization.
