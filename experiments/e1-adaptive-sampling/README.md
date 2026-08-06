# E1 — Physics-aware adaptive quadtree sampling

## Hypothesis

A deterministic adaptive sampler using relativistic lens-map and numerical
structure can reach lower scientific/image error at a given ray count than
uniform sampling and intensity-only adaptive sampling on the E0 corpus.

This package is an empirical experiment. It does **not** claim formal error
bounds, guaranteed subring detection, optimality, critical-curve proof, or
publishable improvement before owner review.

## E0 dependency

- Manifest: `experiments/oracle-benchmark/corpus-v1.toml`
- Lock: `experiments/oracle-benchmark/corpus-lock-v1.json`
- Required lock digest:
  `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb`
- Required baseline oracle digest (`kerr0999-edge-opaque`):
  `ee3c2c92f94ec291c172696fb9a4e75bccdea1bd019d20a74a9a4b3439eeb383`

The runner regenerates the E0 reference corpus into
`<output-dir>/reference/` with `--skip-committed-lock-update` and verifies lock
bytes against the committed lock before evaluation.

## Oracle isolation

The adaptive sampler receives scene, domain, method configuration, and a trace
callback. It does **not** receive `OracleFrame`, reference PPM, oracle
scientific values, or oracle error metrics. Oracle comparison happens only
after candidate reconstruction.

## Methods

| ID | Role |
| --- | --- |
| `uniform-quadtree-v1` | Level-ladder uniform splits |
| `intensity-only-adaptive-v1` | Adaptive on diagnostic RGB luma only |
| `physics-aware-adaptive-v1` | Adaptive on luma + outcome/lens/`g`/radiance/cost |

Ablations (three cases only): `physics-no-outcome`, `physics-no-lens-map`,
`physics-no-g`, `physics-no-radiance`, `physics-no-trace-cost`.

## Quadtree and stencil

Domains are square power-of-two pixel rectangles (128×128 sources, 64×64 crops).
Splits use fixed child order: top-left, top-right, bottom-left, bottom-right.

Conservative probe stencil V1 (before scoring):

1. four pixel corners
2. cell center
3. centers of the four would-be children

Duplicates are removed; indices sort row-major. Forced interior probes are
sentinels, not a proof that cell-interior substructure cannot be missed.

## Reconstruction

Common policy for all methods: `leaf-local-nearest-v1`.

For each output pixel, choose the traced sample in the leaf with minimum
squared integer distance; ties break by ascending source index. No
interpolation, blending, or oracle lookup. Later reconstruction research is
separate work.

## Ray accounting

Candidate unique ray count = cache misses on
`source_index = source_row * source_width + source_col`.
Oracle reference rays are reported separately and never counted as candidate
rays.

## Metrics and claim boundary

Scientific metrics mirror R1 comparison semantics (outcome / presence
independent; scalars only on compatible outcomes). RGB uses MSE and PSNR
sentinel for exact match. Hypothesis classification is corpus-bounded:

`SUPPORTED_ON_E0_CORPUS` / `MIXED_ON_E0_CORPUS` / `NOT_SUPPORTED_ON_E0_CORPUS`

E1 evaluator PASS does not require the hypothesis to win.

## Known blind spots

- Corner + forced child-center probes can still miss thin structure.
- Intensity-only may beat physics-aware on some budgets.
- Trace-cost can over-focus expensive but low-visual-impact rays.
- Leaf-local nearest reconstruction is intentionally crude.

## Commands

```bash
cargo run --release -p xtask -- \
  adaptive-sampling-experiment \
  --config experiments/e1-adaptive-sampling/config-v1.toml \
  --output-dir artifacts/e1-adaptive-sampling \
  --execution parallel \
  --threads 16 \
  --require-release

cargo run --release -p xtask -- evaluate --scope e1-adaptive-sampling
```

## Owner go/no-go

Stop at E1 owner research review. Do not start E2 ray differentials, E3 ray
bundles, Gate 2B2, spectra, physical RGB, OpenEXR, GPU, wgpu, egui, or GUI
without explicit authorization.
