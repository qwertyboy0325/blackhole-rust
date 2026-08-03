# Research sources and licensing record

Sources are grouped by the decision they support. No external source code,
paper, image, or dataset is vendored in Gate 0. "Access only" means the project
links and cites the work under the publisher/archive's terms; it does not assert
a reuse license. Access checked 2026-08-03.

## Foundations and Kerr geometry

- **[Kerr1963]** R. P. Kerr, “Gravitational Field of a Spinning Mass as an
  Example of Algebraically Special Metrics,” *Physical Review Letters* 11, 237.
  [DOI](https://doi.org/10.1103/PhysRevLett.11.237). Primary Kerr solution;
  publisher copyright, access only.
- **[Carter1968]** B. Carter, “Global Structure of the Kerr Family of
  Gravitational Fields,” *Physical Review* 174, 1559.
  [DOI](https://doi.org/10.1103/PhysRev.174.1559). Hamilton-Jacobi separation,
  fourth constant, horizons; publisher copyright, access only.
- **[BoyerLindquist1967]** R. H. Boyer and R. W. Lindquist, “Maximal Analytic
  Extension of the Kerr Metric,” *Journal of Mathematical Physics* 8, 265.
  [DOI](https://doi.org/10.1063/1.1705193). Coordinate chart; publisher
  copyright, access only.
- **[BPT1972]** J. M. Bardeen, W. H. Press, and S. A. Teukolsky, “Rotating Black
  Holes: Locally Nonrotating Frames, Energy Extraction, and Scalar Synchrotron
  Radiation,” *Astrophysical Journal* 178, 347.
  [NASA/OSTI record](https://www.osti.gov/biblio/4585183). ZAMO/LNRF, circular
  photon orbits, marginally stable orbits; publisher copyright, access only.
- **[Bardeen1973]** J. M. Bardeen, “Timelike and Null Geodesics in the Kerr
  Metric,” in *Black Holes (Les Astres Occlus)*, pp. 215–239.
  [ADS record](https://ui.adsabs.harvard.edu/abs/1973blho.conf..215B/abstract).
  Kerr shadow/critical-curve analysis; access only.
- **[MTW1973]** C. W. Misner, K. S. Thorne, and J. A. Wheeler, *Gravitation*,
  chapter 33. Hamiltonian geodesic formulation; book copyright, bibliographic
  citation only.

## Numerical ray tracing and transfer

- **[DormandPrince1980]** J. R. Dormand and P. J. Prince, “A Family of Embedded
  Runge-Kutta Formulae,” *Journal of Computational and Applied Mathematics* 6,
  19–26. [DOI](https://doi.org/10.1016/0771-050X(80)90013-3). Embedded adaptive
  Runge-Kutta foundation; publisher copyright, access only.
- **[Hairer1993]** E. Hairer, S. P. Nørsett, and G. Wanner, *Solving Ordinary
  Differential Equations I: Nonstiff Problems*, 2nd ed., Springer, sections II.5
  and II.6. DOP853 8(5,3), seventh-order dense output, and event-location
  foundation. [Author's official software page](https://www.unige.ch/~hairer/software.html).
  Book copyright, bibliographic access only; no Fortran source reused.
- **[GRay2]** C. Chan et al., “GRay2: A General Purpose Geodesic Integrator for
  Kerr Spacetimes,” *Astrophysical Journal* 867, 59.
  [arXiv](https://arxiv.org/abs/1706.07062). Cartesian Kerr-Schild GPU/CPU
  integration and convergence evidence; arXiv access only, no code reused.
- **[Skylight2022]** O. Reula, F. Carrasco, and C. Bederian, “Skylight: a new code for
  general-relativistic ray-tracing and radiative transfer in arbitrary
  space-times,” *MNRAS* 515, 1316.
  [DOI](https://doi.org/10.1093/mnras/stac1857). Cartesian Kerr-Schild regularity
  and verification cases; publisher copyright, access only.
- **[Younsi2012]** Z. Younsi, K. Wu, and S. V. Fuerst, “General relativistic
  radiative transfer: formulation and emission from structured tori around black
  holes,” *Astronomy & Astrophysics* 545, A13.
  [arXiv](https://arxiv.org/abs/1207.4234). Invariant transfer quantities;
  arXiv access only.
- **[RAPTOR2018]** T. Bronzwaer et al., “RAPTOR I: Time-dependent radiative
  transfer in arbitrary spacetimes,” *Astronomy & Astrophysics* 613, A2.
  [DOI](https://doi.org/10.1051/0004-6361/201732149). Coordinate-independent
  transfer architecture and BL/Kerr-Schild comparison; article access only.
- **[Gralla2020]** S. E. Gralla and A. Lupsasca, “Null geodesics of the Kerr
  exterior,” *Physical Review D* 101, 044032.
  [arXiv](https://arxiv.org/abs/1910.12881). Analytic null-geodesic and photon
  shell reference; arXiv access only.
- **[YNOGK2013]** X. Yang and J. Wang, “YNOGK: A new public code for calculating
  null geodesics in the Kerr spacetime,” *ApJS* 207, 6.
  [arXiv](https://arxiv.org/abs/1305.1250). Weierstrass/Jacobi analytic
  alternative; paper access only, code license must be checked before reuse.

## DNGR and visual precedent

- **[James2015]** O. James, E. von Tunzelmann, P. Franklin, and K. S. Thorne,
  “Gravitational Lensing by Spinning Black Holes in Astrophysics, and in the
  Movie Interstellar,” *Classical and Quantum Gravity* 32, 065001.
  [arXiv](https://arxiv.org/abs/1502.03808),
  [DOI](https://doi.org/10.1088/0264-9381/32/6/065001). Ray bundles, camera
  tetrads, filtering, published spin examples, and explicit film art direction;
  arXiv version licensed CC BY-NC-SA 3.0; no code or assets available/reused.
- **[Luminet1979]** J.-P. Luminet, “Image of a Spherical Black Hole with Thin
  Accretion Disk,” *Astronomy & Astrophysics* 75, 228–235.
  [ADS](https://ui.adsabs.harvard.edu/abs/1979A%26A....75..228L/abstract).
  Thin-disk lensing and redshift precedent; publisher copyright, access only.

## Existing software reviewed

Review is architectural. Gate 0 copies no implementation.

- **GYOTO** — modular arbitrary-metric ray tracing and radiative transfer.
  [Project](https://gyoto.obspm.fr/),
  [paper](https://arxiv.org/abs/1109.4769). GPL-3.0-or-later.
- **RAPTOR** — arbitrary-coordinate GR radiative transfer with Kerr-Schild
  support. [Repository](https://github.com/tbronzwaer/raptor). GPL-3.0.
- **Odyssey** — CUDA GPU Kerr GRRT with adaptive RK5.
  [Repository](https://github.com/hungyipu/Odyssey). GPL-3.0.
- **EinsteinPy Geodesics** — Hamiltonian Kerr/Schwarzschild reference using the
  Julia differential-equation ecosystem.
  [Documentation](https://docs.geodesics.einsteinpy.org/). MIT.
- **AART** — analytic Kerr photon-ring ray tracing and adaptive image grids.
  [Repository](https://github.com/iAART/aart). MIT.
- **Krang** — Julia analytic Kerr null-geodesic ray tracer.
  [JOSS paper](https://doi.org/10.21105/joss.07273). Paper CC BY 4.0; repository
  license must be verified at the exact revision before any code reuse.

GPL implementations are scientific references only unless the owner later
chooses a compatible project license and explicitly approves reuse. Equations
must be implemented from attributable literature, with derivation notes and
tests, rather than transcribed from an incompatibly licensed repository.

## Gate 1A geometry and numerics

- **[KerrSchild1965]** R. P. Kerr and A. Schild, “Some algebraically degenerate
  solutions of Einstein’s gravitational field equations,” *Proc. Symp. Appl.
  Math.* 17, 199 (1965). Kerr–Schild ansatz; bibliographic/access only.
- **Cartesian Kerr–Schild working form** as used by GRay2 / common numerical
  practice: `g = η + 2H ℓ⊗ℓ` with
  `H = M r³/(r⁴ + a² z²)` and
  `ℓ_μ = (1, (rx+ay)/(r²+a²), (ry−ax)/(r²+a²), z/r)`.
  Cross-check: GRay2 arXiv:1706.07062; Wikipedia Kerr metric KS section
  (secondary). No third-party code reused.
- **Stable oblate-radius rationalization** `r² = 2 a² z² / (D − A)` when `A < 0`
  follows from multiplying `(A+D)/2` by `(D−A)/(D−A)`. Project derivation; see
  `crates/relativity-core/src/radius.rs` and work-log conditioning notes.
- **Ingoing Kerr–Schild / BL differentials** used in Gate 1A remediation:
  `dT = dt + 2Mr/Δ dr`, `dψ = dφ + a/Δ dr`, and
  `x+iy = (r+ia)e^{iψ}sinθ`, chosen to match the project `ℓ_μ` signs.
  Documented in `crates/relativity-core/src/coords/` and
  `docs/physics-assumptions.md`. Access/derivation notes 2026-08-03; no
  third-party code reused.
- **ODE crate survey + Gate 1B0 spike** (pinned `ode_solvers =0.6.1`, `ivp =0.6.0`):
  Apache-2.0; repos [srenevey/ode-solvers](https://github.com/srenevey/ode-solvers),
  [Ryan-D-Gast/ivp](https://github.com/Ryan-D-Gast/ivp). Survey:
  `docs/research/dop853-rust-dependency-audit.md`. Executable spike:
  `docs/research/gate-1b0-dop853-spike-report.md`. ADR 0005 remains Proposed.
  No production ODE dependency in tree. Access checked 2026-08-03.

## Formats and GPU capability

- **OpenEXR technical introduction.** Multi-channel `HALF`, `FLOAT`, and `UINT`
  storage and metadata. [Official documentation](https://openexr.com/en/latest/TechnicalIntroduction.html).
  OpenEXR software is BSD-3-Clause; specification/documentation terms are linked
  by the project. No library is vendored in Gate 0.
- **WGSL specification.** Core shader arithmetic is centered on `f32`; `f16` is
  an optional extension. [W3C WGSL](https://www.w3.org/TR/WGSL/). W3C document
  license.
- **wgpu `SHADER_F64`.** Native-only Vulkan feature with substantial possible
  throughput cost; it is not a portable WGSL/WebGPU baseline.
  [wgpu API documentation](https://docs.rs/wgpu/latest/wgpu/struct.Features.html#associatedconstant.SHADER_F64).
  wgpu is MIT OR Apache-2.0; documentation under crate terms.
