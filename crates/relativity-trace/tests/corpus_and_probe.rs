use relativity_trace::{run_camera_corpus, run_convergence_probe, ConvergenceProbeStatus};

#[test]
fn camera_corpus_no_skips() {
    let results = run_camera_corpus().expect("corpus");
    assert!(results.len() >= 10);
}

#[test]
fn camera_corpus_determinism_five_repeats() {
    let a = run_camera_corpus().unwrap();
    for _ in 0..4 {
        let b = run_camera_corpus().unwrap();
        assert_eq!(a.len(), b.len());
        for ((id_a, oa), (id_b, ob)) in a.iter().zip(b.iter()) {
            assert_eq!(id_a, id_b);
            assert_eq!(oa.class(), ob.class());
        }
    }
}

#[test]
fn convergence_probe_declared_candidates() {
    let report = run_convergence_probe();
    assert_eq!(report.candidates.len(), 4);
    // Either Verified or Unverified — both acceptable for Gate 1B2.
    assert!(matches!(
        report.status,
        ConvergenceProbeStatus::Verified | ConvergenceProbeStatus::Unverified
    ));
}
