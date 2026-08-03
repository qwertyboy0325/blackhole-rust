//! Shared JSON schema for Gate 1B0 spike reports.

use serde::{Deserialize, Serialize};

pub const CANDIDATE_ODE_SOLVERS: &str = "ode-solvers";
pub const CANDIDATE_IVP: &str = "ivp";
pub const SPIKE_VERSION: &str = "gate-1b0-v3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExperimentId {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

pub const ALL_EXPERIMENT_IDS: [ExperimentId; 7] = [
    ExperimentId::A,
    ExperimentId::B,
    ExperimentId::C,
    ExperimentId::D,
    ExperimentId::E,
    ExperimentId::F,
    ExperimentId::G,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupportLevel {
    Supported,
    SupportedWithAdapter,
    Unsupported,
    Unverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DenseOutputClass {
    AcceptedStepInterpolant,
    GlobalSolutionQuery,
    PredeterminedSamples,
    ExternalReconstruction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CallbackTiming {
    AfterAcceptedStep,
    AfterStage,
    AfterIntegration,
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentError {
    pub index: usize,
    pub abs: f64,
    pub rel: f64,
    pub analytic: f64,
    pub computed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStats {
    pub accepted_steps: u32,
    pub rejected_steps: u32,
    pub rhs_evaluations: u32,
    pub final_step_size: f64,
    pub min_step_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenseProbe {
    pub theta: f64,
    pub t: f64,
    pub abs_error: f64,
    pub rel_error: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedStepProbe {
    pub step_x0: f64,
    pub step_x1: f64,
    pub theta: f64,
    pub t: f64,
    pub computed: Vec<f64>,
    pub analytic: Vec<f64>,
    pub max_abs_error: f64,
    pub max_rel_error: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminismRecord {
    pub in_process_runs: u32,
    pub signatures: Vec<String>,
    pub endpoint_bits: Vec<String>,
    pub accepted_steps: Vec<u32>,
    pub json_digests: Vec<String>,
    pub deterministic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorScalingAssessment {
    pub norm_type: String,
    pub dimension_dependent: bool,
    pub absolute_relative_formula: String,
    pub zero_component_behavior: String,
    pub position_momentum_notes: String,
    pub scaling_visible_or_configurable: bool,
    pub state_rescaling_changes_dense_semantics: bool,
    pub direct_vector_tolerance: SupportLevel,
    pub adapter_scaled_tolerance: SupportLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenseOutputAssessment {
    pub classes_observed: Vec<DenseOutputClass>,
    pub callback_timing: CallbackTiming,
    pub can_stop_from_callback: bool,
    pub stats_at_callback: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepGuardAssessment {
    pub static_h_max: SupportLevel,
    pub dynamic_h_max: SupportLevel,
    pub pre_rhs_domain_reject: SupportLevel,
    pub post_accepted_step_stop: SupportLevel,
    pub stop_from_callback: SupportLevel,
    pub bracket_recovery: SupportLevel,
    pub typed_domain_failure: SupportLevel,
    pub notes: String,
}

/// Pure root localization on a supplied interpolant — no solver lifecycle claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootLocalizationEvidence {
    pub event_time_analytic: f64,
    pub event_time_found: f64,
    pub time_error: f64,
    pub root_residual: f64,
    pub state_error: f64,
    pub interpolation_calls: u32,
    pub localized_state: Vec<f64>,
    /// True when a shallow *sign-changing* crossing was exercised (not tangent).
    pub shallow_sign_changing_crossing_tested: bool,
    /// Reserved: true only if a no-endpoint-sign-change / tangent case was exercised.
    pub tangent_no_sign_change_tested: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverStopEvidence {
    pub interrupted: bool,
    /// Accepted-step endpoint at Interrupt (raw solver).
    pub raw_solver_stop_time: f64,
    pub raw_solver_stop_state: Vec<f64>,
    /// Localized root from StepInterpolant (adapter computation).
    pub localized_event_time: Option<f64>,
    pub localized_event_state: Option<Vec<f64>>,
    /// Values returned to the caller by the adapter contract.
    pub adapter_returned_time: f64,
    pub adapter_returned_state: Vec<f64>,
    /// adapter_returned == localized (preferred Event contract).
    pub adapter_matches_localized: bool,
    pub callback_count_at_stop: u32,
    pub accepted_steps_at_stop: u32,
    pub rejected_steps_at_stop: u32,
    pub rhs_evaluations_at_stop: u32,
    pub no_steps_after_stop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartEvidence {
    pub restart_time: f64,
    pub restart_state: Vec<f64>,
    pub restart_endpoint: Vec<f64>,
    pub reference_endpoint: Vec<f64>,
    pub endpoint_error: f64,
    pub deterministic: bool,
    pub endpoint_bits: Vec<String>,
    pub in_process_runs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallbackStopEvidence {
    pub callback_invoked: bool,
    pub interrupt_requested: bool,
    pub interrupted: bool,
    pub stop_time: f64,
    pub stop_state: Vec<f64>,
    pub accepted_steps_before_stop: u32,
    pub accepted_steps_after_stop: u32,
    pub deterministic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainErrorEvidence {
    pub latched_error_code: String,
    /// Pattern-matchable caller variant: "Domain" | "NonFiniteResult" | "Solver" | "".
    pub caller_error_variant: String,
    pub typed_error_recovered: bool,
    pub solver_panicked: bool,
    pub raw_solver_status: String,
    pub raw_result_non_finite: bool,
    pub nan_presented_as_public_error: bool,
    /// Separate probe: nominal success with non-finite state → NonFiniteResult.
    pub non_finite_nominal_rejected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub id: ExperimentId,
    pub passed: bool,
    pub detail: String,
    pub endpoint_abs_error: Option<f64>,
    pub endpoint_rel_error: Option<f64>,
    pub component_errors: Vec<ComponentError>,
    pub dense_probes: Vec<DenseProbe>,
    pub accepted_step_probes: Vec<AcceptedStepProbe>,
    pub stats: Option<IntegrationStats>,
    pub determinism: Option<DeterminismRecord>,
    pub dense_assessment: Option<DenseOutputAssessment>,
    pub step_guard: Option<StepGuardAssessment>,
    pub root_localization: Option<RootLocalizationEvidence>,
    pub solver_stop: Option<SolverStopEvidence>,
    pub restart: Option<RestartEvidence>,
    pub callback_stop: Option<CallbackStopEvidence>,
    pub domain_error: Option<DomainErrorEvidence>,
    pub error_scaling: Option<ErrorScalingAssessment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsafeOccurrence {
    pub file: String,
    pub line: u32,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAudit {
    pub crate_name: String,
    pub exact_version: String,
    pub package_id: String,
    pub checksum: String,
    pub source: String,
    pub license: String,
    pub source_repo: String,
    pub source_tag_or_rev: String,
    pub direct_unsafe_occurrences: Vec<UnsafeOccurrence>,
    pub build_scripts: Vec<String>,
    pub proc_macro_crates: Vec<String>,
    pub native_dependencies: Vec<String>,
    pub cargo_tree_digest: String,
    pub audit_commands: Vec<String>,
    pub maintenance_notes: String,
    pub transitive_risk_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractRequirementScore {
    pub requirement: String,
    pub level: SupportLevel,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateReport {
    pub schema_version: String,
    pub candidate: String,
    pub crate_version: String,
    pub commit: String,
    pub toolchain: String,
    pub target: String,
    pub experiments: Vec<ExperimentResult>,
    pub experiments_expected: u32,
    pub experiments_run: u32,
    pub unexplained_skips: u32,
    pub error_scaling: ErrorScalingAssessment,
    pub dependency_audit: DependencyAudit,
    pub decision_matrix: Vec<ContractRequirementScore>,
    pub report_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub schema_version: String,
    pub commit: String,
    pub toolchain: String,
    pub target: String,
    pub candidates: Vec<String>,
    pub comparison_digest: String,
    pub requirement_comparison: Vec<RequirementComparisonRow>,
    pub adr_recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementComparisonRow {
    pub requirement: String,
    pub ode_solvers: SupportLevel,
    pub ivp: SupportLevel,
    pub notes: String,
}

pub const DECISION_REQUIREMENTS: &[&str] = &[
    "mathematical_method_dop853_f64",
    "eight_component_state",
    "vector_tolerance_direct",
    "accepted_step_callback",
    "accepted_step_dense_interpolation",
    "event_localization_fit",
    "stop_restart_semantics",
    "step_guard_control",
    "integration_statistics",
    "determinism_same_platform",
    "error_propagation",
    "adapter_complexity_acceptable",
    "dependency_risk",
    "maintenance_risk",
];
