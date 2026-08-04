# Gate 2A0-3 Final Report — Trace-Once / Shade-Many

## Status

Pending authoritative `evaluate --scope gate-2a0-trace-shade`.

## 1. Base / branch

- Base: `85d11379705914c1cfbea657d386b82b142dd3e0`
- Branch: `gate-2a0-trace-shade-separation`

## 2. Layout

- `shade.rs`: `RgbFrame`, styles, `shade_trace_bundle` / `shade_diagnostic` / `shade_many`
- `image.rs`: `encode_ppm(RgbFrame)`; `write_outcome_ppm` = legacy wrapper
- `trace_digest.rs`: `trace_data_digest` (IEEE bit patterns; no shading)

## 3–14. Results

TBD after release evaluate. Celestial-sphere rendering not started.
