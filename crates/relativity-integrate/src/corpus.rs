//! Deterministic Gate 1B1 validation corpus.

use relativity_core::{
    initialize_rectilinear_ray, zamo_observer, CameraParams, Covector, KerrParams, PositionBl,
    PositionKs, SensorCoord,
};

use crate::adapter::integrate;
use crate::config::Dop853Config;
use crate::error::IntegrationError;
use crate::event::{EscapeSphere, EventId, EventSurface, OuterHorizon};
use crate::outcome::{IntegrationOutcome, IntegrationReport};
use crate::state::GeodesicState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorClass {
    PhysicsDomain,
    NonFiniteState,
    Solver,
    EventDomain,
    StepLimitExceeded,
    InvalidConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExpectedOutcome {
    Event(EventId),
    AffineLimit,
    Error(ErrorClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusId {
    MinkowskiStraightNull,
    MinkowskiEscapeSphere,
    SchwarzschildWeakOutgoing,
    SchwarzschildInwardHorizon,
    KerrWeakEquatorial,
    KerrProgradeEquatorial,
    KerrRetrogradeEquatorial,
    KerrNearAxis,
    KerrNearExtremalExterior,
    InvalidDomain,
}

impl CorpusId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MinkowskiStraightNull => "minkowski_straight_null",
            Self::MinkowskiEscapeSphere => "minkowski_escape_sphere",
            Self::SchwarzschildWeakOutgoing => "schwarzschild_weak_outgoing",
            Self::SchwarzschildInwardHorizon => "schwarzschild_inward_horizon",
            Self::KerrWeakEquatorial => "kerr_weak_equatorial",
            Self::KerrProgradeEquatorial => "kerr_prograde_equatorial",
            Self::KerrRetrogradeEquatorial => "kerr_retrograde_equatorial",
            Self::KerrNearAxis => "kerr_near_axis",
            Self::KerrNearExtremalExterior => "kerr_near_extremal_exterior",
            Self::InvalidDomain => "invalid_domain",
        }
    }
}

pub struct CorpusCase {
    pub id: CorpusId,
    pub expected: ExpectedOutcome,
}

pub const CORPUS: &[CorpusCase] = &[
    CorpusCase {
        id: CorpusId::MinkowskiStraightNull,
        expected: ExpectedOutcome::AffineLimit,
    },
    CorpusCase {
        id: CorpusId::MinkowskiEscapeSphere,
        expected: ExpectedOutcome::Event(EventId::EscapeSphere),
    },
    CorpusCase {
        id: CorpusId::SchwarzschildWeakOutgoing,
        expected: ExpectedOutcome::AffineLimit,
    },
    CorpusCase {
        id: CorpusId::SchwarzschildInwardHorizon,
        expected: ExpectedOutcome::Event(EventId::OuterHorizon),
    },
    CorpusCase {
        id: CorpusId::KerrWeakEquatorial,
        expected: ExpectedOutcome::AffineLimit,
    },
    CorpusCase {
        id: CorpusId::KerrProgradeEquatorial,
        expected: ExpectedOutcome::AffineLimit,
    },
    CorpusCase {
        id: CorpusId::KerrRetrogradeEquatorial,
        expected: ExpectedOutcome::AffineLimit,
    },
    CorpusCase {
        id: CorpusId::KerrNearAxis,
        expected: ExpectedOutcome::AffineLimit,
    },
    CorpusCase {
        id: CorpusId::KerrNearExtremalExterior,
        expected: ExpectedOutcome::AffineLimit,
    },
    CorpusCase {
        id: CorpusId::InvalidDomain,
        expected: ExpectedOutcome::Error(ErrorClass::PhysicsDomain),
    },
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeterminismRecord {
    pub case: String,
    pub outcome_variant: String,
    pub event_id: Option<EventId>,
    pub lambda_bits: Option<u64>,
    pub state_bits: Option<String>,
    pub raw_stop_lambda_bits: Option<u64>,
    pub raw_stop_state_bits: Option<String>,
    pub accepted_steps: u64,
    pub rejected_steps: u64,
    pub rhs_evaluations: u64,
    pub h_initial_bits: u64,
    pub h_final_bits: u64,
    pub p_t_max_drift_bits: u64,
    pub error_class: Option<ErrorClass>,
}

fn minkowski_params() -> KerrParams {
    // Quasi-Minkowski: M > 0 required by KerrParams; M ≪ length scales.
    KerrParams::new(1.0e-18, 0.0).expect("minkowski params")
}

fn minkowski_straight_state() -> GeodesicState {
    // Analytic null line in η: x(λ)=x0+λ k with k^μ = (−1, 1, 0, 0), p_μ = (1, 1, 0, 0).
    GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .expect("minkowski state")
}

fn camera_default() -> CameraParams {
    CameraParams {
        horizontal_fov: 50.0_f64.to_radians(),
        roll: 0.0,
    }
}

fn ray_state(
    params: &KerrParams,
    r: f64,
    theta: f64,
    sensor: SensorCoord,
) -> Result<(GeodesicState, f64), IntegrationError> {
    let bl = PositionBl::new(0.0, r, theta, 0.0);
    let obs = zamo_observer(params, &bl).map_err(IntegrationError::from_core)?;
    let ray = initialize_rectilinear_ray(params, &obs, &camera_default(), sensor)
        .map_err(IntegrationError::from_core)?;
    let state = GeodesicState::new(obs.event, ray.covariant_momentum)?;
    Ok((state, r))
}

fn classify_error(err: &IntegrationError) -> ErrorClass {
    match err {
        IntegrationError::PhysicsDomain { .. } => ErrorClass::PhysicsDomain,
        IntegrationError::NonFiniteState { .. } => ErrorClass::NonFiniteState,
        IntegrationError::Solver { .. } => ErrorClass::Solver,
        IntegrationError::EventDomain { .. } => ErrorClass::EventDomain,
        IntegrationError::StepLimitExceeded { .. } => ErrorClass::StepLimitExceeded,
        IntegrationError::InvalidConfig { .. } => ErrorClass::InvalidConfig,
        IntegrationError::MissingEventOutcome | IntegrationError::InvalidInterpolantBounds => {
            ErrorClass::Solver
        }
    }
}

fn matches_expected(report: &IntegrationReport, expected: &ExpectedOutcome) -> bool {
    match (expected, &report.outcome) {
        (ExpectedOutcome::AffineLimit, IntegrationOutcome::AffineLimit { .. }) => true,
        (ExpectedOutcome::Event(id), IntegrationOutcome::Event(hit)) => hit.event_id == *id,
        _ => false,
    }
}

/// Run one corpus case; returns report on success path or typed error.
pub fn run_corpus_case(
    case: &CorpusCase,
) -> Result<IntegrationReport, (ExpectedOutcome, IntegrationError)> {
    let mut cfg = Dop853Config::diagnostic_default();
    match case.id {
        CorpusId::MinkowskiStraightNull => {
            let params = minkowski_params();
            let y0 = minkowski_straight_state();
            cfg.affine_limit = 5.0;
            cfg.max_step = 0.5;
            integrate(params, &y0, &cfg, &[]).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::MinkowskiEscapeSphere => {
            let params = minkowski_params();
            let y0 = minkowski_straight_state();
            let r0 = 10.0;
            let r_escape = 20.0;
            assert!(r_escape > r0);
            cfg.affine_limit = 50.0;
            cfg.max_step = 0.5;
            let esc = EscapeSphere::new(params, r_escape).expect("escape");
            let surfaces: [&dyn EventSurface; 1] = [&esc];
            integrate(params, &y0, &cfg, &surfaces).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::SchwarzschildWeakOutgoing => {
            let params = KerrParams::new(1.0, 0.0).expect("sch");
            let (y0, _) = ray_state(
                &params,
                200.0,
                std::f64::consts::FRAC_PI_2,
                SensorCoord { x: 0.0, y: 0.0 },
            )
            .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 1.0;
            cfg.max_step = 0.1;
            integrate(params, &y0, &cfg, &[]).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::SchwarzschildInwardHorizon => {
            let params = KerrParams::new(1.0, 0.0).expect("sch");
            // Center sensor looks toward BH (−e₃ in camera).
            let (y0, r0) = ray_state(
                &params,
                20.0,
                std::f64::consts::FRAC_PI_2,
                SensorCoord { x: 0.0, y: 0.0 },
            )
            .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 200.0;
            cfg.max_step = 0.5;
            // Horizon approach collapses adaptive h in f64 KS; value tol must
            // admit endpoint/stall capture of f = r − r₊ (still the physical surface).
            cfg.event_value_tolerance = 1e-10;
            let hor = OuterHorizon::new(params);
            let esc = EscapeSphere::new(params, (r0 * 5.0).max(100.0)).expect("escape");
            let surfaces: [&dyn EventSurface; 2] = [&hor, &esc];
            integrate(params, &y0, &cfg, &surfaces).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::KerrWeakEquatorial => {
            let params = KerrParams::new(1.0, 0.5).expect("kerr");
            let (y0, _) = ray_state(
                &params,
                200.0,
                std::f64::consts::FRAC_PI_2,
                SensorCoord { x: 0.05, y: 0.0 },
            )
            .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 0.5;
            cfg.max_step = 0.05;
            integrate(params, &y0, &cfg, &[]).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::KerrProgradeEquatorial => {
            let params = KerrParams::new(1.0, 0.9).expect("kerr");
            let (y0, _) = ray_state(
                &params,
                50.0,
                std::f64::consts::FRAC_PI_2,
                SensorCoord { x: 0.2, y: 0.0 },
            )
            .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 0.5;
            cfg.max_step = 0.05;
            integrate(params, &y0, &cfg, &[]).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::KerrRetrogradeEquatorial => {
            let params = KerrParams::new(1.0, 0.9).expect("kerr");
            let (y0, _) = ray_state(
                &params,
                50.0,
                std::f64::consts::FRAC_PI_2,
                SensorCoord { x: -0.2, y: 0.0 },
            )
            .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 0.5;
            cfg.max_step = 0.05;
            integrate(params, &y0, &cfg, &[]).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::KerrNearAxis => {
            let params = KerrParams::new(1.0, 0.9).expect("kerr");
            let (y0, _) = ray_state(&params, 40.0, 0.15, SensorCoord { x: 0.0, y: 0.0 })
                .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 0.5;
            cfg.max_step = 0.05;
            integrate(params, &y0, &cfg, &[]).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::KerrNearExtremalExterior => {
            // a/M < 1 — not exactly extremal.
            let params = KerrParams::new(1.0, 0.999).expect("kerr");
            let (y0, _) = ray_state(
                &params,
                30.0,
                85.0_f64.to_radians(),
                SensorCoord { x: 0.0, y: 0.0 },
            )
            .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 0.25;
            cfg.max_step = 0.02;
            integrate(params, &y0, &cfg, &[]).map_err(|e| (case.expected.clone(), e))
        }
        CorpusId::InvalidDomain => {
            let params = KerrParams::new(1.0, 0.9).expect("kerr");
            // Ring-singularity neighborhood: RHS must latch PhysicsDomain.
            let y0 = GeodesicState::new(
                PositionKs::new(0.0, 0.0, 0.0, 0.0),
                Covector::new(1.0, 0.0, 0.0, 0.0),
            )
            .expect("state finite");
            cfg.affine_limit = 1.0;
            cfg.max_step = 0.1;
            match integrate(params, &y0, &cfg, &[]) {
                Ok(r) => Ok(r),
                Err(e) => Err((case.expected.clone(), e)),
            }
        }
    }
}

/// Verify expected outcome; expected errors yield `Ok(None)`.
pub fn run_and_check(case: &CorpusCase) -> Result<Option<IntegrationReport>, String> {
    match (&case.expected, run_corpus_case(case)) {
        (ExpectedOutcome::Error(class), Err((_, err))) => {
            let got = classify_error(&err);
            if got == *class {
                Ok(None)
            } else {
                Err(format!(
                    "{}: expected {:?}, got {:?} ({err})",
                    case.id.as_str(),
                    class,
                    got
                ))
            }
        }
        (ExpectedOutcome::Error(class), Ok(_)) => Err(format!(
            "{}: expected error {:?}, got success",
            case.id.as_str(),
            class
        )),
        (expected, Ok(report)) => {
            if !matches_expected(&report, expected) {
                return Err(format!(
                    "{}: expected {:?}, got {}",
                    case.id.as_str(),
                    expected,
                    report.outcome.variant_name()
                ));
            }
            if let IntegrationOutcome::Event(hit) = &report.outcome {
                if !hit.lambda.0.is_finite() || hit.state.to_array().iter().any(|v| !v.is_finite())
                {
                    return Err(format!("{}: non-finite event success", case.id.as_str()));
                }
            }
            Ok(Some(report))
        }
        (_, Err((_, err))) => Err(format!("{}: unexpected error {err}", case.id.as_str())),
    }
}

pub fn determinism_record(
    case: &CorpusCase,
    report: Option<&IntegrationReport>,
) -> DeterminismRecord {
    match report {
        None => DeterminismRecord {
            case: case.id.as_str().into(),
            outcome_variant: "Error".into(),
            event_id: None,
            lambda_bits: None,
            state_bits: None,
            raw_stop_lambda_bits: None,
            raw_stop_state_bits: None,
            accepted_steps: 0,
            rejected_steps: 0,
            rhs_evaluations: 0,
            h_initial_bits: 0,
            h_final_bits: 0,
            p_t_max_drift_bits: 0,
            error_class: match &case.expected {
                ExpectedOutcome::Error(c) => Some(*c),
                _ => None,
            },
        },
        Some(r) => {
            let (event_id, lambda_bits, state_bits, raw_l, raw_s) = match &r.outcome {
                IntegrationOutcome::Event(hit) => (
                    Some(hit.event_id),
                    Some(hit.lambda.0.to_bits()),
                    Some(hit.state.bits_hex()),
                    Some(hit.raw_solver_stop.lambda.0.to_bits()),
                    Some(hit.raw_solver_stop.state.bits_hex()),
                ),
                IntegrationOutcome::AffineLimit { lambda, state, .. } => (
                    None,
                    Some(lambda.0.to_bits()),
                    Some(state.bits_hex()),
                    None,
                    None,
                ),
            };
            let st = r.outcome.stats();
            DeterminismRecord {
                case: case.id.as_str().into(),
                outcome_variant: r.outcome.variant_name().into(),
                event_id,
                lambda_bits,
                state_bits,
                raw_stop_lambda_bits: raw_l,
                raw_stop_state_bits: raw_s,
                accepted_steps: st.accepted_steps,
                rejected_steps: st.rejected_steps,
                rhs_evaluations: st.rhs_evaluations,
                h_initial_bits: r.diagnostics.h_initial.to_bits(),
                h_final_bits: r.diagnostics.h_final.to_bits(),
                p_t_max_drift_bits: r.diagnostics.p_t_max_abs_drift.to_bits(),
                error_class: None,
            }
        }
    }
}
