use relativity_core::{Covector, KerrParams, PositionKs};
use relativity_integrate::{
    integrate, Dop853Config, EscapeSphere, EventId, EventSurface, GeodesicState, IntegrationOutcome,
};

fn quasi_minkowski() -> KerrParams {
    KerrParams::new(1.0e-18, 0.0).unwrap()
}

fn outward_null() -> GeodesicState {
    GeodesicState::new(
        PositionKs::new(0.0, 10.0, 0.0, 0.0),
        Covector::new(1.0, 1.0, 0.0, 0.0),
    )
    .unwrap()
}

#[test]
fn minkowski_position_follows_analytic_null_line() {
    let params = quasi_minkowski();
    let y0 = outward_null();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 5.0;
    cfg.max_step = 0.25;
    let report = integrate(params, &y0, &cfg, &[]).unwrap();
    let IntegrationOutcome::AffineLimit { lambda, state, .. } = report.outcome else {
        panic!("expected affine limit");
    };
    let lam = lambda.0;
    // Analytic: t(λ)=−λ, x(λ)=10+λ for η-raised k with p=(1,1,0,0).
    let err_t = (state.position.t - (-lam)).abs();
    let err_x = (state.position.x - (10.0 + lam)).abs();
    let err_y = state.position.y.abs();
    let err_z = state.position.z.abs();
    assert!(err_t < 1e-6, "t err {err_t}");
    assert!(err_x < 1e-6, "x err {err_x}");
    assert!(err_y < 1e-8 && err_z < 1e-8);
}

#[test]
fn minkowski_momentum_constant() {
    let params = quasi_minkowski();
    let y0 = outward_null();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 5.0;
    let report = integrate(params, &y0, &cfg, &[]).unwrap();
    let IntegrationOutcome::AffineLimit { state, .. } = report.outcome else {
        panic!("expected affine limit");
    };
    assert!((state.momentum.t - 1.0).abs() < 1e-10);
    assert!((state.momentum.x - 1.0).abs() < 1e-10);
    assert!(state.momentum.y.abs() < 1e-12);
    assert!(state.momentum.z.abs() < 1e-12);
}

#[test]
fn escape_event_matches_analytic() {
    let params = quasi_minkowski();
    let y0 = outward_null();
    let mut cfg = Dop853Config::diagnostic_default();
    cfg.affine_limit = 50.0;
    cfg.max_step = 0.5;
    let esc = EscapeSphere::new(params, 20.0).unwrap();
    let surfaces: [&dyn EventSurface; 1] = [&esc];
    let report = integrate(params, &y0, &cfg, &surfaces).unwrap();
    let IntegrationOutcome::Event(hit) = report.outcome else {
        panic!("expected escape event");
    };
    assert_eq!(hit.event_id, EventId::EscapeSphere);
    let lam_analytic = 10.0;
    assert!(
        (hit.lambda.0 - lam_analytic).abs() < 1e-8,
        "lambda {} vs analytic {lam_analytic}",
        hit.lambda.0
    );
    assert!((hit.state.position.x - 20.0).abs() < 1e-6);
    // Localized differs from raw stop when event is interior to the step.
    assert!(
        (hit.raw_solver_stop.lambda.0 - hit.lambda.0).abs() > 0.0
            || hit.localization.interpolation_calls > 0
    );
    assert_ne!(hit.state.to_array(), hit.raw_solver_stop.state.to_array());
}

#[test]
fn tighter_tolerances_reduce_or_preserve_endpoint_error() {
    let params = quasi_minkowski();
    let y0 = outward_null();
    let mut loose = Dop853Config::diagnostic_default();
    loose.affine_limit = 5.0;
    loose.relative_tolerance = [1e-8; 8];
    loose.absolute_tolerance = [1e-10; 8];
    let mut tight = loose.clone().with_tighter_tol(1e-2);
    tight.affine_limit = 5.0;

    let r_loose = integrate(params, &y0, &loose, &[]).unwrap();
    let r_tight = integrate(params, &y0, &tight, &[]).unwrap();
    let IntegrationOutcome::AffineLimit {
        lambda: l0,
        state: s0,
        ..
    } = r_loose.outcome
    else {
        panic!();
    };
    let IntegrationOutcome::AffineLimit {
        lambda: l1,
        state: s1,
        ..
    } = r_tight.outcome
    else {
        panic!();
    };
    let e0 = (s0.position.x - (10.0 + l0.0)).abs();
    let e1 = (s1.position.x - (10.0 + l1.0)).abs();
    assert!(e1 <= e0 * 1.01 + 1e-14, "loose {e0} tight {e1}");
}
