# Gate 2A2 — First Deterministic Lensed Celestial Diagnostic Image

## Result

Authoritative evaluate **PASS** at commit `f880e77c172415b343eb07371bb20839f75c78ed`.

V1 validation closure (frozen exact `validate()`, sector>8 → `Err` not panic) follows on this branch; digests below must remain unchanged.

| Field | Value |
| --- | --- |
| Approved base | `bab17d21b9e5ff5d153a0f1a7dc7ec46e861df87` |
| Branch | `gate-2a2-first-lensed-celestial-image` |
| Evaluate digest | `4accefb2bcb18fd6b24b59d3c5f5fe309f894a2c7f8d57c4957703dec2047711` |
| Texture spec digest | `6b06bf21a607510a981c5ec7d2521e4d4d9beccb7d5354d29dbbb1520edf495a` |
| Reference atlas (512×256) | `783aba0cde1020045b9a60a5fcd080ad0ecbaed712f9ec482dac744623b41c4c` |

## Renderer crate boundary

```text
relativity-render → relativity-trace → relativity-integrate / relativity-core
```

`relativity-render` has `#![forbid(unsafe_code)]`, no filesystem/CLI/GPU/windowing, no tracing loop. It operates on completed Gate 2A1 coordinate frames and returns validated `RgbFrame` values.

## Procedural texture V1

ID `procedural-coordinate-grid-v1`. Spec documented in `docs/procedural-celestial-texture-v1.md`. RGB is a diagnostic coordinate/orientation field, not radiometry.

## Surface-set API

```text
trace_grid / trace_grid_with_execution
  → TraceSurfaceSet::OpaqueDiskHorizonEscape

trace_grid_with_execution_and_surface_set(..., HorizonEscapeOnly)
  → disk-omitted celestial diagnostic (no ThinDisk registered)
```

Existing APIs unchanged in behavior. Disk omission is a separate trace configuration, not a transparent physical disk.

## Opaque Gate digests (128×128)

| Artifact | Digest |
| --- | --- |
| Trace data | `b2c60252aea519866370774d97a8d8c1b9c7d626d3429fc2a1ae4b57a0f691a9` |
| Class | `64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4` |
| Coordinate | `5d8df5ba007beeb3742ef9c3a684dbd86704f6b9a29271356e87d07fc2c71328` |
| Coordinate JSON | `e37b8f32990aa8dd95557899ccdc80fd5d38bec5ace7fccef18541b666cb61ca` |
| UV-debug PPM | `4262eb4fe84937557cf3679fa390d2883151a2aaf25e9b973d6297acfe8f2107` |
| Categorical PPM | `ac058d5af01b425e411b5c33017210bf888aa52918cfd085bb863d7ddc99184c` |
| RHS PGM | `2df226390057bb87b64d172cd258087b0ef4c1ad0ce0d4378e003b5861a75db5` |
| Lensed PPM | `e4cb10b98e97793ddbf365edc1bdf29fde32e70afc7b05604275bc78a335de0a` |

Counts: disk_hit 12307 / escaped 2442 / horizon_event 1462 / horizon_approach 173 / affine_limit 0 / failed 0. Texture samples = 2442.

Gate 1B2 and Gate 2A1 opaque references retained exactly.

## Disk-omitted Gate digests (128×128)

| Artifact | Digest |
| --- | --- |
| Trace data | `b3ea05de07e071ac9733a79120ad13f6b6c63e6effc133ae7c97eba0d4cc644a` |
| Class | `f88083886662d7862796b9c08f53be9b5d528488bc396c49f1a4fd6d07d169e0` |
| Coordinate | `05b1b330233150e43506573c85d72429335179417e0056a69ac091ba3b1976c3` |
| Lensed PPM | `4c78e57082eea9dc7226944e9d055f15ce8ed3f58058d613daaaa0ec3c0577d5` |

Counts: disk_hit 0 / escaped 12864 / horizon_event 3160 / horizon_approach 352 / affine_limit 8 / failed 0. Texture samples = 12864 (> opaque).

Two independent opaque workers byte-identical; two independent disk-omitted workers byte-identical.

## Showcase

Path: `artifacts/gate-2a2-lensed-celestial/showcase-disk-omitted/`  
Lensed PPM digest: `400afe080578b503ba3cc13e7788fbe87254a2e867e0783b686e732945ebeb58`  
Texture samples: 51486 (non-authoritative 256×256).

## Explicit non-starts

No radiometry, redshift, Doppler, disk emission, asymptotic correction, OpenEXR, GPU, wgpu, or egui.

## Owner stop

Stop at Gate 2A2 boundary for owner review. Do not merge without owner acceptance.
