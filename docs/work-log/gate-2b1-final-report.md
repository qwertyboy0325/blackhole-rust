# Gate 2B1 final report — diagnostic bolometric radiance

## Result

Authoritative `PASS` evaluated at commit `ab214c382db6754b03fee2ec13909a99fb3bdf6e`
(owner closures for emission-claim provenance + disk-bounds invariant).

| Item | Value |
| --- | --- |
| Base | `0d0c2fc6627120f285bdf393d90b973df654a523` |
| Branch | `gate-2b1-diagnostic-bolometric-radiance` |
| Draft PR | #13 |
| Owner closure comment | `5200367497` |
| Evaluator digest | `5c2e9685afb118ab9360a69f70145bea40a4528f542333e29be099acf714c323` |

## Emission model

Frozen V1 `diagnostic-radial-power-law-v1`: `I_em = (r_inner/r)^3`, normalization 1,
isotropic emitter frame, arbitrary normalized bolometric specific intensity.

Emission-spec digest: `95347496d2ade139a6002bb5d2ef70a4ba4b77085eac4a7b6232a49f9fd1c0db`

Resolved disk bounds from trace scene: **inner = 3**, **outer = 20**
(`resolved-trace-scene-thin-disk-v1`). No ISCO geometry claim.

## Emission claim provenance (closure)

Pre-trace exact-string validation of preset:

```text
emission_model = "diagnostic_radial_profile"
emission_claim = "project diagnostic, not astrophysical or film-asset reconstruction"
```

Accepted model/claim are written into the bolometric map artifact, worker report,
and scientific digest. Altered claim and unsupported model CLI negatives reject
with no artifacts.

## Disk-bounds invariant (closure)

`ResolvedDiskBounds` fields are private; `new()` / `Deserialize` / `validate()`
enforce finite, `inner > 0`, `outer > inner`. Public scientific entry points
re-validate. Illegal construction/deserialization is typed-rejected; never clamped.

## Transport

Convention `diagnostic-bolometric-disk-g4-v1`. Source Gate 2B0 frequency digest
`65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2`.
`I_obs = g⁴ I_em` with canonical `(g*g)*(g*g)`. Max transport residual: `0`.

## Authoritative accounting

| Metric | Value |
| --- | --- |
| disk_hit / mapped / fail | 12307 / 12307 / 0 |
| attenuated / boosted / unchanged | 8293 / 4014 / 0 |
| bolometric_digest | `d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2` |
| JSON digest | `90e78a0fc45ea61ae89db539a475e0aa8488f771f53fab6025c178afb4e14021` |
| emitted / observed / composite PPM | `e8476d69…` / `c50aa789…` / `7982aaa9…` |

Bolometric scientific digest changed vs pre-closure (`17addd12…`) because accepted
emission model/claim are now hashed into the digest. Visualization PPM digests
unchanged.

## Compatibility

Gate 1B2 / 2A1 / 2A2 / 2B0 frequency artifacts unchanged. Frequency-only and
no-flag workers retain exact prior digests. Subprocess and smoke thread-count
equivalence hold.

## Exclusions

No spectra, blackbody/temperature, physical RGB, ACEScg, OpenEXR, GPU, wgpu, or
egui work started.

## Owner review

Stop at Gate 2B1 boundary. Do not merge until owner accepts closures.
Do not start R1/E0 or Gate 2B2.
