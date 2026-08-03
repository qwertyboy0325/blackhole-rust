use relativity_integrate::{
    build_canonical_corpus_report, canonical_corpus_json, determinism_record, run_and_check,
    DeterminismRecord, CORPUS,
};
use serde_json::to_string;
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn complete_corpus_expected_outcomes() {
    for case in CORPUS {
        run_and_check(case).unwrap_or_else(|e| panic!("{e}"));
    }
    assert_eq!(CORPUS.len(), 10);
}

#[test]
fn in_process_determinism_five_repeats() {
    for case in CORPUS {
        let mut records: Vec<String> = Vec::new();
        for _ in 0..5 {
            let report = run_and_check(case).unwrap();
            let rec = determinism_record(case, report.as_ref());
            records.push(to_string(&rec).unwrap());
        }
        for r in &records[1..] {
            assert_eq!(r, &records[0], "determinism fail {}", case.id.as_str());
        }
    }
}

#[test]
fn determinism_record_shape() {
    let case = &CORPUS[0];
    let report = run_and_check(case).unwrap();
    let rec: DeterminismRecord = determinism_record(case, report.as_ref());
    assert_eq!(rec.case, case.id.as_str());
    assert!(!rec.outcome_variant.is_empty());
}

#[test]
fn canonical_corpus_sorted_unique_complete() {
    let report = build_canonical_corpus_report().unwrap();
    assert_eq!(report.case_count, CORPUS.len());
    assert_eq!(report.cases.len(), CORPUS.len());
    for w in report.cases.windows(2) {
        assert!(w[0].case < w[1].case);
    }
    let json = canonical_corpus_json().unwrap();
    let digest = sha256_hex(json.as_bytes());
    assert_eq!(digest.len(), 64);
}

#[test]
fn surface_approach_never_serialized_as_event() {
    use relativity_integrate::{CorpusId, IntegrationOutcome};
    for case in CORPUS {
        if case.id == CorpusId::SchwarzschildInwardHorizon {
            let report = run_and_check(case).unwrap().unwrap();
            assert!(matches!(
                report.outcome,
                IntegrationOutcome::SurfaceApproach(_)
            ));
            assert!(!matches!(report.outcome, IntegrationOutcome::Event(_)));
        }
    }
}
