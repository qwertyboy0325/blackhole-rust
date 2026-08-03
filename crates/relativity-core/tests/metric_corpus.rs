//! Stratified metric identity / Kerr–Schild invariant corpus.

use relativity_core::{
    evaluate_kerr_schild, identity_residual, matrix_inverse_oracle, stratified_corpus,
    ExpectedOutcome, KerrParams, MetricTensor, PositionKs, Vector, CORPUS_SEED,
};

#[test]
fn corpus_ks_invariants_and_inverse_identity() {
    let mut worst_id = 0.0_f64;
    let mut worst_eta_ll = 0.0_f64;
    let mut worst_g_ll = 0.0_f64;
    let mut worst_det = 0.0_f64;
    let mut worst_raw_asym = 0.0_f64;
    let mut worst_pos = PositionKs::spatial(0.0, 0.0, 0.0);
    let mut n_valid = 0;
    let mut n_expected_fail = 0;

    for pt in stratified_corpus() {
        let params = pt.params().unwrap();
        match pt.expected {
            ExpectedOutcome::ExpectedDomainFailure(reason) => {
                n_expected_fail += 1;
                let err = evaluate_kerr_schild(&params, &pt.pos).unwrap_err();
                match err {
                    relativity_core::CoreError::ChartDomain { reason: r, .. } => {
                        assert_eq!(r, reason, "domain reason mismatch at {:?}", pt.pos);
                    }
                    other => panic!("expected domain failure, got {other:?}"),
                }
            }
            ExpectedOutcome::Valid => {
                let q = evaluate_kerr_schild(&params, &pt.pos).unwrap();
                n_valid += 1;
                assert!(q.metric.max_abs_asymmetry() < 1e-14);
                let id = identity_residual(&q.metric, &q.inverse_metric);
                if id > worst_id {
                    worst_id = id;
                    worst_pos = pt.pos;
                }
                assert!(
                    id < 1e-9,
                    "identity {id} at {:?} seed={CORPUS_SEED}",
                    pt.pos
                );

                let eta = MetricTensor::minkowski();
                let ell = Vector::from_components(q.ell_con);
                let eta_ll = eta.contract(&ell, &ell).abs();
                let g_ll = q.metric.contract(&ell, &ell).abs();
                let det_res = (q.metric.determinant() + 1.0).abs();
                worst_eta_ll = worst_eta_ll.max(eta_ll);
                worst_g_ll = worst_g_ll.max(g_ll);
                worst_det = worst_det.max(det_res);
                assert!(eta_ll < 1e-10, "η(ℓ,ℓ)={eta_ll}");
                assert!(g_ll < 1e-10, "g(ℓ,ℓ)={g_ll}");
                assert!(det_res < 1e-8, "det(g)+1={det_res}");

                let oracle = matrix_inverse_oracle(&q.metric).unwrap();
                worst_raw_asym = worst_raw_asym.max(oracle.raw_asymmetry);
                assert!(oracle.raw_asymmetry < 1e-9);
                assert!(oracle.identity_residual < 1e-9);
            }
        }
    }

    assert!(n_valid >= 15);
    assert!(n_expected_fail >= 2);
    eprintln!(
        "corpus invariants seed={CORPUS_SEED} valid={n_valid} expected_fail={n_expected_fail} \
         worst_id={worst_id:.3e}@{:?} eta(l,l)={worst_eta_ll:.3e} g(l,l)={worst_g_ll:.3e} \
         |det+1|={worst_det:.3e} raw_inv_asym={worst_raw_asym:.3e}",
        worst_pos
    );
}

#[test]
fn lorentzian_via_ks_null_update_documented() {
    // For g = η + 2H ℓ⊗ℓ with η(ℓ,ℓ)=0, the matrix determinant identity
    // det(g)=det(η)=−1 holds, and g inherits Lorentzian inertia from η by the
    // null-update theorem (Kerr–Schild). This is the Gate 1A signature proof —
    // not weak-field diagonal sign checks.
    let p = KerrParams::new(1.0, 0.9).unwrap();
    for pos in [
        PositionKs::spatial(10.0, 0.0, 0.0),
        PositionKs::spatial(2.0, 0.5, 0.5),
        PositionKs::spatial(0.0, 0.0, 6.0),
    ] {
        let q = evaluate_kerr_schild(&p, &pos).unwrap();
        let ell = Vector::from_components(q.ell_con);
        assert!(MetricTensor::minkowski().contract(&ell, &ell).abs() < 1e-12);
        assert!((q.metric.determinant() + 1.0).abs() < 1e-10);
    }
}
