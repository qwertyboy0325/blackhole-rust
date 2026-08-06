# E1 final report — physics-aware adaptive quadtree sampling

## Result

Owner closure `5201833970` complete. Authoritative evaluate **PASS** with full
final scientific exactness, corrected matched-budget hypothesis semantics,
explicit optional scientific dimensions, structured failure categories, and
repeat `outcome-disagreement.pgm` comparison.

| Item | Value |
| --- | --- |
| Approved base | `86dd63dc537d5e4f41f5e798f5f30a4e3694558e` |
| Evaluated commit | `d1f5a60898ac229450a78e533544fd57b991961f` |
| Tracking | GitHub Issue #12 / Draft PR #15 |
| E0 lock digest | `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb` |
| Baseline oracle digest | `ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383` |
| Experiment digest | `f1690e46caf2fee90fab0b92d1d9c19b783d9b868c1de447870217e39bb2932f` |
| Evaluate digest | `4d84e629a2a885cab93996caef6a7c7291a206c544e2c23212ac308f2d64e3b9` |
| Hypothesis classification | `NOT_SUPPORTED_ON_E0_CORPUS` |
| Recommendation | `PAUSE_RESEARCH_WEDGE` |

```text
E1 implementation core           PASS
CI                               (tip CI after push)
Authoritative evaluator          PASS
NOT_SUPPORTED classification     FINAL (recomputed; not ray-count artifact)
PR #15                           DRAFT / NOT MERGED
Performance planning prompt      HOLD
E2 / Gate 2B2                    NOT STARTED
```

## Closure checks (`5201833970`)

1. **Final scientific exactness** — finals require exact ray counts
   (`16384` source / `4096` crop), RGB exact, parity zero, outcome/RHS/presence
   zero, and all applicable scientific scalars exact zero.
2. **Matched-budget hypothesis** — cross-method wins use
   `error_improves_at_matched_budget` (no `cand.rays <= base.rays` double gate).
   Same-method frontier still uses ray-aware `dominates`. After correction,
   classification remains `NOT_SUPPORTED_ON_E0_CORPUS` (physics-aware still does
   not meet the crop/source win thresholds).
3. **Optional scientific metrics** — mixed `Some`/`None` on an applicable
   dimension blocks dominance and matched wins; case-level consistency checked.
4. **Failure + repeat** — `failure-analysis.json` includes
   observed/not-observed categories; repeat byte-compares
   `outcome-disagreement.pgm`.

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

## Claim boundary

E1 evaluate PASS does not require hypothesis support. Corpus-bounded result is
`NOT_SUPPORTED_ON_E0_CORPUS` under corrected matched-budget and optional-metric
semantics. Recommendation: `PAUSE_RESEARCH_WEDGE`.

## Exclusions confirmed

E2 ray differentials, E3 ray bundles, Gate 2B2, spectra, physical RGB, OpenEXR,
GPU, wgpu, egui, and GUI were not started. Execution-performance restructuring
remains HOLD until after owner accepts merge.

## Owner review

Stop at E1 research review. Do not merge until owner accepts. Do not start
performance restructuring or E2 / Gate 2B2 without explicit authorization.
