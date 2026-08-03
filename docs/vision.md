# Vision and product boundary

## Goal

Build an offline-first Rust renderer that can explain and reproduce the physical
phenomena of Kerr black-hole lensing: a shadow and critical curve, multiple
images of a celestial sphere and thin disk, frame dragging, frequency shift,
relativistic beaming, and near-critical photon trajectories.

"Gargantua" names a reproducible demonstration preset, not a claim of asset or
pixel equivalence with *Interstellar*. The published DNGR work is a scientific
and engineering reference. The film's source textures, exact shots, lens model,
compositing, and many production parameters are unavailable or proprietary.

## Two products, one explicit boundary

### Scientific render

The scientific path owns spacetime, observer state, ray initial conditions,
integration, event classification, disk intersections, redshift, invariant
radiative transfer, spectral sampling, and raw diagnostic channels. It must be
deterministic and testable without a window, UI, or GPU.

### Cinematic presentation

The presentation path may later own exposure, tone mapping, display conversion,
glare, bloom, veiling flare, motion blur, and art-directed emission. It consumes
scientific output and must never overwrite it. Every presentation control is
labeled as artistic or display-referred.

## Intended users

- researchers and students who want inspectable rays and conservation errors;
- graphics engineers comparing CPU and GPU numerical behavior;
- artists who need a physically grounded input with reversible presentation;
- reviewers who need a one-command evidence packet rather than a plausible PNG.

## Non-goals through Gate 1

- full GRMHD plasma simulation;
- proprietary DNGR ray-bundle filtering or film assets;
- real-time rendering;
- polarized transfer, scattering, or volumetric absorption;
- an egui application;
- a GPU implementation;
- claims that the baseline disk is astrophysically complete.

## Success trajectory

Gate 1 should establish a Schwarzschild/Kerr CPU ray oracle and typed diagnostic
outputs. Later gates may add thin-disk radiometry, spectral output, approved
image regressions, performance work, GPU acceleration, then presentation. Each
stage inherits the same evidence contract and cannot hide numerical failures.
