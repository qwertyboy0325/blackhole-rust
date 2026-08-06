# R1/E0 final report — reference oracle and benchmark corpus

## Result

Authoritative `PASS` evaluated at commit `EVAL_COMMIT_PLACEHOLDER`
(owner closures for evaluator authority, OracleFrame invariant, presence metrics).

| Item | Value |
| --- | --- |
| Base | `dcceef661574d21ce4c0aa8817fcf9d9fa1039a1` |
| Branch | `r1-e0-oracle-benchmark-corpus` |
| Draft PR | #14 |
| Owner closure comment | `5200642850` |
| Reviewed head before closures | `9f8681e9a22283a97caca28c04b685fbaea47272` |
| Evaluator digest | `EVAL_DIGEST_PLACEHOLDER` |
| Corpus lock digest | `647cb722b8ca5bc83b7ec77bfa612c97429ead61e36f10d47db75ade269941fb` |

## Closures

### Authoritative evaluator

`evaluate --scope r1-e0-oracle-corpus` regenerates the corpus (parallel ×2,
serial, CLI subprocess), compares locks to the committed
`experiments/oracle-benchmark/corpus-lock-v1.json`, validates all eight
oracle frames under checked deserialization, and checks inherited Gate digests
for `kerr0999-edge-opaque`. Writes `evaluation.json` with content digest.

### OracleFrame public invariant

`OracleFrame::validate()` seals schema/oracle ID, dimensions/length, row-major
indices, sensor-window membership, outcome/channel consistency,
finite/range/positivity, and stored scientific digest equality. Deserialize,
`build`, `crop`, and `compare` call it. Crop uses checked pixel access;
crop-of-crop sensor windows are sub-windows of the source window. Scientific
`f64` fields are JSON-encoded as hex `to_bits()` for bit-exact load.

### Channel-presence metrics

`compare_oracle_frames` counts outcome disagreement and disk/celestial presence
mismatch independently. Scalar errors accumulate only when outcomes are
compatible and both sides carry the channel.

## Inherited Gate digests (`kerr0999-edge-opaque`)

| Channel | Digest |
| --- | --- |
| numerical profile | `af0041d3…` |
| outcome class | `64462a83…` |
| celestial coordinate | `5d8df5ba…` |
| frequency shift | `65df7b55…` |
| bolometric | `d3721de7…` |
| composite PPM | `7982aaa9…` |
| counts | 12307 / 2442 / 1462 / 173 / 0 / 0 |

## Exclusions

E1 adaptive sampling, Gate 2B2, spectra, physical RGB, OpenEXR, GPU, wgpu, and
egui remain out of scope.

## Owner review

Stop at R1/E0 boundary. Do not merge until owner accepts closures.
