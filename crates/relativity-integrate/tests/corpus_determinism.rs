use relativity_integrate::{determinism_record, run_and_check, DeterminismRecord, CORPUS};
use serde_json::to_string;

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
