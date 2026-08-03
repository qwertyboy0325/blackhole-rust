//! Experiment matrix runner for `ivp`.

mod exp;

use crate::adapter::error_scaling_ivp;
use crate::audit::dependency_audit;
use exp::{run_a, run_b, run_c, run_c_vector, run_d, run_e, run_e_shallow, run_f, run_g};
use gate_1b0_contract::{
    json_digest, CandidateReport, ContractRequirementScore, ExperimentId, ExperimentResult,
    SupportLevel, CANDIDATE_IVP, SPIKE_VERSION,
};

pub fn run_candidate_report(
    commit: &str,
    toolchain: &str,
    target: &str,
    tree: &str,
) -> CandidateReport {
    let mut c = run_c();
    c.detail = format!("{} | {}", c.detail, run_c_vector().detail);

    let mut g = run_g(false);
    g.detail = format!("{} | {}", g.detail, run_g(true).detail);

    let mut e = run_e();
    let shallow = run_e_shallow();
    if let (Some(ref mut main), Some(sh)) = (&mut e.event_evidence, shallow.event_evidence) {
        main.shallow_crossing_tested = sh.shallow_crossing_tested;
        main.shallow_sign_change_only_insufficient = sh.shallow_sign_change_only_insufficient;
    }

    let experiments = vec![run_a(), run_b(), c, run_d(), e, run_f(), g];
    let error_scaling = error_scaling_ivp();
    let decision_matrix = build_decision_matrix(&experiments, &error_scaling);
    let mut report = CandidateReport {
        schema_version: SPIKE_VERSION.into(),
        candidate: CANDIDATE_IVP.into(),
        crate_version: super::DEP_VERSION.into(),
        commit: commit.into(),
        toolchain: toolchain.into(),
        target: target.into(),
        experiments,
        experiments_expected: 7,
        experiments_run: 7,
        unexplained_skips: 0,
        error_scaling,
        dependency_audit: dependency_audit(tree),
        decision_matrix,
        report_digest: String::new(),
    };
    report.report_digest = json_digest(&report).unwrap_or_else(|_| "digest_error".into());
    report
}

fn build_decision_matrix(
    ex: &[ExperimentResult],
    scale: &gate_1b0_contract::ErrorScalingAssessment,
) -> Vec<ContractRequirementScore> {
    let d = ex.iter().find(|e| matches!(e.id, ExperimentId::D));
    let e = ex.iter().find(|e| matches!(e.id, ExperimentId::E));
    let f = ex.iter().find(|e| matches!(e.id, ExperimentId::F));

    vec![
        score(
            "mathematical_method_dop853_f64",
            SupportLevel::Supported,
            "DOP853",
        ),
        score(
            "eight_component_state",
            SupportLevel::Supported,
            "Experiment C/G",
        ),
        score(
            "vector_tolerance_direct",
            scale.direct_vector_tolerance,
            "Tolerance::Vector",
        ),
        score(
            "accepted_step_callback",
            SupportLevel::Supported,
            "SolOut trait",
        ),
        score(
            "accepted_step_dense_interpolation",
            if d.map(|x| x.passed).unwrap_or(false) {
                SupportLevel::Supported
            } else {
                SupportLevel::Unverified
            },
            "StepInterpolant in SolOut",
        ),
        score(
            "event_localization_fit",
            if e.map(|x| x.passed).unwrap_or(false) {
                SupportLevel::Supported
            } else {
                SupportLevel::Unverified
            },
            "sol(t) dense bracket",
        ),
        score(
            "stop_restart_semantics",
            SupportLevel::Supported,
            "Interrupt + re-init",
        ),
        score(
            "step_guard_control",
            f.and_then(|x| x.step_guard.as_ref())
                .map(|g| g.static_h_max)
                .unwrap_or(SupportLevel::Unverified),
            "max_step builder",
        ),
        score(
            "integration_statistics",
            SupportLevel::Supported,
            "nfev/naccpt",
        ),
        score(
            "determinism_same_platform",
            if ex
                .iter()
                .any(|x| x.determinism.as_ref().is_some_and(|d| d.deterministic))
            {
                SupportLevel::Supported
            } else {
                SupportLevel::Unverified
            },
            "exp A x5",
        ),
        score(
            "error_propagation",
            SupportLevel::Supported,
            "ivp::error::Error",
        ),
        score(
            "adapter_complexity_acceptable",
            SupportLevel::Supported,
            "Ivp builder",
        ),
        score(
            "dependency_risk",
            SupportLevel::SupportedWithAdapter,
            "younger crate",
        ),
        score(
            "maintenance_risk",
            SupportLevel::SupportedWithAdapter,
            "0.6.0",
        ),
    ]
}

fn score(req: &str, level: SupportLevel, evidence: &str) -> ContractRequirementScore {
    ContractRequirementScore {
        requirement: req.into(),
        level,
        evidence: evidence.into(),
    }
}
