//! Gate 1B0 candidate report validation.

use crate::schema::{
    CandidateReport, DenseOutputClass, ExperimentId, ExperimentResult, SupportLevel,
    ALL_EXPERIMENT_IDS, CANDIDATE_IVP, CANDIDATE_ODE_SOLVERS,
};

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub code: String,
    pub detail: String,
}

pub fn validate_candidate_report(report: &CandidateReport) -> Result<(), Vec<ValidationIssue>> {
    let mut issues = Vec::new();

    if report.experiments.len() != ALL_EXPERIMENT_IDS.len() {
        issues.push(issue(
            "experiment_count",
            format!(
                "expected {} experiments, got {}",
                ALL_EXPERIMENT_IDS.len(),
                report.experiments.len()
            ),
        ));
    }

    for id in ALL_EXPERIMENT_IDS {
        let matches: Vec<_> = report.experiments.iter().filter(|e| e.id == id).collect();
        if matches.is_empty() {
            issues.push(issue("missing_experiment", format!("missing {id:?}")));
        } else if matches.len() > 1 {
            issues.push(issue(
                "duplicate_experiment",
                format!("duplicate {id:?} count={}", matches.len()),
            ));
        }
    }

    for exp in &report.experiments {
        if !exp.passed {
            issues.push(issue(
                "experiment_failed",
                format!("{:?} failed: {}", exp.id, exp.detail),
            ));
        }
        issues.extend(validate_experiment_fields(&report.candidate, exp));
    }

    if report.unexplained_skips > 0 {
        issues.push(issue(
            "unexplained_skips",
            format!("skips={}", report.unexplained_skips),
        ));
    }

    issues.extend(validate_decision_matrix(report));

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn validate_experiment_fields(candidate: &str, exp: &ExperimentResult) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if exp
        .determinism
        .as_ref()
        .is_none_or(|d| !d.deterministic || d.in_process_runs < 5 || d.signatures.len() < 5)
    {
        issues.push(issue(
            "determinism_incomplete",
            format!("{:?} missing x5 in-process determinism", exp.id),
        ));
    }

    match exp.id {
        ExperimentId::A => {
            if exp.endpoint_abs_error.is_none() || exp.stats.is_none() {
                issues.push(issue(
                    "missing_evidence",
                    "A requires endpoint error and stats".into(),
                ));
            }
        }
        ExperimentId::B => {
            if exp.endpoint_abs_error.is_none() {
                issues.push(issue(
                    "missing_evidence",
                    "B requires endpoint/phase evidence".into(),
                ));
            }
        }
        ExperimentId::C => {
            if exp.component_errors.is_empty() {
                issues.push(issue(
                    "missing_evidence",
                    "C requires component errors".into(),
                ));
            }
        }
        ExperimentId::D => {
            let dense_ok = if candidate == CANDIDATE_ODE_SOLVERS {
                exp.dense_assessment.is_some()
            } else if candidate == CANDIDATE_IVP {
                exp.dense_assessment.is_some()
                    && !exp.accepted_step_probes.is_empty()
                    && exp.accepted_step_probes.iter().all(|p| {
                        !p.computed.is_empty()
                            && !p.analytic.is_empty()
                            && p.max_abs_error.is_finite()
                            && p.max_rel_error.is_finite()
                    })
            } else {
                exp.dense_assessment.is_some() && !exp.accepted_step_probes.is_empty()
            };
            if !dense_ok {
                issues.push(issue(
                    "missing_evidence",
                    if candidate == CANDIDATE_ODE_SOLVERS {
                        "D requires dense assessment (accepted_step_probes may be empty for ode-solvers)"
                            .into()
                    } else {
                        "D requires dense assessment and non-empty accepted-step probes with measured errors"
                            .into()
                    },
                ));
            }
        }
        ExperimentId::E => match (&exp.root_localization, &exp.solver_stop, &exp.restart) {
            (None, _, _) | (_, None, _) | (_, _, None) => {
                issues.push(issue(
                    "missing_evidence",
                    "E requires root localization, solver stop, and restart evidence".into(),
                ));
            }
            (_, Some(stop), Some(restart)) if candidate == CANDIDATE_IVP && exp.passed => {
                if !stop.interrupted {
                    issues.push(issue(
                        "missing_evidence",
                        "ivp E requires solver_stop.interrupted=true when passed".into(),
                    ));
                }
                if !stop.adapter_matches_localized {
                    issues.push(issue(
                        "missing_evidence",
                        "ivp E requires adapter_returned == localized root".into(),
                    ));
                }
                if stop.localized_event_time.is_none() || stop.localized_event_state.is_none() {
                    issues.push(issue(
                        "missing_evidence",
                        "ivp E requires localized_event_time/state distinct from raw".into(),
                    ));
                }
                if !restart.deterministic {
                    issues.push(issue(
                        "missing_evidence",
                        "ivp E requires restart.deterministic=true when passed".into(),
                    ));
                }
            }
            _ => {}
        },
        ExperimentId::F => match (&exp.step_guard, &exp.callback_stop, &exp.domain_error) {
            (None, _, _) | (_, None, _) | (_, _, None) => {
                issues.push(issue(
                    "missing_evidence",
                    "F requires step guard, callback stop, and domain error evidence".into(),
                ));
            }
            (Some(_), Some(_), Some(_)) => issues.extend(validate_f_domain(exp)),
        },
        ExperimentId::G => {
            if exp.stats.is_none() {
                issues.push(issue(
                    "missing_evidence",
                    "G requires integration stats".into(),
                ));
            }
        }
    }
    issues
}

fn validate_f_domain(exp: &ExperimentResult) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let cb = exp.callback_stop.as_ref().unwrap();
    if !(cb.callback_invoked && cb.interrupt_requested && cb.interrupted) {
        issues.push(issue(
            "f_callback_incomplete",
            "F callback-stop evidence incomplete".into(),
        ));
    }
    let d = exp.domain_error.as_ref().unwrap();
    if !d.typed_error_recovered {
        issues.push(issue(
            "f_domain_not_recovered",
            "typed_error_recovered=false".into(),
        ));
    }
    if d.latched_error_code.is_empty() {
        issues.push(issue(
            "f_domain_code_empty",
            "latched_error_code empty".into(),
        ));
    }
    if d.caller_error_variant != "Domain" {
        issues.push(issue(
            "f_caller_not_domain",
            format!(
                "caller_error_variant={} (expected Domain)",
                d.caller_error_variant
            ),
        ));
    }
    if d.nan_presented_as_public_error {
        issues.push(issue(
            "f_nan_public_error",
            "NaN presented as public error".into(),
        ));
    }
    if !d.non_finite_nominal_rejected {
        issues.push(issue(
            "f_non_finite_accepted",
            "nominal non-finite success was not rejected".into(),
        ));
    }
    if let Some(g) = exp.step_guard.as_ref() {
        if matches!(
            g.typed_domain_failure,
            SupportLevel::Supported | SupportLevel::SupportedWithAdapter
        ) && !d.typed_error_recovered
        {
            issues.push(issue(
                "f_typed_claim_without_recovery",
                "typed-domain support claimed while typed_error_recovered=false".into(),
            ));
        }
    }
    issues
}

fn validate_decision_matrix(report: &CandidateReport) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let d = find_exp(report, ExperimentId::D);
    let e = find_exp(report, ExperimentId::E);
    let f = find_exp(report, ExperimentId::F);

    let dense_level = matrix_level(report, "accepted_step_dense_interpolation");
    let event_level = matrix_level(report, "event_localization_fit");
    let stop_level = matrix_level(report, "stop_restart_semantics");

    if report.candidate == "ivp" {
        if dense_level == SupportLevel::Supported {
            let probes_ok = d.is_some_and(|x| {
                x.passed
                    && x.accepted_step_probes
                        .iter()
                        .all(|p| p.max_abs_error < 1e-6)
            });
            if !probes_ok {
                issues.push(issue(
                    "matrix_contradiction",
                    "accepted_step_dense_interpolation=Supported but D probes missing/failing"
                        .into(),
                ));
            }
        }
        if event_level == SupportLevel::Supported || stop_level == SupportLevel::Supported {
            let e_ok = e.is_some_and(|x| {
                x.passed
                    && x.root_localization.is_some()
                    && x.solver_stop
                        .as_ref()
                        .is_some_and(|s| s.interrupted && s.adapter_matches_localized)
                    && x.restart.as_ref().is_some_and(|r| r.deterministic)
            });
            if !e_ok {
                issues.push(issue(
                    "matrix_contradiction",
                    "event/stop Supported but E adapter lifecycle evidence incomplete".into(),
                ));
            }
        }
    }

    if report.candidate == "ode-solvers" {
        if dense_level == SupportLevel::Supported
            || dense_level == SupportLevel::SupportedWithAdapter
        {
            let has_true_interp = d
                .and_then(|x| x.dense_assessment.as_ref())
                .is_some_and(|a| {
                    a.classes_observed
                        .iter()
                        .any(|c| matches!(c, DenseOutputClass::AcceptedStepInterpolant))
                });
            if has_true_interp {
                issues.push(issue(
                    "matrix_contradiction",
                    "ode_solvers must not claim AcceptedStepInterpolant".into(),
                ));
            }
        }
        if event_level == SupportLevel::Supported
            || event_level == SupportLevel::SupportedWithAdapter
        {
            issues.push(issue(
                "matrix_contradiction",
                "ode_solvers event_localization_fit must be Unsupported for preferred architecture"
                    .into(),
            ));
        }
    }

    if f.is_some_and(|x| x.passed) {
        let cb_ok = f
            .and_then(|x| x.callback_stop.as_ref())
            .is_some_and(|c| c.callback_invoked && c.interrupt_requested && c.interrupted);
        if !cb_ok {
            issues.push(issue(
                "matrix_contradiction",
                "F passed but callback stop evidence incomplete".into(),
            ));
        }
    }

    issues
}

fn matrix_level(report: &CandidateReport, req: &str) -> SupportLevel {
    report
        .decision_matrix
        .iter()
        .find(|r| r.requirement == req)
        .map(|r| r.level)
        .unwrap_or(SupportLevel::Unverified)
}

fn find_exp(report: &CandidateReport, id: ExperimentId) -> Option<&ExperimentResult> {
    report.experiments.iter().find(|e| e.id == id)
}

fn issue(code: &str, detail: String) -> ValidationIssue {
    ValidationIssue {
        code: code.into(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{
        CallbackStopEvidence, ContractRequirementScore, DependencyAudit, DomainErrorEvidence,
        ErrorScalingAssessment, ExperimentResult, StepGuardAssessment, SPIKE_VERSION,
    };

    fn minimal_report(experiments: Vec<ExperimentResult>) -> CandidateReport {
        CandidateReport {
            schema_version: SPIKE_VERSION.into(),
            candidate: "test".into(),
            crate_version: "0".into(),
            commit: "test".into(),
            toolchain: "test".into(),
            target: "test".into(),
            experiments,
            experiments_expected: 7,
            experiments_run: 7,
            unexplained_skips: 0,
            error_scaling: ErrorScalingAssessment {
                norm_type: String::new(),
                dimension_dependent: false,
                absolute_relative_formula: String::new(),
                zero_component_behavior: String::new(),
                position_momentum_notes: String::new(),
                scaling_visible_or_configurable: false,
                state_rescaling_changes_dense_semantics: false,
                direct_vector_tolerance: SupportLevel::Unverified,
                adapter_scaled_tolerance: SupportLevel::Unverified,
            },
            dependency_audit: DependencyAudit {
                crate_name: String::new(),
                exact_version: String::new(),
                package_id: String::new(),
                checksum: String::new(),
                source: String::new(),
                license: String::new(),
                source_repo: String::new(),
                source_tag_or_rev: String::new(),
                direct_unsafe_occurrences: vec![],
                build_scripts: vec![],
                proc_macro_crates: vec![],
                native_dependencies: vec![],
                cargo_tree_digest: String::new(),
                audit_commands: vec![],
                maintenance_notes: String::new(),
                transitive_risk_notes: String::new(),
            },
            decision_matrix: vec![],
            report_digest: String::new(),
        }
    }

    fn stub_exp(id: ExperimentId, passed: bool) -> ExperimentResult {
        ExperimentResult {
            id,
            passed,
            detail: String::new(),
            endpoint_abs_error: None,
            endpoint_rel_error: None,
            component_errors: vec![],
            dense_probes: vec![],
            accepted_step_probes: vec![],
            stats: None,
            determinism: None,
            dense_assessment: None,
            step_guard: None,
            root_localization: None,
            solver_stop: None,
            restart: None,
            callback_stop: None,
            domain_error: None,
            error_scaling: None,
        }
    }

    fn f_exp_with_domain(domain: DomainErrorEvidence, passed: bool) -> ExperimentResult {
        let mut e = stub_exp(ExperimentId::F, passed);
        e.callback_stop = Some(CallbackStopEvidence {
            callback_invoked: true,
            interrupt_requested: true,
            interrupted: true,
            stop_time: 0.1,
            stop_state: vec![1.0],
            accepted_steps_before_stop: 1,
            accepted_steps_after_stop: 0,
            deterministic: true,
        });
        e.step_guard = Some(StepGuardAssessment {
            static_h_max: SupportLevel::Supported,
            dynamic_h_max: SupportLevel::Unsupported,
            pre_rhs_domain_reject: SupportLevel::Unsupported,
            post_accepted_step_stop: SupportLevel::Supported,
            stop_from_callback: SupportLevel::Supported,
            bracket_recovery: SupportLevel::Unsupported,
            typed_domain_failure: SupportLevel::Supported,
            notes: String::new(),
        });
        e.domain_error = Some(domain);
        e
    }

    #[test]
    fn one_failed_experiment_fails_gate() {
        let exps: Vec<_> = ALL_EXPERIMENT_IDS
            .iter()
            .map(|&id| stub_exp(id, id != ExperimentId::A))
            .collect();
        let report = minimal_report(exps);
        let err = validate_candidate_report(&report).unwrap_err();
        assert!(err.iter().any(|i| i.code == "experiment_failed"));
    }

    #[test]
    fn missing_experiment_fails_gate() {
        let report = minimal_report(vec![stub_exp(ExperimentId::A, true)]);
        let err = validate_candidate_report(&report).unwrap_err();
        assert!(err.iter().any(|i| i.code == "missing_experiment"));
    }

    #[test]
    fn duplicate_experiment_fails_gate() {
        let mut exps: Vec<_> = ALL_EXPERIMENT_IDS
            .iter()
            .map(|&id| stub_exp(id, true))
            .collect();
        exps.push(stub_exp(ExperimentId::A, true));
        let report = minimal_report(exps);
        let err = validate_candidate_report(&report).unwrap_err();
        assert!(err.iter().any(|i| i.code == "duplicate_experiment"));
    }

    #[test]
    fn contradictory_matrix_fails_gate() {
        let exps: Vec<_> = ALL_EXPERIMENT_IDS
            .iter()
            .map(|&id| stub_exp(id, true))
            .collect();
        let mut report = minimal_report(exps);
        report.candidate = "ode-solvers".into();
        report.decision_matrix = vec![ContractRequirementScore {
            requirement: "event_localization_fit".into(),
            level: SupportLevel::SupportedWithAdapter,
            evidence: String::new(),
        }];
        let err = validate_candidate_report(&report).unwrap_err();
        assert!(err.iter().any(|i| i.code == "matrix_contradiction"));
    }

    #[test]
    fn latched_domain_without_typed_caller_fails() {
        let mut exps: Vec<_> = ALL_EXPERIMENT_IDS
            .iter()
            .map(|&id| stub_exp(id, true))
            .collect();
        exps[5] = f_exp_with_domain(
            DomainErrorEvidence {
                latched_error_code: "DOMAIN_X_EXCEEDED".into(),
                caller_error_variant: String::new(),
                typed_error_recovered: false,
                solver_panicked: false,
                raw_solver_status: "Err".into(),
                raw_result_non_finite: true,
                nan_presented_as_public_error: false,
                non_finite_nominal_rejected: true,
            },
            true,
        );
        let err = validate_candidate_report(&minimal_report(exps)).unwrap_err();
        assert!(err
            .iter()
            .any(|i| i.code == "f_caller_not_domain" || i.code == "f_domain_not_recovered"));
    }

    #[test]
    fn typed_claim_without_recovery_fails() {
        let mut exps: Vec<_> = ALL_EXPERIMENT_IDS
            .iter()
            .map(|&id| stub_exp(id, true))
            .collect();
        exps[5] = f_exp_with_domain(
            DomainErrorEvidence {
                latched_error_code: "DOMAIN_X_EXCEEDED".into(),
                caller_error_variant: "Solver".into(),
                typed_error_recovered: false,
                solver_panicked: false,
                raw_solver_status: "Err".into(),
                raw_result_non_finite: true,
                nan_presented_as_public_error: false,
                non_finite_nominal_rejected: true,
            },
            true,
        );
        let err = validate_candidate_report(&minimal_report(exps)).unwrap_err();
        assert!(err.iter().any(|i| {
            i.code == "f_typed_claim_without_recovery" || i.code == "f_domain_not_recovered"
        }));
    }

    #[test]
    fn non_finite_nominal_not_rejected_fails() {
        let mut exps: Vec<_> = ALL_EXPERIMENT_IDS
            .iter()
            .map(|&id| stub_exp(id, true))
            .collect();
        exps[5] = f_exp_with_domain(
            DomainErrorEvidence {
                latched_error_code: "DOMAIN_X_EXCEEDED".into(),
                caller_error_variant: "Domain".into(),
                typed_error_recovered: true,
                solver_panicked: false,
                raw_solver_status: "latched".into(),
                raw_result_non_finite: true,
                nan_presented_as_public_error: false,
                non_finite_nominal_rejected: false,
            },
            true,
        );
        let err = validate_candidate_report(&minimal_report(exps)).unwrap_err();
        assert!(err.iter().any(|i| i.code == "f_non_finite_accepted"));
    }
}
