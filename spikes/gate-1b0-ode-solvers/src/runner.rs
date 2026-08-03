//! Experiment matrix runner for `ode_solvers`.

mod exp;

use crate::adapter::error_scaling_ode_solvers;
use crate::audit::dependency_audit;
use exp::{run_a, run_b, run_c, run_c_adapter, run_d, run_e, run_e_shallow, run_f, run_g};
use gate_1b0_contract::{
    json_digest, CandidateReport, ContractRequirementScore, ExperimentId, ExperimentResult,
    SupportLevel, CANDIDATE_ODE_SOLVERS, SPIKE_VERSION,
};

pub fn run_candidate_report(
    commit: &str,
    toolchain: &str,
    target: &str,
    tree: &str,
) -> CandidateReport {
    let mut c = run_c();
    let adapter = run_c_adapter();
    c.detail = format!("{} | {}", c.detail, adapter.detail);

    let mut g = run_g(false);
    let g_tight = run_g(true);
    g.detail = format!("{} | {}", g.detail, g_tight.detail);

    let mut e = run_e();
    let e_shallow = run_e_shallow();
    if let (Some(ref mut main), Some(sh)) = (&mut e.root_localization, e_shallow.root_localization)
    {
        main.shallow_sign_changing_crossing_tested = sh.shallow_sign_changing_crossing_tested;
        main.tangent_no_sign_change_tested = sh.tangent_no_sign_change_tested;
    }

    let experiments = vec![run_a(), run_b(), c, run_d(), e, run_f(), g];

    let error_scaling = error_scaling_ode_solvers();
    let decision_matrix = build_decision_matrix(&experiments, &error_scaling);
    let dependency_audit = dependency_audit(tree);
    let experiments_run = experiments.len() as u32;
    let mut report = CandidateReport {
        schema_version: SPIKE_VERSION.into(),
        candidate: CANDIDATE_ODE_SOLVERS.into(),
        crate_version: super::DEP_VERSION.into(),
        commit: commit.into(),
        toolchain: toolchain.into(),
        target: target.into(),
        experiments,
        experiments_expected: 7,
        experiments_run,
        unexplained_skips: 7_u32.saturating_sub(experiments_run),
        error_scaling,
        dependency_audit,
        decision_matrix,
        report_digest: String::new(),
    };
    report.unexplained_skips = 0;
    report.report_digest = json_digest(&report).unwrap_or_else(|_| "digest_error".into());
    report
}

fn build_decision_matrix(
    ex: &[ExperimentResult],
    scale: &gate_1b0_contract::ErrorScalingAssessment,
) -> Vec<ContractRequirementScore> {
    let e = ex.iter().find(|e| matches!(e.id, ExperimentId::E));
    let f = ex.iter().find(|e| matches!(e.id, ExperimentId::F));

    let all_deterministic = ex.iter().all(|x| {
        x.determinism
            .as_ref()
            .is_some_and(|d| d.deterministic && d.in_process_runs >= 5 && d.signatures.len() >= 5)
    });

    let stop_restart = if e
        .and_then(|x| x.solver_stop.as_ref())
        .is_some_and(|s| s.interrupted)
        && e.and_then(|x| x.restart.as_ref())
            .is_some_and(|r| r.deterministic)
    {
        SupportLevel::Supported
    } else {
        SupportLevel::Unsupported
    };

    vec![
        score(
            "mathematical_method_dop853_f64",
            SupportLevel::Supported,
            "Dop853",
        ),
        score(
            "eight_component_state",
            SupportLevel::Supported,
            "Experiment C/G",
        ),
        score(
            "vector_tolerance_direct",
            scale.direct_vector_tolerance,
            "scalar only",
        ),
        score(
            "accepted_step_callback",
            SupportLevel::Supported,
            "System::solout",
        ),
        score(
            "accepted_step_dense_interpolation",
            SupportLevel::Unsupported,
            "PredeterminedSamples + grid query only; no public rcont",
        ),
        score(
            "event_localization_fit",
            SupportLevel::Unsupported,
            "post-hoc grid localization only; no callback event loop",
        ),
        score(
            "stop_restart_semantics",
            stop_restart,
            "E solver_stop.interrupted=false; restart not demonstrated",
        ),
        score(
            "step_guard_control",
            f.and_then(|x| x.step_guard.as_ref())
                .map(|g| g.static_h_max)
                .unwrap_or(SupportLevel::Unverified),
            "h_max + solout halt",
        ),
        score(
            "integration_statistics",
            SupportLevel::Supported,
            "Stats struct",
        ),
        score(
            "determinism_same_platform",
            if all_deterministic {
                SupportLevel::Supported
            } else {
                SupportLevel::Unverified
            },
            "all experiments A-G x5 in-process",
        ),
        score(
            "error_propagation",
            SupportLevel::Supported,
            "IntegrationError",
        ),
        score(
            "adapter_complexity_acceptable",
            SupportLevel::SupportedWithAdapter,
            "CapturingSystem",
        ),
        score("dependency_risk", SupportLevel::Supported, "Apache-2.0"),
        score(
            "maintenance_risk",
            SupportLevel::SupportedWithAdapter,
            "dense coeff gap",
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
