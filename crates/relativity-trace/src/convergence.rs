//! Longer Kerr convergence-order probe (Gate 1B2).
//!
//! Candidate set is declared before execution. If no candidate yields
//! `d_loose_medium > 0` with `d_medium_tight <= d_loose_medium`, the probe is
//! `Unverified` — geometry preview is not blocked.

use relativity_core::{
    initialize_rectilinear_ray, zamo_observer, CameraParams, KerrParams, PositionBl, SensorCoord,
};
use relativity_integrate::{integrate, Dop853Config, GeodesicState, IntegrationOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConvergenceProbeStatus {
    Verified,
    Unverified,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConvergenceCandidateResult {
    pub name: String,
    pub spin: f64,
    pub r_obs: f64,
    pub sensor_x: f64,
    pub affine_limit: f64,
    pub d_loose_medium: Option<f64>,
    pub d_medium_tight: Option<f64>,
    pub measurable_separation: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConvergenceProbeReport {
    pub status: ConvergenceProbeStatus,
    pub candidates: Vec<ConvergenceCandidateResult>,
}

const CANDIDATES: &[(&str, f64, f64, f64, f64)] = &[
    // name, spin, r_obs, sensor_x, affine_limit
    ("kerr_a0.5_r40_sx0.2_L5", 0.5, 40.0, 0.2, 5.0),
    ("kerr_a0.9_r30_sx0.15_L8", 0.9, 30.0, 0.15, 8.0),
    ("kerr_a0.99_r25_sx0.1_L10", 0.99, 25.0, 0.1, 10.0),
    ("sch_r50_sx0.3_L6", 0.0, 50.0, 0.3, 6.0),
];

fn endpoint_max_abs(a: &GeodesicState, b: &GeodesicState) -> f64 {
    a.to_array()
        .iter()
        .zip(b.to_array().iter())
        .map(|(u, v)| (u - v).abs())
        .fold(0.0, f64::max)
}

pub fn run_convergence_probe() -> ConvergenceProbeReport {
    let mut results = Vec::new();
    let mut any = false;
    for &(name, spin, r_obs, sx, lim) in CANDIDATES {
        let Ok(params) = KerrParams::new(1.0, spin) else {
            results.push(ConvergenceCandidateResult {
                name: name.to_string(),
                spin,
                r_obs,
                sensor_x: sx,
                affine_limit: lim,
                d_loose_medium: None,
                d_medium_tight: None,
                measurable_separation: false,
            });
            continue;
        };
        let bl = PositionBl::new(0.0, r_obs, std::f64::consts::FRAC_PI_2, 0.0);
        let Ok(obs) = zamo_observer(&params, &bl) else {
            results.push(ConvergenceCandidateResult {
                name: name.to_string(),
                spin,
                r_obs,
                sensor_x: sx,
                affine_limit: lim,
                d_loose_medium: None,
                d_medium_tight: None,
                measurable_separation: false,
            });
            continue;
        };
        let cam = CameraParams {
            horizontal_fov: 50.0_f64.to_radians(),
            roll: 0.0,
        };
        let Ok(ray) =
            initialize_rectilinear_ray(&params, &obs, &cam, SensorCoord { x: sx, y: 0.0 })
        else {
            results.push(ConvergenceCandidateResult {
                name: name.to_string(),
                spin,
                r_obs,
                sensor_x: sx,
                affine_limit: lim,
                d_loose_medium: None,
                d_medium_tight: None,
                measurable_separation: false,
            });
            continue;
        };
        let Ok(y0) = GeodesicState::new(obs.event, ray.covariant_momentum) else {
            results.push(ConvergenceCandidateResult {
                name: name.to_string(),
                spin,
                r_obs,
                sensor_x: sx,
                affine_limit: lim,
                d_loose_medium: None,
                d_medium_tight: None,
                measurable_separation: false,
            });
            continue;
        };

        let mut loose = Dop853Config::diagnostic_default();
        loose.affine_limit = lim;
        loose.relative_tolerance = [1e-6; 8];
        loose.absolute_tolerance = [1e-8; 8];
        let medium = loose.clone().with_tighter_tol(1e-2);
        let tight = medium.clone().with_tighter_tol(1e-2);

        let run = |cfg: &Dop853Config| -> Option<GeodesicState> {
            let r = integrate(params, &y0, cfg, &[]).ok()?;
            match r.outcome {
                IntegrationOutcome::AffineLimit { state, .. } => Some(state),
                _ => None,
            }
        };
        let (Some(s_l), Some(s_m), Some(s_t)) = (run(&loose), run(&medium), run(&tight)) else {
            results.push(ConvergenceCandidateResult {
                name: name.to_string(),
                spin,
                r_obs,
                sensor_x: sx,
                affine_limit: lim,
                d_loose_medium: None,
                d_medium_tight: None,
                measurable_separation: false,
            });
            continue;
        };
        let d_lm = endpoint_max_abs(&s_l, &s_m);
        let d_mt = endpoint_max_abs(&s_m, &s_t);
        let ok = d_lm > 0.0 && d_mt <= d_lm + 1e-15;
        if ok {
            any = true;
        }
        results.push(ConvergenceCandidateResult {
            name: name.to_string(),
            spin,
            r_obs,
            sensor_x: sx,
            affine_limit: lim,
            d_loose_medium: Some(d_lm),
            d_medium_tight: Some(d_mt),
            measurable_separation: ok,
        });
    }
    ConvergenceProbeReport {
        status: if any {
            ConvergenceProbeStatus::Verified
        } else {
            ConvergenceProbeStatus::Unverified
        },
        candidates: results,
    }
}
