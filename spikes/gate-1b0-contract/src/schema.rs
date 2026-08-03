//! Shared JSON schema for Gate 1B0 spike reports.

use serde::{Deserialize, Serialize};

pub const CANDIDATE_ODE_SOLVERS: &str = "ode-solvers";
pub const CANDIDATE_IVP: &str = "ivp";
pub const SPIKE_VERSION: &str = "gate-1b0-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// True current accepted-step interpolant / coefficients exposed.
    AcceptedStepInterpolant,
    /// Query completed global solution (e.g. sol(t) after integrate).
    GlobalSolutionQuery,
    /// Samples at predetermined output times only.
    PredeterminedSamples,
    /// External reconstruction from stage values (not used unless proven).
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
pub struct DeterminismRecord {
    pub in_process_runs: u32,
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
    pub stop_from_callback: SupportLevel,
    pub bracket_recovery: SupportLevel,
    pub typed_domain_failure: SupportLevel,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEvidence {
    pub event_time_analytic: f64,
    pub event_time_found: f64,
    pub time_error: f64,
    pub root_residual: f64,
    pub state_error: f64,
    pub interpolation_calls: u32,
    pub stopped_at_event: bool,
    pub restart_deterministic: bool,
    pub shallow_crossing_tested: bool,
    pub shallow_sign_change_only_insufficient: bool,
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
    pub stats: Option<IntegrationStats>,
    pub determinism: Option<DeterminismRecord>,
    pub dense_assessment: Option<DenseOutputAssessment>,
    pub step_guard: Option<StepGuardAssessment>,
    pub event_evidence: Option<EventEvidence>,
    pub error_scaling: Option<ErrorScalingAssessment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyAudit {
    pub crate_name: String,
    pub exact_version: String,
    pub license: String,
    pub source_repo: String,
    pub source_tag_or_rev: String,
    pub direct_unsafe_in_crate: bool,
    pub build_scripts: Vec<String>,
    pub native_dependencies: Vec<String>,
    pub cargo_tree_digest: String,
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
