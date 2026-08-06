# Gate 2B0 final report — frequency-shift kinematics

## Result

Authoritative `PASS` evaluated at commit `5d1b85e520e79c78042bff80324f64fa48f87f51`.

| Item | Value |
| --- | --- |
| Base | `33a8248c6b92e13a2c6b90187c6741e89b7fb1ab` |
| Branch | `gate-2b0-frequency-shift-kinematics` |
| Evaluator digest | `80d7d745686748e1c9a91859bd0ca130de6057b8c3756b3dc214813f27104abe` |

## Orientation and APIs

- Stored momentum: past-directed covector `p_backward`
- Equivalent future: `k_future = -p_backward`
- Measured frequency: `ν = p_backward_μ u^μ = -k_future_μ u^μ` (strictly positive)
- Core: `MeasuredFrequency`, `FrequencyShift`, backward/future constructors, `frequency_shift_ratio`
- Circular equatorial: `Ω_s = s √M / (r^{3/2} + s a √M)`, prograde `a≥0 → +φ`, `a<0 → −φ`
- Camera: `ν_obs = 1` (`camera-local-unit-past-null`); gate ratio `g = 1/ν_em`
- Max observer unit-frequency residual: `6.661338147750939e-16` (≤ `1e-10`)

## Authoritative Gate accounting

| Metric | Value |
| --- | --- |
| disk_hit / mapped / fail | 12307 / 12307 / 0 |
| redshifted / blueshifted / unity | 8293 / 4014 / 0 |
| min g | 0.27800904424370243 @ (22, 69) |
| max g | 1.529479565030175 @ (90, 65) |
| closest to unity | 1.0000072711413068 @ (86, 109) |
| max \|disk radius residual\| | 0 |
| frequency_shift_digest | `65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2` |
| JSON digest | `a2f440e76bc0f89c539e7dcb7ab76171a3dc84d67a26185871fe8678c9ed7106` |
| g-factor debug PPM digest | `30b6cf872056fdfa59021bd58bbad15a0cf24a234f31fe80cfc5bc0cfbc0fb6f` |
| resolved direction | `positive-phi` |

Passes: trace=1, observer verification=1, frequency-shift=1. Two Gate workers byte-identical for scientific digest, JSON, and debug PPM. Smoke thread-count equivalence holds.

## Compatibility

Gate 1B2 class/PPM/PGM/counts, Gate 2A1 coordinate digest, Gate 2A2 opaque lensed PPM and texture-spec digest unchanged. Without `--emit-disk-frequency-shift`, optional report field omitted and 2A2 references intact.

## Exclusions confirmed

No emission profiles, `g³`/`g⁴` transport, temperature, spectra, physical RGB, OpenEXR, GPU, wgpu, or egui work in this gate.

## Owner review

Stop at Gate 2B0 boundary. Min/max `g` above are first-run observations, not permanent hard references until owner acceptance.
