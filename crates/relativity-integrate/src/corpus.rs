//! Deterministic Gate 1B1 validation corpus.

use relativity_core::{
    initialize_rectilinear_ray, zamo_observer, CameraParams, Covector, KerrParams, PositionBl,
    PositionKs, SensorCoord,
};
use serde::Serialize;

use crate::adapter::integrate;
use crate::config::{Dop853Config, HorizonProximityPolicy};
use crate::error::IntegrationError;
use crate::event::{EscapeSphere, EventId, EventSurface, OuterHorizon};
use crate::outcome::{IntegrationOutcome, IntegrationReport, SurfaceApproachReason};
use crate::state::GeodesicState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorClass {
    PhysicsDomain,
    NonFiniteState,
    Solver,
    EventDomain,
    StepLimitExceeded,
    InvalidConfig,
    EventLocalizationDidNotConverge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExpectedOutcome {
    Event(EventId),
    /// SurfaceApproach for OuterHorizon with the documented stall reason.
    SurfaceApproach {
        event_id: EventId,
        reason: SurfaceApproachReason,
    },
    AffineLimit,
    Error(ErrorClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        // Demonstrated: f64 KS adaptive stall near r₊⁺ with opt-in proximity —
        // not an exact OuterHorizon EventHit.
        expected: ExpectedOutcome::SurfaceApproach {
            event_id: EventId::OuterHorizon,
            reason: SurfaceApproachReason::SolverStepSizeTooSmall,
        },
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

/// Canonical per-case numerical record for cross-process determinism.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct CanonicalCaseRecord {
    pub case: String,
    pub expected_outcome: String,
    pub actual_outcome: String,
    pub termination_id: Option<EventId>,
    pub lambda_bits: Option<u64>,
    pub state_bits: Option<String>,
    pub raw_stop_lambda_bits: Option<u64>,
    pub raw_stop_state_bits: Option<String>,
    pub accepted_steps: u64,
    pub rejected_steps: u64,
    pub rhs_evaluations: u64,
    pub callback_count: u64,
    pub h_initial_bits: u64,
    pub h_final_bits: u64,
    pub h_max_residual_bits: u64,
    pub p_t_initial_bits: u64,
    pub p_t_final_bits: u64,
    pub p_t_max_drift_bits: u64,
    pub signed_event_value_bits: Option<u64>,
    pub approach_tolerance_bits: Option<u64>,
    pub surface_approach_reason: Option<SurfaceApproachReason>,
    pub localization_termination: Option<String>,
    pub typed_error_class: Option<ErrorClass>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct CanonicalCorpusReport {
    pub schema: String,
    pub case_count: usize,
    pub cases: Vec<CanonicalCaseRecord>,
}

#[derive(Debug, Clone, Serialize)]
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
    KerrParams::new(1.0e-18, 0.0).expect("minkowski params")
}

fn minkowski_straight_state() -> GeodesicState {
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
        IntegrationError::EventLocalizationDidNotConverge { .. } => {
            ErrorClass::EventLocalizationDidNotConverge
        }
        IntegrationError::MissingEventOutcome | IntegrationError::InvalidInterpolantBounds => {
            ErrorClass::Solver
        }
    }
}

fn expected_label(e: &ExpectedOutcome) -> String {
    match e {
        ExpectedOutcome::Event(id) => format!("Event({id:?})"),
        ExpectedOutcome::SurfaceApproach { event_id, reason } => {
            format!("SurfaceApproach({event_id:?},{reason:?})")
        }
        ExpectedOutcome::AffineLimit => "AffineLimit".into(),
        ExpectedOutcome::Error(c) => format!("Error({c:?})"),
    }
}

fn matches_expected(report: &IntegrationReport, expected: &ExpectedOutcome) -> bool {
    match (expected, &report.outcome) {
        (ExpectedOutcome::AffineLimit, IntegrationOutcome::AffineLimit { .. }) => true,
        (ExpectedOutcome::Event(id), IntegrationOutcome::Event(hit)) => hit.event_id == *id,
        (
            ExpectedOutcome::SurfaceApproach { event_id, reason },
            IntegrationOutcome::SurfaceApproach(a),
        ) => a.event_id == *event_id && a.reason == *reason && a.signed_event_value > 0.0,
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
            cfg.affine_limit = 50.0;
            cfg.max_step = 0.5;
            let esc = EscapeSphere::new(params, 20.0).expect("escape");
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
            let (y0, r0) = ray_state(
                &params,
                20.0,
                std::f64::consts::FRAC_PI_2,
                SensorCoord { x: 0.0, y: 0.0 },
            )
            .map_err(|e| (case.expected.clone(), e))?;
            cfg.affine_limit = 200.0;
            cfg.max_step = 0.5;
            // Opt-in OuterHorizon proximity — separate from event_value_tolerance.
            // Documents stall; does not claim exact horizon crossing.
            cfg.horizon_proximity =
                HorizonProximityPolicy::enabled(1e-10).expect("horizon proximity");
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
            // Proximity/stall must never appear as EventHit.
            if matches!(report.outcome, IntegrationOutcome::Event(_))
                && matches!(expected, ExpectedOutcome::SurfaceApproach { .. })
            {
                return Err(format!(
                    "{}: SurfaceApproach expected but got EventHit",
                    case.id.as_str()
                ));
            }
            if let IntegrationOutcome::Event(hit) = &report.outcome {
                if hit.localization.termination
                    == crate::event::LocalizationTermination::ExactEndpoint
                    || hit.localization.interpolation_calls > 0
                    || matches!(
                        hit.localization.termination,
                        crate::event::LocalizationTermination::EventValueTolerance
                            | crate::event::LocalizationTermination::AffineWidthTolerance
                    )
                {
                    // ok — has a valid localization termination kind
                } else {
                    return Err(format!(
                        "{}: EventHit missing localization termination",
                        case.id.as_str()
                    ));
                }
            }
            if let IntegrationOutcome::SurfaceApproach(a) = &report.outcome {
                if a.signed_event_value <= 0.0 {
                    return Err(format!(
                        "{}: SurfaceApproach must have positive residual (not crossed)",
                        case.id.as_str()
                    ));
                }
            }
            if !matches_expected(&report, expected) {
                return Err(format!(
                    "{}: expected {:?}, got {} ({:?})",
                    case.id.as_str(),
                    expected,
                    report.outcome.variant_name(),
                    report.outcome
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

fn record_from_report(
    case: &CorpusCase,
    report: Option<&IntegrationReport>,
) -> CanonicalCaseRecord {
    match report {
        None => CanonicalCaseRecord {
            case: case.id.as_str().into(),
            expected_outcome: expected_label(&case.expected),
            actual_outcome: "Error".into(),
            termination_id: None,
            lambda_bits: None,
            state_bits: None,
            raw_stop_lambda_bits: None,
            raw_stop_state_bits: None,
            accepted_steps: 0,
            rejected_steps: 0,
            rhs_evaluations: 0,
            callback_count: 0,
            h_initial_bits: 0,
            h_final_bits: 0,
            h_max_residual_bits: 0,
            p_t_initial_bits: 0,
            p_t_final_bits: 0,
            p_t_max_drift_bits: 0,
            signed_event_value_bits: None,
            approach_tolerance_bits: None,
            surface_approach_reason: None,
            localization_termination: None,
            typed_error_class: match &case.expected {
                ExpectedOutcome::Error(c) => Some(*c),
                _ => None,
            },
        },
        Some(r) => {
            let st = r.outcome.stats();
            let (
                termination_id,
                lambda_bits,
                state_bits,
                raw_l,
                raw_s,
                signed_f,
                approach_tol,
                approach_reason,
                loc_term,
            ) = match &r.outcome {
                IntegrationOutcome::Event(hit) => (
                    Some(hit.event_id),
                    Some(hit.lambda.0.to_bits()),
                    Some(hit.state.bits_hex()),
                    Some(hit.raw_solver_stop.lambda.0.to_bits()),
                    Some(hit.raw_solver_stop.state.bits_hex()),
                    Some(hit.event_value.to_bits()),
                    None,
                    None,
                    Some(format!("{:?}", hit.localization.termination)),
                ),
                IntegrationOutcome::SurfaceApproach(a) => (
                    Some(a.event_id),
                    Some(a.lambda.0.to_bits()),
                    Some(a.state.bits_hex()),
                    Some(a.raw_solver_stop.lambda.0.to_bits()),
                    Some(a.raw_solver_stop.state.bits_hex()),
                    Some(a.signed_event_value.to_bits()),
                    Some(a.approach_tolerance.to_bits()),
                    Some(a.reason),
                    None,
                ),
                IntegrationOutcome::AffineLimit { lambda, state, .. } => (
                    None,
                    Some(lambda.0.to_bits()),
                    Some(state.bits_hex()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
            };
            CanonicalCaseRecord {
                case: case.id.as_str().into(),
                expected_outcome: expected_label(&case.expected),
                actual_outcome: r.outcome.variant_name().into(),
                termination_id,
                lambda_bits,
                state_bits,
                raw_stop_lambda_bits: raw_l,
                raw_stop_state_bits: raw_s,
                accepted_steps: st.accepted_steps,
                rejected_steps: st.rejected_steps,
                rhs_evaluations: st.rhs_evaluations,
                callback_count: st.callback_count,
                h_initial_bits: r.diagnostics.h_initial.to_bits(),
                h_final_bits: r.diagnostics.h_final.to_bits(),
                h_max_residual_bits: r.diagnostics.h_max_abs_residual.to_bits(),
                p_t_initial_bits: r.diagnostics.p_t_initial.to_bits(),
                p_t_final_bits: r.diagnostics.p_t_final.to_bits(),
                p_t_max_drift_bits: r.diagnostics.p_t_max_abs_drift.to_bits(),
                signed_event_value_bits: signed_f,
                approach_tolerance_bits: approach_tol,
                surface_approach_reason: approach_reason,
                localization_termination: loc_term,
                typed_error_class: None,
            }
        }
    }
}

/// Emit sorted canonical corpus JSON (exact case count, no duplicates/skips).
pub fn build_canonical_corpus_report() -> Result<CanonicalCorpusReport, String> {
    let mut cases = Vec::with_capacity(CORPUS.len());
    for case in CORPUS {
        let report = run_and_check(case)?;
        cases.push(record_from_report(case, report.as_ref()));
    }
    cases.sort_by(|a, b| a.case.cmp(&b.case));
    // Uniqueness
    for w in cases.windows(2) {
        if w[0].case == w[1].case {
            return Err(format!("duplicate corpus case {}", w[0].case));
        }
    }
    if cases.len() != CORPUS.len() {
        return Err(format!(
            "case count mismatch: got {} expected {}",
            cases.len(),
            CORPUS.len()
        ));
    }
    Ok(CanonicalCorpusReport {
        schema: "gate-1b1-corpus-v1".into(),
        case_count: cases.len(),
        cases,
    })
}

pub fn canonical_corpus_json() -> Result<String, String> {
    let report = build_canonical_corpus_report()?;
    serde_json::to_string(&report).map_err(|e| e.to_string())
}

pub fn determinism_record(
    case: &CorpusCase,
    report: Option<&IntegrationReport>,
) -> DeterminismRecord {
    let c = record_from_report(case, report);
    DeterminismRecord {
        case: c.case,
        outcome_variant: c.actual_outcome,
        event_id: c.termination_id,
        lambda_bits: c.lambda_bits,
        state_bits: c.state_bits,
        raw_stop_lambda_bits: c.raw_stop_lambda_bits,
        raw_stop_state_bits: c.raw_stop_state_bits,
        accepted_steps: c.accepted_steps,
        rejected_steps: c.rejected_steps,
        rhs_evaluations: c.rhs_evaluations,
        h_initial_bits: c.h_initial_bits,
        h_final_bits: c.h_final_bits,
        p_t_max_drift_bits: c.p_t_max_drift_bits,
        error_class: c.typed_error_class,
    }
}
