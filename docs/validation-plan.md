# Validation and autonomous evaluation plan

## Principle

Validation is layered because equations, solvers, events, radiometry, and images
fail differently. A plausible image can coexist with wrong time orientation,
missed disk crossings, or large invariant drift near the critical curve.

No numeric acceptance threshold in Gate 1 may be adopted merely because it is a
round number or makes the current implementation pass. The preset contains
**provisional run controls**, not acceptance limits.

## Calibration protocol

For each observable or invariant:

1. choose analytic/limiting cases and a stratified ray corpus, oversampling
   horizon grazers, disk tangencies, turning points, and critical directions;
2. compute a convergence ladder by tightening tolerances and maximum step size;
3. where possible, compare to the Carter-separated or analytic solution at
   higher precision;
4. estimate the converged value and observed order, separating truncation,
   event-localization, spectral, and floating-point error;
5. set an acceptance budget above the measured reproducibility/noise floor and
   below the smallest scientifically material difference;
6. record the corpus, raw sweep, plot, rationale, hardware/toolchain, and owner
   approval in a versioned calibration report;
7. never relax the budget without a new report and ADR/review.

Different ray classes may require different budgets. Classification mismatches
are categorical failures unless the approved reference explicitly marks a
critical-curve uncertainty band.

## Test layers

### Algebraic and unit tests

- Kerr-Schild metric symmetry and inverse identity;
- `a -> 0` reduction to Schwarzschild Kerr-Schild;
- large-radius approach to Minkowski space;
- coordinate and covector round trips away from chart singularities;
- tetrad orthonormality, handedness, and time orientation;
- camera rays null at initialization;
- horizon, ergosurface, prograde/retrograde ISCO reference values;
- emitter four-velocity normalization;
- invariant redshift under coordinate transforms.

### Analytic and limiting cases

- Minkowski rays are straight and land at known sky coordinates;
- radial Schwarzschild null rays match analytic behavior;
- Schwarzschild critical impact parameter tends to `3 sqrt(3) M` for a distant
  observer and photon sphere radius is `3M`;
- spherical symmetry at `a=0` produces a circular critical curve independent of
  camera azimuth;
- equatorial symmetry pairs classifications and redshifts appropriately;
- weak-field deflection approaches `4M/b` in its regime of validity;
- face-on disk removes line-of-sight rotational asymmetry while retaining
  gravitational shift.

### Convergence tests

For representative regular, turning, grazing, disk-hit, captured, escaping, and
near-critical rays, halve requested error/step scales and measure:

- endpoint/event position convergence and observed order;
- event-affine-parameter convergence;
- maximum normalized `H`, `E`, `L_z`, and `Q` drift;
- outcome stability;
- redshift and spectral integral convergence;
- shadow/critical-curve location convergence by azimuth.

Near-critical rays may require a separate high-precision reference and are never
excluded merely because they dominate error.

### Differential tests

- Hamiltonian Cartesian Kerr-Schild vs Carter-separated Boyer-Lindquist;
- optional second-order Christoffel solver vs Hamiltonian solver;
- this renderer vs published analytic critical curves;
- selected rays vs AART/Krang/YNOGK outputs generated independently under their
  license and configuration terms;
- CPU `f64` vs prospective GPU modes.

Comparison occurs in invariant/physical quantities or a common regular chart,
not by naïvely subtracting singular coordinate components.

### Property and metamorphic tests

- rescaling all lengths by `M` leaves dimensionless paths unchanged;
- rotating camera and scene together about the spin axis rotates the image;
- reversing spin and azimuth maps to the expected reflected configuration;
- changing tile/thread order does not change deterministic artifacts;
- smaller accepted steps cannot introduce an unreported surface crossing;
- every ray returns exactly one outcome and all pixels are accounted for.

### Image and artifact regression

Approved low-resolution artifacts include outcome masks, sky coordinates, disk
radius/azimuth, redshift, maximum invariant drift, accepted/rejected steps, and
scene-linear radiance. Comparisons report:

- exact outcome-confusion matrix;
- maximum, percentile, RMS, and spatially located channel errors;
- structural image metrics only as secondary presentation diagnostics;
- worst pixels with their full ray reports;
- artifact and preset digests.

An image can pass presentation similarity and still fail a physical channel.

## Intended `cargo xtask evaluate` contract

```bash
cargo xtask evaluate --preset gargantua-baseline
```

The future command must:

1. resolve and schema-validate the preset, refusing unknown fields;
2. capture dirty-tree state, Rust toolchain, target, CPU/backend, and features;
3. run `cargo fmt --all -- --check`;
4. run `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
5. run unit, integration, property, analytic, limit, and convergence tests;
6. render fixed low-resolution diagnostics with deterministic scheduling;
7. verify nullness and `H/E/L_z/Q` drift per ray under calibrated budgets;
8. compare against an explicitly approved artifact set with schema compatibility;
9. write `evaluation.json`, a human-readable `evaluation.md`, EXRs, masks, and
   worst-ray traces into a content-addressed run directory;
10. classify every check and the whole gate as `PASS` or `FAIL` (no partial pass);
11. print the worst rays, pixels, invariants, termination confusion, and failure
   categories with paths to evidence;
12. exit zero only on `PASS`.

Missing references, uncalibrated required thresholds, unknown preset keys,
non-finite data, or unclassified rays are failures. Updating approved references
is a separate owner-reviewed command and never an automatic side effect.

## Other future command contracts

```bash
cargo xtask render --preset gargantua-baseline
```

Render scientific EXR/report artifacts only. Optional presentation export must be
an explicit flag and must not replace raw output.

```bash
cargo xtask inspect-ray --x <x> --y <y>
```

Reproduce a pixel/sample and emit initial tetrad/ray, accepted/rejected steps,
turning points, events considered, invariant history, termination, redshift, and
machine-readable trace. It accepts preset and sample selectors in the full CLI.

```bash
cargo xtask compare-renders <old.exr> <new.exr>
```

Check schemas/metadata first, then compare outcome masks and numeric channels,
write diff EXRs and ranked worst pixels, and use the calibration policy selected
by the artifacts. It never approves a new reference.

```bash
cargo xtask benchmark
```

Run fixed ray-class and image workloads, reporting rays/s, accepted steps/s,
event cost, memory, and percentile latency. Benchmarks never affect correctness
thresholds and record thermal/toolchain/backend context.

## Gate 0 acceptance audit

| Criterion | Evidence | Status |
|---|---|---|
| Assumptions explicit | `physics-assumptions.md` | PASS |
| Equations attributable | equations keyed to `research-sources.md` | PASS |
| Three formulations compared | architecture A–D | PASS |
| Singularities/failures documented | assumptions and event strategy | PASS |
| Credible CPU oracle | Hamiltonian KS `f64`, DOP853, independent Carter oracle | PASS |
| GPU boundary deferred/defined | architecture GPU boundary | PASS |
| Five validation modes | analytic, limit, convergence, differential, regression | PASS |
| Baseline provenance separated | annotated TOML and assumptions | PASS |
| Physics decoupled from UI/GPU | crate boundary and repository rules | PASS |
| Skeleton checks | no Rust skeleton added in Gate 0 | NOT APPLICABLE |

Gate 0 documentation passes this design audit. Scientific correctness remains to
be demonstrated by Gate 1 implementation and calibrated evidence; Gate 0 does
not claim that a renderer already exists.
