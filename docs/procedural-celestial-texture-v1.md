# Procedural celestial texture V1

Diagnostic coordinate/orientation field for Gate 2A2 lensed celestial images.

## Purpose

Provide a deterministic, project-owned RGB field so escaped rays can sample a
structured sky through Gate 2A1 finite-boundary coordinates. The result is a
**diagnostic** lensed image, not physical radiometry and not a reconstruction of
any film frame.

## Coordinate source

Samples use Gate 2A1 celestial boundary samples:

```text
finite-oblate-escape-boundary-position
```

Not asymptotic directions at null infinity. No asymptotic correction is applied.

## Texture ID

```text
procedural-coordinate-grid-v1
```

Canonical specification fields (frozen exact V1 — `validate()` rejects any
mutation of these values):

| Field | Value |
| --- | --- |
| schema_version | 1 |
| longitude_sectors | 8 |
| latitude_cells | 12 |
| minor_longitude_divisions | 24 |
| minor_latitude_divisions | 12 |
| major_longitude_stride | 3 |
| major_latitude_stride | 3 |
| marker_radius_millidegrees | 7000 (7°) |

No external texture assets. No star catalogs. No copyrighted sky maps.
Non-canonical specs must fail with a typed `CelestialRenderError` before any
palette indexing; they must not panic.

## Algorithm

Given normalized UV `(u,v)` and unit coordinate direction `n`:

1. **Markers** (angular distance on the unit sphere, fixed priority):
   +X, +Y, −X, −Y, north pole, south pole. Marker radius = 7°.
2. **Seam** near `u = 0` (wraps): RGB `[255, 48, 48]`.
3. **Equator** near `v = 0.5`: RGB `[255, 255, 255]`.
4. **Major grid**, then **minor grid**, using normalized distance to nearest
   cell boundary in minor-cell units.
5. **Base sectors**: `sector = floor(u * 8)` with north palette for `v < 0.5`
   and south palette for `v >= 0.5`.
6. **Checker**: for alternating `floor(u*24)` / `min(floor(v*12), 11)` cells,
   add 18 to each channel with saturating arithmetic.

### North palette

```text
0 [ 28,  72, 160]
1 [ 24, 120, 142]
2 [ 40, 132,  76]
3 [152, 132,  36]
4 [164,  72,  36]
5 [148,  48, 132]
6 [ 84,  60, 168]
7 [ 40, 104, 176]
```

### South palette

```text
0 [ 16, 40,  96]
1 [ 16, 72,  84]
2 [ 24, 80,  48]
3 [ 92, 80,  24]
4 [100, 44,  24]
5 [ 88, 28,  80]
6 [ 48, 36, 104]
7 [ 24, 60, 108]
```

### Grid colors

```text
minor grid: [112, 112, 112]
major grid: [232, 232, 232]
```

### Artistic diagnostic line widths (frozen)

These are artistic diagnostic parameters, not physical angular widths:

```text
MINOR_LINE_HALF_WIDTH_CELL = 0.06
MAJOR_LINE_HALF_WIDTH_CELL = 0.10
EQUATOR_HALF_WIDTH_V       = 0.004
SEAM_HALF_WIDTH_U          = 0.003
```

Changing them without recording the change in the texture specification /
digest contract is forbidden.

## What RGB does not mean

Values are not specific intensity, spectral radiance, physical stars, exposure,
redshift, Doppler beaming, disk emission, or film grading.

## Opaque vs disk-omitted

| Mode | Surface set | Meaning |
| --- | --- | --- |
| Opaque disk mask | `opaque-disk-horizon-escape` | Existing ThinDisk + horizon + escape |
| Disk-omitted celestial diagnostic | `horizon-escape-only` | Horizon + escape only; no disk surface registered |

Disk omission is a **separate trace configuration**. It is not a transparent
physical disk and must not be described as seeing through the opaque disk.

## Reference atlas

`render_procedural_texture_reference` produces an equirectangular visualization
of the same procedural function (canonical 512×256). The lensed renderer does
**not** sample that raster; it evaluates the procedural function directly from
each Gate 2A1 coordinate sample.

## Known limitations

- One sample per pixel (pixel-center UV / direction) → aliasing of grid lines
  and markers is expected.
- No filtering, anti-aliasing, or multi-sample.
- Finite escape boundary only; not null infinity.
