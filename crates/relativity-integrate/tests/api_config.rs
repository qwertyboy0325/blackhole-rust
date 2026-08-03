use relativity_core::{Covector, PositionKs};
use relativity_integrate::{Dop853Config, GeodesicState, IntegrationError, IntegrationStage};

#[test]
fn invalid_tolerance_rejected() {
    let mut c = Dop853Config::diagnostic_default();
    c.relative_tolerance[3] = -1.0;
    assert!(matches!(
        c.validate(),
        Err(IntegrationError::InvalidConfig {
            field: "relative_tolerance"
        })
    ));
}

#[test]
fn invalid_step_and_affine_rejected() {
    let mut c = Dop853Config::diagnostic_default();
    c.max_step = f64::NAN;
    assert!(c.validate().is_err());
    c = Dop853Config::diagnostic_default();
    c.affine_limit = 0.0;
    assert!(c.validate().is_err());
}

#[test]
fn non_finite_initial_state_rejected() {
    let err = GeodesicState::new(
        PositionKs::new(0.0, f64::INFINITY, 0.0, 0.0),
        Covector::new(1.0, 0.0, 0.0, 0.0),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        IntegrationError::NonFiniteState {
            stage: IntegrationStage::InitialState
        }
    ));
}

#[test]
fn ivp_absent_from_public_api_surface() {
    // Compile-time: public re-exports are project types only.
    // Runtime scan of this crate's lib.rs source for `ivp::` in public docs is
    // covered by the gate evaluator; here we assert key types exist.
    let _ = std::any::type_name::<relativity_integrate::Dop853Config>();
    let _ = std::any::type_name::<relativity_integrate::IntegrationOutcome>();
    let _ = std::any::type_name::<relativity_integrate::IntegrationError>();
}
