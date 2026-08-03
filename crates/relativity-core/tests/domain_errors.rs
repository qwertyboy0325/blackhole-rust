//! Typed rejection of non-finite, invalid-spin, singular, and ill-conditioned inputs.

use relativity_core::{
    bl_to_ks_position, evaluate_hamiltonian, evaluate_kerr_schild, evaluate_oblate_radius,
    initialize_rectilinear_ray, ks_to_bl_position, zamo_observer, CameraParams, CoreError,
    Covector, DomainReason, KerrParams, PositionBl, PositionKs, SensorCoord,
};

#[test]
fn rejects_invalid_spin_and_nonfinite_metric_inputs() {
    assert!(matches!(
        KerrParams::new(1.0, 1.1),
        Err(CoreError::InvalidSpin { .. })
    ));
    let p = KerrParams::new(1.0, 0.5).unwrap();
    assert!(matches!(
        evaluate_kerr_schild(&p, &PositionKs::spatial(f64::INFINITY, 0.0, 1.0)),
        Err(CoreError::NonFinite { .. })
    ));
}

#[test]
fn rejects_ring_singularity_without_mapping_to_r_zero_success() {
    let p = KerrParams::new(1.0, 0.8).unwrap();
    let err = evaluate_oblate_radius(&p, &PositionKs::spatial(0.8, 0.0, 0.0)).unwrap_err();
    assert!(matches!(
        err,
        CoreError::ChartDomain {
            reason: DomainReason::RingSingularityOrExcludedDisk,
            ..
        }
    ));
}

#[test]
fn rejects_bl_axis_and_inside_horizon_zamo() {
    let p = KerrParams::new(1.0, 0.9).unwrap();
    assert!(matches!(
        ks_to_bl_position(&p, &PositionKs::spatial(0.0, 0.0, 12.0)),
        Err(CoreError::ChartDomain {
            reason: DomainReason::BoyerLindquistSingular,
            ..
        })
    ));
    let bl = PositionBl::new(0.0, 1.1, 1.0, 0.0);
    assert!(zamo_observer(&p, &bl).is_err());
    // Valid BL still maps.
    let bl_ok = PositionBl::new(0.0, 10.0, 1.0, 0.2);
    assert!(bl_to_ks_position(&p, &bl_ok).is_ok());
}

#[test]
fn hamiltonian_rejects_nonfinite_momentum() {
    let p = KerrParams::new(1.0, 0.2).unwrap();
    let pos = PositionKs::spatial(8.0, 0.0, 0.0);
    let bad = Covector::new(f64::NAN, 0.0, 0.0, 0.0);
    assert!(matches!(
        evaluate_hamiltonian(&p, &pos, &bad),
        Err(CoreError::NonFinite { .. })
    ));
}

#[test]
fn ray_init_rejects_bad_fov() {
    let p = KerrParams::new(1.0, 0.5).unwrap();
    let bl = PositionBl::new(0.0, 25.0, 1.0, 0.0);
    let obs = zamo_observer(&p, &bl).unwrap();
    let cam = CameraParams {
        horizontal_fov: -1.0,
        roll: 0.0,
    };
    assert!(initialize_rectilinear_ray(&p, &obs, &cam, SensorCoord { x: 0.0, y: 0.0 }).is_err());
}
