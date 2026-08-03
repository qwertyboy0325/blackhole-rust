use relativity_core::{Covector, KerrParams, PositionKs};
use relativity_integrate::{
    integrate, Dop853Config, EscapeSphere, EventId, EventSurface, GeodesicState, IntegrationError,
    IntegrationStage,
};

#[test]
fn core_domain_becomes_physics_domain() {
    let params = KerrParams::new(1.0, 0.9).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 0.0, 0.0, 0.0),
        Covector::new(1.0, 0.0, 0.0, 0.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 1.0;
    let err = integrate(params, &y0, &cfg, &[]).unwrap_err();
    assert!(
        matches!(err, IntegrationError::PhysicsDomain { .. }),
        "{err}"
    );
}

#[test]
fn event_function_failure_preserves_event_id() {
    // EscapeSphere with valid r_escape but evaluate at singularity → EventDomain.
    let params = KerrParams::new(1.0, 0.9).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 0.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap();
    // First RHS may PhysicsDomain before events; use a state that starts valid then
    // hits domain via event value at endpoints — covered by PhysicsDomain latch first.
    // Direct unit: construct EventDomain manually path via EscapeSphere::value.
    let esc = EscapeSphere::new(params, 10.0).unwrap();
    let err = esc
        .value(relativity_integrate::AffineParameter(0.0), &y0)
        .unwrap_err();
    match err {
        IntegrationError::EventDomain { event_id, .. } => {
            assert_eq!(event_id, EventId::EscapeSphere);
        }
        other => panic!("expected EventDomain, got {other}"),
    }
}

#[test]
fn step_limit_exhaustion_typed() {
    let params = KerrParams::new(1.0e-18, 0.0).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 100.0;
    cfg.max_step = 0.01;
    cfg.max_accepted_steps = 3;
    let err = integrate(params, &y0, &cfg, &[]).unwrap_err();
    assert!(
        matches!(err, IntegrationError::StepLimitExceeded { .. }),
        "{err}"
    );
}

#[test]
fn no_panic_on_corpus_error_case() {
    use relativity_integrate::{run_and_check, CORPUS};
    for case in CORPUS {
        let _ = run_and_check(case);
    }
}

#[test]
fn non_finite_stage_is_typed_not_nan_identity() {
    let err = IntegrationError::NonFiniteState {
        stage: IntegrationStage::Rhs,
    };
    let s = err.to_string();
    assert!(!s.contains("NaN"));
    assert!(s.contains("non-finite"));
}
