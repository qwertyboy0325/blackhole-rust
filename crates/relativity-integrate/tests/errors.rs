use relativity_core::{Covector, KerrParams, PositionKs};
use relativity_integrate::{
    integrate, AffineParameter, Dop853Config, EscapeSphere, EventId, EventSurface, GeodesicState,
    IntegrationError, IntegrationStage,
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

struct FailingSurface;

impl EventSurface for FailingSurface {
    fn id(&self) -> EventId {
        EventId::EscapeSphere
    }

    fn value(
        &self,
        _lambda: AffineParameter,
        _state: &GeodesicState,
    ) -> Result<f64, IntegrationError> {
        Err(IntegrationError::EventDomain {
            event_id: EventId::EscapeSphere,
            detail: "test failure".into(),
        })
    }

    fn crossing(&self) -> relativity_integrate::CrossingDirection {
        relativity_integrate::CrossingDirection::Increasing
    }
}

#[test]
fn event_domain_lifecycle_preserves_event_id() {
    // Valid exterior state so RHS succeeds; event value fails in SolOut.
    let params = KerrParams::new(1.0e-18, 0.0).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 5.0;
    cfg.max_step = 0.5;
    let surf = FailingSurface;
    let surfaces: [&dyn EventSurface; 1] = [&surf];
    let err = integrate(params, &y0, &cfg, &surfaces).unwrap_err();
    match err {
        IntegrationError::EventDomain { event_id, .. } => {
            assert_eq!(event_id, EventId::EscapeSphere);
        }
        other => panic!("expected EventDomain through latch, got {other}"),
    }
}

#[test]
fn event_function_failure_preserves_event_id_direct() {
    let params = KerrParams::new(1.0, 0.9).unwrap();
    let y0 = GeodesicState::new(
        PositionKs::new(0.0, 0.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap();
    let esc = EscapeSphere::new(params, 10.0).unwrap();
    let err = esc.value(AffineParameter(0.0), &y0).unwrap_err();
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
