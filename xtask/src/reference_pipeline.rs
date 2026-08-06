use crate::diagnostic_scene::{build_diagnostic_trace_scene, DiagnosticNumericalProfile};
use crate::preset::Preset;
use relativity_oracle::{OracleChannelSet, OracleSourceDigests};
use relativity_render::{
    build_disk_bolometric_frame, build_disk_frequency_shift_frame,
    diagnostic_bolometric_emission_v1, disk_bolometric_digest, disk_frequency_shift_digest,
    validate_disk_emission_provenance, verify_disk_bolometric_frame,
    verify_observer_unit_frequency, DiskBolometricConvention, DiskBolometricFrame,
    DiskFrequencyShiftConvention, DiskFrequencyShiftFrame, DiskVelocityModel, ResolvedDiskBounds,
    CANONICAL_DISK_EMISSION_CLAIM, CANONICAL_DISK_EMISSION_MODEL,
};
use relativity_trace::{
    build_celestial_coordinate_frame, celestial_coordinate_digest, hex_sha, outcome_class_bytes,
    trace_data_digest, trace_grid_with_execution_and_surface_set, validate_celestial_seam,
    CelestialCoordinateConvention, CelestialCoordinateFrame, OutcomeCounts, RayOutcome,
    TraceBundle, TraceExecution, TraceGrid, TraceScene, TraceSurfaceSet,
};
use std::error::Error;
use std::time::Instant;

#[derive(Debug)]
pub struct ReferenceScientificFrames {
    pub scene: TraceScene,
    pub numerical_profile: DiagnosticNumericalProfile,
    pub trace: TraceBundle,
    pub celestial: CelestialCoordinateFrame,
    pub frequency: Option<DiskFrequencyShiftFrame>,
    pub bolometric: Option<DiskBolometricFrame>,
    pub source_digests: OracleSourceDigests,
    pub trace_wall_clock_seconds: f64,
    pub channel_wall_clock_seconds: f64,
    pub outcome_counts: OutcomeCounts,
}

pub fn compute_reference_scientific_frames(
    preset: &Preset,
    grid: TraceGrid,
    surface_set: TraceSurfaceSet,
    channel_set: OracleChannelSet,
    execution: TraceExecution,
) -> Result<ReferenceScientificFrames, Box<dyn Error>> {
    if channel_set == OracleChannelSet::FullBolometricDisk
        && surface_set != TraceSurfaceSet::OpaqueDiskHorizonEscape
    {
        return Err("full-bolometric-disk requires opaque-disk-horizon-escape".into());
    }
    if channel_set == OracleChannelSet::FullBolometricDisk {
        validate_disk_emission_provenance(&preset.disk.emission_model, &preset.disk.emission_claim)
            .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
    }
    validate_celestial_seam(&preset.celestial_sphere.seam)?;

    let (scene, numerical_profile) = build_diagnostic_trace_scene(preset, grid)?;
    let bounds = ResolvedDiskBounds::new(scene.disk.r_inner, scene.disk.r_outer)
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;

    let t_trace = Instant::now();
    let trace = trace_grid_with_execution_and_surface_set(&scene, execution, surface_set)?;
    let trace_wall_clock_seconds = t_trace.elapsed().as_secs_f64();

    for outcome in &trace.outcomes {
        if !matches!(outcome, RayOutcome::Failed(_)) && !outcome.state_finite() {
            return Err("non-finite success state in TraceBundle".into());
        }
    }
    let outcome_counts = summarize_outcomes(&trace);
    if outcome_counts.failed != 0 {
        return Err(format!(
            "failed ray count must be zero, got {}",
            outcome_counts.failed
        )
        .into());
    }

    let t_channel = Instant::now();
    let celestial = build_celestial_coordinate_frame(&scene.kerr, &trace)?;
    let celestial_convention = CelestialCoordinateConvention::finite_oblate_ks_boundary_uv_v1();
    let celestial_coordinate_digest =
        celestial_coordinate_digest(&celestial, &celestial_convention);

    let trace_data_digest = trace_data_digest(&trace);
    let outcome_class_digest = hex_sha(&outcome_class_bytes(&trace));

    let (frequency, frequency_shift_digest) = if channel_set == OracleChannelSet::FullBolometricDisk
    {
        let verification = verify_observer_unit_frequency(&scene.kerr, &scene)
            .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        let frequency = build_disk_frequency_shift_frame(
            &scene.kerr,
            &trace,
            DiskVelocityModel::ProgradeCircularGeodesic,
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        if verification.maximum_residual > relativity_render::OBSERVER_UNIT_FREQUENCY_TOLERANCE {
            return Err("observer unit-frequency verification exceeded tolerance".into());
        }
        let digest = disk_frequency_shift_digest(&frequency, &DiskFrequencyShiftConvention::v1());
        (Some(frequency), Some(digest))
    } else {
        (None, None)
    };

    let (bolometric, bolometric_digest) = if let Some(frequency) = &frequency {
        let emission_spec = diagnostic_bolometric_emission_v1();
        let bolometric = build_disk_bolometric_frame(frequency, &emission_spec, bounds)
            .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        verify_disk_bolometric_frame(frequency, &bolometric, &emission_spec, bounds)
            .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        let frequency_digest = frequency_shift_digest
            .as_deref()
            .ok_or("missing frequency digest for bolometric digest")?;
        let digest = disk_bolometric_digest(
            &bolometric,
            &DiskBolometricConvention::v1(),
            &emission_spec,
            bounds,
            frequency_digest,
            CANONICAL_DISK_EMISSION_MODEL,
            CANONICAL_DISK_EMISSION_CLAIM,
        )
        .map_err(|e| -> Box<dyn Error> { e.to_string().into() })?;
        (Some(bolometric), Some(digest))
    } else {
        (None, None)
    };

    let channel_wall_clock_seconds = t_channel.elapsed().as_secs_f64();
    let source_digests = OracleSourceDigests {
        numerical_profile_digest: numerical_profile.digest.clone(),
        trace_data_digest,
        outcome_class_digest,
        celestial_coordinate_digest,
        frequency_shift_digest,
        bolometric_digest,
    };

    Ok(ReferenceScientificFrames {
        scene,
        numerical_profile,
        trace,
        celestial,
        frequency,
        bolometric,
        source_digests,
        trace_wall_clock_seconds,
        channel_wall_clock_seconds,
        outcome_counts,
    })
}

pub fn summarize_outcomes(bundle: &TraceBundle) -> OutcomeCounts {
    let mut counts = OutcomeCounts {
        disk_hit: 0,
        escaped: 0,
        horizon_event: 0,
        horizon_approach: 0,
        affine_limit: 0,
        failed: 0,
    };
    for outcome in &bundle.outcomes {
        match outcome.class() {
            relativity_trace::OutcomeClass::DiskHit => counts.disk_hit += 1,
            relativity_trace::OutcomeClass::Escaped => counts.escaped += 1,
            relativity_trace::OutcomeClass::HorizonEvent => counts.horizon_event += 1,
            relativity_trace::OutcomeClass::HorizonApproach => counts.horizon_approach += 1,
            relativity_trace::OutcomeClass::AffineLimit => counts.affine_limit += 1,
            relativity_trace::OutcomeClass::Failed => counts.failed += 1,
        }
    }
    counts
}
