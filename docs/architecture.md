# Gate 0 architecture

## Decision summary

| Concern | Selected decision | Why |
|---|---|---|
| Primary geodesics | Canonical 8D Hamilton equations | First order, coordinate-generic, null Hamiltonian is a direct diagnostic |
| Coordinates | Cartesian Kerr-Schild (ingoing; via spherical KS `(T,r,θ,ψ)`) | Horizon-penetrating and axis-regular; demonstrated by GRay2 and Skylight; BL↔KS uses `dT/dt,dψ/dφ` radial terms |
| CPU integration | Adaptive DOP853 in Rust `f64` | High-order non-stiff oracle with embedded error estimate and dense output |
| Events | Bracket plus dense-output root localization; subdivide ambiguous steps | Prevents endpoint-only disk/horizon misses and produces ordered events |
| Camera | Local orthonormal tetrad attached to an explicit observer four-velocity | Separates local optics from coordinates and supports moving observers |
| Termination | Closed typed taxonomy | Numerical failures cannot masquerade as physical surfaces |
| Disk | Surface geometry + velocity field + emission law | Lets physics tests isolate intersection, motion, and radiometry |
| Radiometry | Spectral `f64` transport; scene-linear float EXR export | Preserves redshift semantics and HDR diagnostic range |
| CPU renderer | Deterministic, headless, tile-parallel oracle | Reviewable baseline independent of UI and GPU |
| GPU | Deferred backend consuming a frozen core contract | Precision/capability must be measured, especially near critical curves |
| Artifacts | EXR + canonical JSON report + TOML preset | Pixel data, typed channels, provenance, and human-reviewable configuration |
| Validation | Analytic + limits + convergence + differential + image regression | No single oracle can validate all strong-field behavior |

ADRs in [`adr/`](adr) make the consequential decisions independently reviewable.

## Formulation comparison

### A. Second-order geodesic equation

`d2 x^mu/dlambda2 + Gamma^mu_(alpha beta) dx^alpha/dlambda dx^beta/dlambda = 0`.
It is general and familiar, and packages such as GYOTO and RAPTOR demonstrate
generic-metric designs. It requires position and velocity state plus Christoffel
symbols, makes the null constraint secondary, and can accumulate velocity drift.
It is retained as a potential test implementation, not selected as primary.

### B. Canonical Hamilton equations — selected

`H = 1/2 g^(mu nu) p_mu p_nu` yields eight first-order equations. It is generic
over coordinate charts, exposes a scalar null constraint, and naturally retains
covariant momentum for `-k.u` radiometry. Metric inverse derivatives are the main
cost. Gate 1 should implement analytic Kerr-Schild expressions and verify them
against finite differences or dual numbers in tests; automatic differentiation
must not enter the hot path until benchmarked.

### C. Carter-separated first-order equations

Kerr's `R(r)` and `Theta(theta)` potentials reduce the problem using `E`, `L_z`,
and `Q`. They are fast and excellent for analytic comparisons. Turning-point sign
bookkeeping, coordinate singularities, and Kerr specificity make them a poor
generic production abstraction. Selected as an independent differential oracle.

### D. Analytic elliptic-function mapping

YNOGK, AART, and Krang show that Kerr rays can be evaluated through special
functions with excellent photon-ring performance. This is a valuable later
oracle/accelerator, but it increases branch complexity and cannot extend to an
arbitrary metric. Rejected for Gate 1 primary implementation.

## Coordinate comparison

| Chart | Advantages | Failure modes | Role |
|---|---|---|---|
| Boyer-Lindquist | Separability, literature formulas, intuitive `r/theta/phi` | Horizon `Delta=0`, polar axis, near-extremal cancellation | Input/reporting and independent oracle |
| Spherical Kerr-Schild | Horizon penetrating; common in GRMHD | Still has spherical polar axis behavior | Interchange with simulation data later |
| Cartesian Kerr-Schild | Horizon and axis regular; GPU precedent | Implicit oblate radius and more algebra | Primary CPU state |

## Proposed crate boundaries

No crates are created in Gate 0 because a dependency graph can be reviewed
without committing premature APIs. Gate 1 should start with:

```text
relativity-core       metric, coordinates, tetrads, rays, invariants (no I/O)
relativity-integrate  DOP853, dense output, event arbitration
relativity-scene      disk/sky geometry, velocities, emission interfaces
relativity-render     deterministic CPU scheduling and spectral accumulation
relativity-artifact   EXR and canonical report serialization
xtask                 orchestration only
```

Dependency direction is downward in that list except `xtask`, which may call
public application crates. `relativity-core` cannot depend on egui, wgpu, EXR,
TOML, filesystem, or a global logger. A future GUI and GPU backend are peers of
the headless frontend, never owners of physics.

## Core contracts proposed for Gate 1

- `Metric`: inverse metric and spatial derivatives at a coordinate event.
- `CoordinateMap`: checked position/covector transformations and chart domain.
- `Observer`: event, normalized four-velocity, and orthonormal tetrad.
- `RayState`: coordinates, covariant momentum, affine parameter, and counters.
- `Integrator`: advances a state and returns accepted-step dense output.
- `EventSurface`: continuous event function plus direction and priority policy.
- `RayOutcome`: one physical termination or one explicit numerical failure.
- `Emitter`: local velocity and spectral specific intensity; no geometry logic.

Traits should not force dynamic dispatch in hot loops; generic/static dispatch is
the default, with object-safe adapters only at configuration boundaries.

## Integration and event strategy

DOP853 [Hairer1993] is selected for the CPU oracle. Its absolute/relative error norm must use
component scales appropriate to positions and momenta, not one unexamined scalar
tolerance. A curvature/metric-domain guard limits steps in addition to the
embedded estimate. Step rejection and minimum-step exhaustion are recorded.

After every accepted step:

1. test horizon, disk, and celestial-boundary continuous event functions;
2. inspect dense output for brackets and possible internal extrema;
3. if multiple/tangential events cannot be ordered, recursively subdivide;
4. localize each bracket with a safeguarded root solver on dense output;
5. choose the earliest event in backward affine parameter;
6. re-evaluate the state and invariants at the localized root;
7. emit a typed outcome or `AmbiguousEvent`, never precedence by code order.

The horizon capture surface is just inside `r_+` by a calibrated numerical
margin in the horizon-penetrating chart. The margin must converge toward the
horizon and must not change the shadow beyond the approved error budget. Disk
contact uses equatorial-plane crossing plus radial bounds. Tangencies require a
near-zero/extremum check, not only a sign change. The sky event is outward
crossing of a finite validation sphere.

## Ray termination taxonomy

Physical outcomes:

- `HorizonCaptured { event }`
- `DiskHit { event, radius, side }`
- `CelestialSphere { event, direction }`

Controlled resource outcomes:

- `AffineLimit { last_state }`
- `StepLimit { last_state }`

Numerical failures:

- `StepUnderflow`
- `NonFiniteState`
- `MetricDomainError`
- `ConstraintViolation`
- `AmbiguousEvent`
- `IntegratorFailure`

Reports retain the last valid state, worst normalized local error, maximum drift
for `H/E/L_z/Q`, accepted/rejected steps, closest horizon approach, turning-point
counts, and event-localization residual.

## Camera model

Presets specify an observer event and motion model (`static`, `zamo`, circular
geodesic, or explicit tetrad in later gates). Construction occurs in a stable
local frame, with modified Gram-Schmidt or a closed-form tetrad followed by
metric-aware orthonormality checks. Projection (rectilinear initially), sensor
mapping, and observer motion are separate. The camera produces local null
directions; it does not know about Kerr coordinates.

## Disk and sky abstractions

The Gate 1 disk is an equatorial surface. Inner edge policies include explicit
radius or computed prograde/retrograde ISCO. Its material can use circular
geodesic motion only where timelike circular orbits exist. Plunging-region and
finite-thickness models require later ADRs.

The celestial sphere is a finite event surface with a procedural, seam-defined
UV diagnostic texture. Scientific regressions use IDs/coordinates rather than a
copyrighted star catalog. A real catalog is a later data/license decision.

## HDR, spectrum, and deterministic artifacts

The physical API carries spectral specific intensity in `f64` samples at an
explicit wavelength/frequency grid. Sampling density is a convergence parameter,
not a physical constant. Diagnostic outputs include redshift `g`, bolometric
factor, outcome ID, affine length, step counts, and invariant drift even before
full disk spectra exist.

The archival render is a multi-channel OpenEXR with `FLOAT` scientific channels.
**Gate 2C1 scientific RGB authority** is scene-linear Rec.709 / D65 (linear, no
OETF); ACEScg / AP1 remains an aspirational presentation path for a later gate
and is **not** used in V1 (avoids D60 CAT). Display-referred PNG is deferred and
non-authoritative. A canonical JSON sidecar
uses sorted keys, decimal finite numbers only, stable enum strings, schema
version, preset SHA-256, executable/toolchain identity, target triple, backend,
thread count, and per-file SHA-256. Non-finite values are encoded as typed status,
not JSON `NaN`.

Determinism means identical classifications and bitwise artifacts on the pinned
CPU/toolchain/backend. Cross-platform comparisons use numerical metrics and
classification masks because libm and instruction contraction may differ. Tile
assembly order is fixed; stochastic sampling uses a counter-based pixel/sample
key and never scheduling order.

## GPU boundary

The prospective GPU backend consumes packed ray initial states and immutable
scene parameters and returns the same outcome/diagnostic schema. It does not own
camera semantics, preset parsing, artifact policy, or acceptance thresholds.

Portable WGSL assumes `f32`; wgpu's `SHADER_F64` is native Vulkan-only and may be
much slower. Therefore Gate 0 selects neither WGSL nor a particular GPU API as a
scientific oracle. A later gate must probe adapter capabilities and compare
`f32`, mixed precision, and any native `f64` path ray-by-ray against CPU `f64`,
with special sampling around the critical curve. Pixels outside the proven
envelope may fall back to CPU or be marked unresolved.

## Rejected coupling

- egui-owned scene or physics state;
- shader code as the only equation implementation;
- PNG as the authoritative regression artifact;
- average image similarity as the only correctness measure;
- event detection based only on step endpoints;
- silently painting failed rays black;
- hard-coded *Interstellar* grading inside radiative transfer.
