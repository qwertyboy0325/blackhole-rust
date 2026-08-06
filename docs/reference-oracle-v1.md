# Reference Oracle V1

OracleFrame V1 is the stable CPU `f64` export boundary over the accepted
scientific channels: typed ray outcome, finite-boundary celestial direction,
disk frequency shift `g`, diagnostic emitted bolometric intensity, observed
`g^4`-transported bolometric intensity, and per-ray RHS cost.

`relativity-oracle` depends inward on `relativity-core`, `relativity-trace`, and
`relativity-render`. No existing crate depends on it, and it contains no CLI,
filesystem access, subprocess execution, wall-clock measurement, GPU, windowing,
or integration algorithm.

## Channel Sets

`GeometryCelestial` exports outcome class, RHS evaluations, failure class where
applicable, and celestial boundary coordinates for escaped rays.

`FullBolometricDisk` exports all geometry/celestial channels plus frequency and
bolometric payloads for every `DiskHit`. It is valid only with
`TraceSurfaceSet::OpaqueDiskHorizonEscape`.

Celestial directions are finite oblate escape-boundary coordinates. They are not
null-infinity directions. Disk frequency uses the accepted backward-covector
circular equatorial emitter `g` factor. Bolometric intensity is diagnostic
arbitrary-unit specific intensity with `g^4` transport. Spectra and physical RGB
remain not implemented.

## Coordinates And Digest

Source frames use the existing `sensor_at_pixel_center(trace.grid, col, row)`
mapping exactly. Crops preserve source coordinates and sensor positions while
local coordinates are re-indexed row-major from zero. Crop-of-crop sensor
windows are sub-windows of the source `sensor_window`, never remapped as if the
source were the full `[-1,1]^2` frame.

`OracleFrame::validate()` seals the public invariant: schema/oracle ID,
dimensions/pixel length, row-major indices, source coordinates
(`source_index`/`source_col`/`source_row` row-major consistency; full-frame
identity with local coords), sensor window membership, outcome/channel
consistency, finite/range/positivity rules, and equality of the stored
scientific digest with a recomputation. Deserialization and public scientific
entry points (`build`, `crop`, `compare`) reject malformed frames; they never
clamp.

The oracle scientific digest hashes schema version, oracle ID, scientific claim,
dimensions, sensor window, surface set, channel set, source scientific digests,
and every row-major pixel using explicit domain separators, project-owned enum
tags, length-prefixed strings, fixed-endian integers, and `f64::to_bits()`.

The digest excludes case IDs, artifact paths, wall-clock time, host, PID, memory
measurement, PPM bytes, presentation metadata, `Debug`, `Display`, and serde
bytes as the sole schema.

Oracle JSON artifacts encode every scientific `f64` as a 16-digit lowercase hex
`to_bits()` string so deserialization is bit-exact and the stored scientific
digest remains verifiable after load.

Comparison metrics count outcome disagreement and channel-presence mismatch
independently. Scalar channel errors are accumulated only when outcomes are
compatible and both sides carry the channel.

## Limitations

OracleFrame V1 freezes accepted diagnostic channels only. It does not provide
adaptive sampling, ray differentials, spectra, physical RGB, OpenEXR, GPU
execution, GUI integration, or formal error guarantees.
