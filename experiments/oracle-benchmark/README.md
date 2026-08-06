# E0 Oracle Benchmark Corpus

E0 creates owner-reviewable experimental baselines for later rendering-method
experiments. It consumes the R1 OracleFrame V1 schema but does not create
permanent scientific governance for temporary timing, memory, or summary
statistics.

The corpus has six 128x128 source cases:

- `kerr0999-edge-opaque`
- `kerr0999-edge-sky`
- `kerr0999-midinc-opaque`
- `kerr0999-midinc-sky`
- `kerr050-edge-sky`
- `schwarzschild-edge-sky`

It also derives two 64x64 outcome-boundary-rich candidate crops from the two
edge baseline sources. The crop selector enumerates every 64x64 window with
top-left coordinates that are multiples of 8, counts horizontal and vertical
outcome-class transitions, then breaks ties by lowest top and lowest left.

Metrics include oracle scientific comparison metrics, seam-aware celestial `u`
error, log2 disk-channel errors, RGB MSE/PSNR, ray counts, and performance
evidence. Timing and memory are excluded from deterministic lock content.

E1 will consume these source and crop oracle frames as comparison references for
adaptive-method experiments. E0 makes no formal error claim and does not
implement adaptive subdivision, supersampling, spectra, physical RGB, OpenEXR,
GPU, or GUI work.
