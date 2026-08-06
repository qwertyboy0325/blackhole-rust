# Gate 2B1 final report — diagnostic bolometric radiance

## Result

Authoritative `PASS` evaluated at commit `ce6415fd1c0bc3189549c764bdc217ff2fda0210`.

| Item | Value |
| --- | --- |
| Base | `0d0c2fc6627120f285bdf393d90b973df654a523` |
| Branch | `gate-2b1-diagnostic-bolometric-radiance` |
| Evaluator digest | `285fd11b29f1d7e3643b6d4bedabdaed33c5009f3787307a233a582c1f54e534` |

## Emission model

Frozen V1 `diagnostic-radial-power-law-v1`: `I_em = (r_inner/r)^3`, normalization 1,
isotropic emitter frame, arbitrary normalized bolometric specific intensity.

Emission-spec digest: `95347496d2ade139a6002bb5d2ef70a4ba4b77085eac4a7b6232a49f9fd1c0db`

Resolved disk bounds from trace scene: **inner = 3**, **outer = 20**
(`resolved-trace-scene-thin-disk-v1`). No ISCO geometry claim.

## Transport

Convention `diagnostic-bolometric-disk-g4-v1`. Source Gate 2B0 frequency digest
`65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2`.
`I_obs = g⁴ I_em` with canonical `(g*g)*(g*g)`. Max transport residual: `0`.

## Authoritative accounting

| Metric | Value |
| --- | --- |
| disk_hit / mapped / fail | 12307 / 12307 / 0 |
| attenuated / boosted / unchanged | 8293 / 4014 / 0 |
| bolometric_digest | `17addd12645a78220428d1074fb257fc74806f425a87cddb45deb2240c14b304` |
| JSON digest | `335251165788827e5a5bf0b946900bfa2f0381edd6642b830129bb12bb2e7e73` |
| emitted / observed / composite PPM | `e8476d69…` / `c50aa789…` / `7982aaa9…` |

Min/max emitted and observed intensities (first-run; not permanent refs until
owner review) are recorded in `artifacts/gate-2b1-bolometric-radiance/evaluation.md`.

## Compatibility

Gate 1B2 / 2A1 / 2A2 / 2B0 frequency artifacts unchanged. Frequency-only and
no-flag workers retain exact prior digests. Subprocess and smoke thread-count
equivalence hold.

## Exclusions

No spectra, blackbody/temperature, physical RGB, ACEScg, OpenEXR, GPU, wgpu, or
egui work started.

## Owner review

Stop at Gate 2B1 boundary.
