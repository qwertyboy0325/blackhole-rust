//! Canonical trace-data digest (independent of shading / PPM).

use sha2::{Digest, Sha256};

use crate::camera::pixel_index;
use crate::diagnostics::hex_sha;
use crate::outcome::{OutcomeClass, RayOutcome};
use crate::trace::TraceBundle;
use relativity_integrate::{
    DiskCrossingSide, EventId, GeodesicState, IntegrationError, LocalizationTermination,
    SurfaceApproachReason,
};

fn push_u32(hasher: &mut Sha256, v: u32) {
    hasher.update(v.to_le_bytes());
}

fn push_u64(hasher: &mut Sha256, v: u64) {
    hasher.update(v.to_le_bytes());
}

fn push_f64_bits(hasher: &mut Sha256, v: f64) {
    // Canonical IEEE-754 bit pattern (including NaN payloads if ever present).
    hasher.update(v.to_bits().to_le_bytes());
}

fn push_str(hasher: &mut Sha256, s: &str) {
    push_u64(hasher, s.len() as u64);
    hasher.update(s.as_bytes());
}

fn push_state(hasher: &mut Sha256, state: &GeodesicState) {
    for c in state.to_array() {
        push_f64_bits(hasher, c);
    }
}

fn push_stats(hasher: &mut Sha256, accepted: u64, rejected: u64, rhs: u64) {
    push_u64(hasher, accepted);
    push_u64(hasher, rejected);
    push_u64(hasher, rhs);
}

fn push_crossing(hasher: &mut Sha256, side: DiskCrossingSide) {
    push_str(
        hasher,
        match side {
            DiskCrossingSide::UpperToLower => "UpperToLower",
            DiskCrossingSide::LowerToUpper => "LowerToUpper",
            DiskCrossingSide::ExactEndpoint => "ExactEndpoint",
        },
    );
}

fn push_event_id(hasher: &mut Sha256, id: EventId) {
    push_str(
        hasher,
        match id {
            EventId::OuterHorizon => "OuterHorizon",
            EventId::EscapeSphere => "EscapeSphere",
            EventId::ThinDisk => "ThinDisk",
        },
    );
}

fn push_localization_term(hasher: &mut Sha256, t: LocalizationTermination) {
    push_str(
        hasher,
        match t {
            LocalizationTermination::ExactEndpoint => "ExactEndpoint",
            LocalizationTermination::EventValueTolerance => "EventValueTolerance",
            LocalizationTermination::AffineWidthTolerance => "AffineWidthTolerance",
        },
    );
}

fn push_approach_reason(hasher: &mut Sha256, r: SurfaceApproachReason) {
    push_str(
        hasher,
        match r {
            SurfaceApproachReason::AcceptedEndpointWithinTolerance => {
                "AcceptedEndpointWithinTolerance"
            }
            SurfaceApproachReason::SolverStepSizeTooSmall => "SolverStepSizeTooSmall",
        },
    );
}

fn push_error(hasher: &mut Sha256, err: &IntegrationError) {
    push_str(
        hasher,
        match err {
            IntegrationError::InvalidConfig { .. } => "InvalidConfig",
            IntegrationError::PhysicsDomain { .. } => "PhysicsDomain",
            IntegrationError::EventDomain { .. } => "EventDomain",
            IntegrationError::NonFiniteState { .. } => "NonFiniteState",
            IntegrationError::Solver { .. } => "Solver",
            IntegrationError::StepLimitExceeded { .. } => "StepLimitExceeded",
            IntegrationError::MissingEventOutcome => "MissingEventOutcome",
            IntegrationError::InvalidInterpolantBounds => "InvalidInterpolantBounds",
            IntegrationError::EventLocalizationDidNotConverge { .. } => {
                "EventLocalizationDidNotConverge"
            }
        },
    );
    match err {
        IntegrationError::InvalidConfig { field } => push_str(hasher, field),
        IntegrationError::PhysicsDomain { source } => push_str(hasher, &source.to_string()),
        IntegrationError::EventDomain { event_id, detail } => {
            push_event_id(hasher, *event_id);
            push_str(hasher, detail);
        }
        IntegrationError::NonFiniteState { stage } => {
            push_str(hasher, &format!("{stage:?}"));
        }
        IntegrationError::Solver { detail } => push_str(hasher, detail),
        IntegrationError::StepLimitExceeded { accepted_steps } => {
            push_u64(hasher, *accepted_steps);
        }
        IntegrationError::EventLocalizationDidNotConverge {
            event_id,
            iterations,
            residual,
            bracket_width,
        } => {
            push_event_id(hasher, *event_id);
            push_u64(hasher, *iterations);
            push_f64_bits(hasher, *residual);
            push_f64_bits(hasher, *bracket_width);
        }
        IntegrationError::MissingEventOutcome | IntegrationError::InvalidInterpolantBounds => {}
    }
}

fn push_outcome(hasher: &mut Sha256, outcome: &RayOutcome) {
    push_str(
        hasher,
        match outcome.class() {
            OutcomeClass::DiskHit => "DiskHit",
            OutcomeClass::Escaped => "Escaped",
            OutcomeClass::HorizonEvent => "HorizonEvent",
            OutcomeClass::HorizonApproach => "HorizonApproach",
            OutcomeClass::AffineLimit => "AffineLimit",
            OutcomeClass::Failed => "Failed",
        },
    );
    match outcome {
        RayOutcome::DiskHit(h) => {
            push_f64_bits(hasher, h.lambda.0);
            push_state(hasher, &h.state);
            push_stats(
                hasher,
                h.integration.accepted_steps,
                h.integration.rejected_steps,
                h.integration.rhs_evaluations,
            );
            push_f64_bits(hasher, h.oblate_radius);
            push_crossing(hasher, h.crossing_side);
            push_f64_bits(hasher, h.event_value);
            push_u64(hasher, h.localization.interpolation_calls);
            push_f64_bits(hasher, h.localization.final_bracket_width);
            push_u64(hasher, h.localization.iterations);
            push_localization_term(hasher, h.localization.termination);
        }
        RayOutcome::Escaped(h) => {
            push_f64_bits(hasher, h.lambda.0);
            push_state(hasher, &h.state);
            push_stats(
                hasher,
                h.integration.accepted_steps,
                h.integration.rejected_steps,
                h.integration.rhs_evaluations,
            );
            push_f64_bits(hasher, h.event_value);
        }
        RayOutcome::HorizonEvent(h) => {
            push_event_id(hasher, h.event_id);
            push_f64_bits(hasher, h.lambda.0);
            push_state(hasher, &h.state);
            push_stats(
                hasher,
                h.integration.accepted_steps,
                h.integration.rejected_steps,
                h.integration.rhs_evaluations,
            );
            push_f64_bits(hasher, h.event_value);
            push_u64(hasher, h.localization.interpolation_calls);
            push_f64_bits(hasher, h.localization.final_bracket_width);
            push_u64(hasher, h.localization.iterations);
            push_localization_term(hasher, h.localization.termination);
        }
        RayOutcome::HorizonApproach(h) => {
            push_event_id(hasher, h.event_id);
            push_f64_bits(hasher, h.lambda.0);
            push_state(hasher, &h.state);
            push_stats(
                hasher,
                h.integration.accepted_steps,
                h.integration.rejected_steps,
                h.integration.rhs_evaluations,
            );
            push_f64_bits(hasher, h.signed_event_value);
            push_f64_bits(hasher, h.approach_tolerance);
            push_approach_reason(hasher, h.reason);
        }
        RayOutcome::AffineLimit(h) => {
            push_f64_bits(hasher, h.lambda.0);
            push_state(hasher, &h.state);
            push_stats(
                hasher,
                h.integration.accepted_steps,
                h.integration.rejected_steps,
                h.integration.rhs_evaluations,
            );
        }
        RayOutcome::Failed(f) => {
            push_error(hasher, &f.error);
        }
    }
}

/// Deterministic digest of row-major trace results (excludes all shading / RGB / PPM).
///
/// Floating values use `f64::to_bits()`. Successful non-finite states are forbidden
/// by the tracing pipeline; if a NaN bit pattern were present it is hashed as-is.
pub fn trace_data_digest(bundle: &TraceBundle) -> String {
    let mut hasher = Sha256::new();
    push_u32(&mut hasher, bundle.grid.width);
    push_u32(&mut hasher, bundle.grid.height);
    for row in 0..bundle.grid.height {
        for col in 0..bundle.grid.width {
            push_u32(&mut hasher, col);
            push_u32(&mut hasher, row);
            push_u64(&mut hasher, pixel_index(bundle.grid, col, row) as u64);
            push_outcome(&mut hasher, bundle.outcome_at(col, row));
        }
    }
    hex_sha(&hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::TraceGrid;
    use crate::shade::{shade_diagnostic, DiagnosticShadeStyle};
    use relativity_core::{Covector, PositionKs};
    use relativity_integrate::{
        AffineParameter, IntegrationStats, InvariantDiagnostics, RawSolverStop,
    };

    fn one_escape_bundle() -> TraceBundle {
        let state = GeodesicState::new(
            PositionKs::new(0.0, 10.0, 0.0, 0.0),
            Covector::new(-1.0, 0.0, 0.0, 0.0),
        )
        .unwrap();
        let integration = IntegrationStats {
            accepted_steps: 3,
            rejected_steps: 1,
            rhs_evaluations: 20,
            callback_count: 3,
        };
        let diagnostics = InvariantDiagnostics {
            h_initial: 0.0,
            h_final: 0.0,
            h_max_abs_residual: 0.0,
            p_t_initial: -1.0,
            p_t_final: -1.0,
            p_t_max_abs_drift: 0.0,
            non_finite_checks: 0,
            raw_vs_localized_lambda_separation: None,
            relative_tolerance: [1e-8; 8],
            absolute_tolerance: [1e-9; 8],
        };
        TraceBundle {
            grid: TraceGrid {
                width: 1,
                height: 1,
            },
            outcomes: vec![RayOutcome::Escaped(crate::outcome::EscapeHit {
                lambda: AffineParameter(2.0),
                state,
                raw_solver_stop: RawSolverStop {
                    lambda: AffineParameter(2.0),
                    state,
                },
                integration,
                diagnostics,
                event_value: 1.0,
            })],
        }
    }

    #[test]
    fn shading_does_not_change_trace_data_digest() {
        let bundle = one_escape_bundle();
        let d0 = trace_data_digest(&bundle);
        let _ = shade_diagnostic(&bundle, DiagnosticShadeStyle::Gate1b2Categorical);
        let _ = shade_diagnostic(&bundle, DiagnosticShadeStyle::DiskSuppressed);
        assert_eq!(d0, trace_data_digest(&bundle));
    }

    #[test]
    fn terminal_state_change_alters_digest() {
        let a = one_escape_bundle();
        let mut b = one_escape_bundle();
        if let RayOutcome::Escaped(h) = &mut b.outcomes[0] {
            h.lambda = AffineParameter(3.0);
        }
        assert_ne!(trace_data_digest(&a), trace_data_digest(&b));
    }

    #[test]
    fn uses_bit_patterns_not_formatted_strings() {
        let mut a = one_escape_bundle();
        let mut b = one_escape_bundle();
        if let RayOutcome::Escaped(h) = &mut a.outcomes[0] {
            h.event_value = 0.0;
        }
        if let RayOutcome::Escaped(h) = &mut b.outcomes[0] {
            h.event_value = -0.0;
        }
        assert_ne!((0.0f64).to_bits(), (-0.0f64).to_bits());
        assert_ne!(trace_data_digest(&a), trace_data_digest(&b));
    }
}
