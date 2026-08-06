//! Gate 2B1 bolometric radiance evaluator.

use crate::build_meta::{
    is_optimized_release_execution, read_build_execution_report, require_release_execution,
    BuildExecutionMetadata,
};
use crate::render_lensed_celestial::LensedCelestialReport;
use crate::render_tier::{DiagnosticRenderTier, RenderAuthorityClass};
use crate::trace_outcome_map::read_trace_execution_report;
use relativity_core::{EquatorialAngularDirection, FrequencyShift};
use relativity_render::{
    bolometric_debug_display_v1, build_disk_bolometric_frame, canonical_g_fourth,
    diagnostic_bolometric_emission_spec_digest, diagnostic_bolometric_emission_v1,
    disk_bolometric_digest, procedural_coordinate_grid_v1, procedural_texture_spec_digest,
    sample_diagnostic_bolometric_emission, shade_observed_bolometric_debug,
    transport_bolometric_specific_intensity, BolometricSpecificIntensity, DiskBolometricConvention,
    DiskFrequencyShiftConvention, DiskFrequencyShiftFrame, DiskFrequencyShiftPixel,
    DiskFrequencyShiftSample, DiskVelocityModel, LensedCelestialMode, ObserverFrequencySource,
    ResolvedDiskBounds, CANONICAL_DISK_EMISSION_CLAIM, CANONICAL_DISK_EMISSION_MODEL,
    DISK_BOUNDS_SOURCE_V1, TEXTURE_ID_V1,
};
use relativity_trace::{hex_sha, OutcomeCounts, TraceGrid, TraceSurfaceSet};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

const REF_CLASS: &str = "64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4";
const REF_PPM: &str = "ac058d5af01b425e411b5c33017210bf888aa52918cfd085bb863d7ddc99184c";
const REF_PGM: &str = "2df226390057bb87b64d172cd258087b0ef4c1ad0ce0d4378e003b5861a75db5";
const REF_COUNTS: OutcomeCounts = OutcomeCounts {
    disk_hit: 12307,
    escaped: 2442,
    horizon_event: 1462,
    horizon_approach: 173,
    affine_limit: 0,
    failed: 0,
};
const REF_NUMERICAL_PROFILE: &str =
    "af0041d388c61576e18a400a4f35a4220bd4981d34a05a42dacb6e77d97e888b";
const REF_COORD: &str = "5d8df5ba007beeb3742ef9c3a684dbd86704f6b9a29271356e87d07fc2c71328";
const REF_TEXTURE_SPEC: &str = "6b06bf21a607510a981c5ec7d2521e4d4d9beccb7d5354d29dbbb1520edf495a";
const REF_OPAQUE_LENSED: &str = "e4cb10b98e97793ddbf365edc1bdf29fde32e70afc7b05604275bc78a335de0a";
const REF_FREQ: &str = "65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2";
const REF_FREQ_JSON: &str = "a2f440e76bc0f89c539e7dcb7ab76171a3dc84d67a26185871fe8678c9ed7106";
const REF_G_PPM: &str = "30b6cf872056fdfa59021bd58bbad15a0cf24a234f31fe80cfc5bc0cfbc0fb6f";
const APPROVED_BASE: &str = "0d0c2fc6627120f285bdf393d90b973df654a523";
const OBS_TOL: f64 = 1e-10;
const TRANSPORT_RESIDUAL_TOL: f64 = 1e-15;

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct Gate2b1Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    texture_spec_digest: String,
    emission_spec_digest: String,
    checks: Vec<Check>,
    smoke_thread_1: Option<LensedCelestialReport>,
    smoke_thread_bounded: Option<LensedCelestialReport>,
    gate_runs: Vec<LensedCelestialReport>,
    gate_2b0_compat: Option<LensedCelestialReport>,
    gate_2a2_compat: Option<LensedCelestialReport>,
    content_digest_excluding_digest_field: String,
}

pub fn evaluate() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let build = BuildExecutionMetadata::current();
    let (dirty, dirty_detail) = porcelain_dirty(&root)?;
    let commit = git_stdout(&root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());

    let mut checks = Vec::new();
    push(
        &mut checks,
        "worktree_clean",
        !dirty,
        if dirty {
            format!("non-authoritative dirty worktree: {dirty_detail}")
        } else {
            "clean".into()
        },
    );
    let self_release = is_optimized_release_execution();
    push(
        &mut checks,
        "evaluator_release_build",
        self_release,
        build.describe(),
    );
    if !self_release {
        let mut report = empty(&build, commit.trim(), dirty, dirty_detail, checks);
        finalize(&root, &mut report)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Err("gate-2b1-bolometric-radiance requires release evaluator".into());
    }
    require_release_execution(&build)?;

    let ancestor_ok = Command::new("git")
        .current_dir(&root)
        .args(["merge-base", "--is-ancestor", APPROVED_BASE, "HEAD"])
        .status()?
        .success();
    push(
        &mut checks,
        "descends_from_approved_base",
        ancestor_ok,
        APPROVED_BASE.into(),
    );

    run_check(
        &mut checks,
        "fmt",
        Command::new("cargo")
            .current_dir(&root)
            .args(["fmt", "--all", "--", "--check"]),
    )?;
    run_check(
        &mut checks,
        "clippy",
        Command::new("cargo").current_dir(&root).args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ]),
    )?;
    run_check(
        &mut checks,
        "tests",
        Command::new("cargo")
            .current_dir(&root)
            .args(["test", "--workspace", "--all-features"]),
    )?;

    check_algebraic_corpus(&mut checks);

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;
    let smoke_threads = available.clamp(1, 2);

    let out_root = root.join("artifacts/gate-2b1-bolometric-radiance");
    std::fs::create_dir_all(&out_root)?;

    let texture_spec = procedural_coordinate_grid_v1();
    let texture_spec_digest = procedural_texture_spec_digest(&texture_spec);
    push(
        &mut checks,
        "gate_2a2_texture_spec_identity",
        texture_spec_digest == REF_TEXTURE_SPEC,
        texture_spec_digest.clone(),
    );

    let emission_spec = diagnostic_bolometric_emission_v1();
    let emission_spec_digest = diagnostic_bolometric_emission_spec_digest(&emission_spec);
    push(
        &mut checks,
        "emission_spec_canonical",
        emission_spec.validate().is_ok(),
        emission_spec_digest.clone(),
    );

    // CLI negatives: reject before artifacts.
    check_cli_negative(
        &root,
        &mut checks,
        "cli_reject_bolo_without_frequency",
        TraceSurfaceSet::OpaqueDiskHorizonEscape,
        LensedCelestialMode::OpaqueDiskMask,
        "artifacts/gate-2b1-bolometric-radiance/cli-neg-bolo-without-freq",
        false,
        true,
        None,
    )?;
    check_cli_negative(
        &root,
        &mut checks,
        "cli_reject_horizon_escape_disk_omitted",
        TraceSurfaceSet::HorizonEscapeOnly,
        LensedCelestialMode::DiskOmittedDiagnostic,
        "artifacts/gate-2b1-bolometric-radiance/cli-neg-horizon-escape",
        true,
        true,
        None,
    )?;
    check_cli_negative(
        &root,
        &mut checks,
        "cli_reject_altered_emission_claim",
        TraceSurfaceSet::OpaqueDiskHorizonEscape,
        LensedCelestialMode::OpaqueDiskMask,
        "artifacts/gate-2b1-bolometric-radiance/cli-neg-altered-claim",
        true,
        true,
        Some(PresetMutation::AlteredClaim),
    )?;
    check_cli_negative(
        &root,
        &mut checks,
        "cli_reject_unsupported_emission_model",
        TraceSurfaceSet::OpaqueDiskHorizonEscape,
        LensedCelestialMode::OpaqueDiskMask,
        "artifacts/gate-2b1-bolometric-radiance/cli-neg-unsupported-model",
        true,
        true,
        Some(PresetMutation::UnsupportedModel),
    )?;

    let smoke_thread_1 = run_worker(
        &root,
        DiagnosticRenderTier::Smoke,
        "artifacts/gate-2b1-bolometric-radiance/smoke-thread-1",
        1,
        true,
        true,
    )?;
    check_bolo_worker(&mut checks, "smoke1", &smoke_thread_1, false)?;

    let smoke_thread_bounded = run_worker(
        &root,
        DiagnosticRenderTier::Smoke,
        "artifacts/gate-2b1-bolometric-radiance/smoke-thread-bounded",
        smoke_threads,
        true,
        true,
    )?;
    check_bolo_worker(&mut checks, "smoke_bounded", &smoke_thread_bounded, false)?;

    let s1 = smoke_thread_1
        .disk_bolometric_radiance
        .as_ref()
        .expect("smoke1 bolo");
    let sb = smoke_thread_bounded
        .disk_bolometric_radiance
        .as_ref()
        .expect("smoke bounded bolo");
    push(
        &mut checks,
        "smoke_thread_count_bolometric_digest_identical",
        s1.bolometric_digest == sb.bolometric_digest
            && s1.bolometric_json_digest == sb.bolometric_json_digest
            && s1.emitted_debug_ppm_digest == sb.emitted_debug_ppm_digest
            && s1.observed_debug_ppm_digest == sb.observed_debug_ppm_digest
            && s1.composite_ppm_digest == sb.composite_ppm_digest,
        s1.bolometric_digest.clone(),
    );
    push(
        &mut checks,
        "smoke_thread_count_artifacts_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-1",
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-bounded",
            "disk-bolometric-radiance-map.json",
        )? && files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-1",
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-bounded",
            "emitted-bolometric-debug.ppm",
        )? && files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-1",
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-bounded",
            "observed-bolometric-debug.ppm",
        )? && files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-1",
            "artifacts/gate-2b1-bolometric-radiance/smoke-thread-bounded",
            "bolometric-disk-celestial-composite.ppm",
        )?,
        "json+3ppm".into(),
    );

    let mut gate_runs = Vec::new();
    for i in 0..2 {
        gate_runs.push(run_worker(
            &root,
            DiagnosticRenderTier::Gate,
            &format!("artifacts/gate-2b1-bolometric-radiance/gate-run-{i}"),
            authoritative_threads,
            true,
            true,
        )?);
    }
    check_bolo_worker(&mut checks, "gate0", &gate_runs[0], true)?;
    check_bolo_worker(&mut checks, "gate1", &gate_runs[1], true)?;

    let g0 = &gate_runs[0];
    let g1 = &gate_runs[1];
    let b0 = g0.disk_bolometric_radiance.as_ref().expect("gate0 bolo");
    let b1 = g1.disk_bolometric_radiance.as_ref().expect("gate1 bolo");
    let f0 = g0.disk_frequency_shift.as_ref().expect("gate0 freq");
    let f1 = g1.disk_frequency_shift.as_ref().expect("gate1 freq");

    push(
        &mut checks,
        "gate_workers_scientific_digest_identical",
        b0.bolometric_digest == b1.bolometric_digest
            && f0.frequency_shift_digest == f1.frequency_shift_digest,
        b0.bolometric_digest.clone(),
    );
    push(
        &mut checks,
        "gate_workers_json_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/gate-run-0",
            "artifacts/gate-2b1-bolometric-radiance/gate-run-1",
            "disk-bolometric-radiance-map.json",
        )? && files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/gate-run-0",
            "artifacts/gate-2b1-bolometric-radiance/gate-run-1",
            "disk-frequency-shift-map.json",
        )?,
        b0.bolometric_json_digest.clone(),
    );
    push(
        &mut checks,
        "gate_workers_ppm_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/gate-run-0",
            "artifacts/gate-2b1-bolometric-radiance/gate-run-1",
            "emitted-bolometric-debug.ppm",
        )? && files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/gate-run-0",
            "artifacts/gate-2b1-bolometric-radiance/gate-run-1",
            "observed-bolometric-debug.ppm",
        )? && files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/gate-run-0",
            "artifacts/gate-2b1-bolometric-radiance/gate-run-1",
            "bolometric-disk-celestial-composite.ppm",
        )? && files_eq(
            &root,
            "artifacts/gate-2b1-bolometric-radiance/gate-run-0",
            "artifacts/gate-2b1-bolometric-radiance/gate-run-1",
            "g-factor-debug.ppm",
        )?,
        b0.composite_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_regression_corpus_identical",
        b0.regression_corpus == b1.regression_corpus
            && f0.regression_corpus == f1.regression_corpus,
        format!(
            "bolo_n={} freq_n={}",
            b0.regression_corpus.len(),
            f0.regression_corpus.len()
        ),
    );

    // Gate 1B2 / 2A1 / 2A2 / 2B0 identity on bolometric-enabled gate run.
    push(
        &mut checks,
        "numerical_profile_matches_2a0",
        g0.numerical_profile_digest == REF_NUMERICAL_PROFILE,
        g0.numerical_profile_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_1b2_class",
        g0.outcome_class_digest == REF_CLASS,
        g0.outcome_class_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_1b2_categorical_ppm",
        g0.categorical_ppm_digest == REF_PPM,
        g0.categorical_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_1b2_rhs_pgm",
        g0.rhs_pgm_digest == REF_PGM,
        g0.rhs_pgm_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_1b2_counts",
        counts_eq(&g0.outcome_counts, &REF_COUNTS) && g0.outcome_counts.failed == 0,
        format!("{:?}", g0.outcome_counts),
    );
    push(
        &mut checks,
        "opaque_gate_2a1_coordinate_digest",
        g0.coordinate_digest == REF_COORD,
        g0.coordinate_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_2a2_lensed_ppm",
        g0.lensed_ppm_digest == REF_OPAQUE_LENSED,
        g0.lensed_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_2b0_frequency_digests",
        f0.frequency_shift_digest == REF_FREQ
            && f0.frequency_shift_json_digest == REF_FREQ_JSON
            && f0.g_factor_debug_ppm_digest == REF_G_PPM,
        f0.frequency_shift_digest.clone(),
    );
    push(
        &mut checks,
        "bolometric_accounting",
        b0.attenuated_count == 8293
            && b0.boosted_count == 4014
            && b0.unchanged_count == 0
            && b0.disk_hit_count == 12307
            && b0.mapped_count == 12307
            && b0.mapping_failure_count == 0
            && b0.resolved_disk_bounds.inner_radius() == 3.0
            && b0.resolved_disk_bounds.outer_radius() == 20.0
            && b0.disk_bounds_source == DISK_BOUNDS_SOURCE_V1
            && b0.accepted_emission_model == CANONICAL_DISK_EMISSION_MODEL
            && b0.accepted_emission_claim == CANONICAL_DISK_EMISSION_CLAIM
            && b0.bolometric_emission_passes == 1
            && b0.bolometric_transport_passes == 1
            && b0.bolometric_visualization_passes == 3
            && f0.observer_frequency_verification_passes == 1
            && f0.frequency_shift_passes == 1
            && g0.trace_invocations == 1
            && g0.coordinate_passes == 1
            && g0.texture_render_passes == 1,
        format!(
            "att={} boost={} unch={} disk={} bounds=({},{}) src={} model={} claim={} bolo_passes={}/{}/{}",
            b0.attenuated_count,
            b0.boosted_count,
            b0.unchanged_count,
            b0.disk_hit_count,
            b0.resolved_disk_bounds.inner_radius(),
            b0.resolved_disk_bounds.outer_radius(),
            b0.disk_bounds_source,
            b0.accepted_emission_model,
            b0.accepted_emission_claim,
            b0.bolometric_emission_passes,
            b0.bolometric_transport_passes,
            b0.bolometric_visualization_passes
        ),
    );
    push(
        &mut checks,
        "transport_residual",
        b0.maximum_abs_transport_residual <= TRANSPORT_RESIDUAL_TOL,
        format!("{}", b0.maximum_abs_transport_residual),
    );
    push(
        &mut checks,
        "observer_unit_frequency_residual",
        f0.maximum_observer_unit_frequency_residual <= OBS_TOL,
        format!("{}", f0.maximum_observer_unit_frequency_residual),
    );
    push(
        &mut checks,
        "convention_canonical_v1",
        bolo_convention_ok(&b0.convention)
            && freq_convention_ok(&f0.convention)
            && f0.velocity_model == DiskVelocityModel::ProgradeCircularGeodesic
            && f0.resolved_direction == EquatorialAngularDirection::PositivePhi
            && b0.emission_spec_digest == emission_spec_digest
            && b0.source_frequency_shift_digest == f0.frequency_shift_digest,
        b0.convention.convention_id.clone(),
    );

    // Gate 2B0 compatibility: frequency only, no bolometric field.
    let gate_2b0_compat = run_worker(
        &root,
        DiagnosticRenderTier::Gate,
        "artifacts/gate-2b1-bolometric-radiance/gate-2b0-compat",
        authoritative_threads,
        true,
        false,
    )?;
    push(
        &mut checks,
        "gate_2b0_compat_no_bolometric_field",
        gate_2b0_compat.disk_bolometric_radiance.is_none()
            && gate_2b0_compat.disk_frequency_shift.is_some(),
        "bolo omitted; freq present".into(),
    );
    if let Some(freq) = &gate_2b0_compat.disk_frequency_shift {
        push(
            &mut checks,
            "gate_2b0_compat_frequency_digest",
            freq.frequency_shift_digest == REF_FREQ
                && freq.frequency_shift_json_digest == REF_FREQ_JSON
                && freq.g_factor_debug_ppm_digest == REF_G_PPM,
            freq.frequency_shift_digest.clone(),
        );
    } else {
        push(
            &mut checks,
            "gate_2b0_compat_frequency_digest",
            false,
            "missing disk_frequency_shift".into(),
        );
    }
    push(
        &mut checks,
        "gate_2b0_compat_1b2_class",
        gate_2b0_compat.outcome_class_digest == REF_CLASS
            && gate_2b0_compat.categorical_ppm_digest == REF_PPM
            && gate_2b0_compat.rhs_pgm_digest == REF_PGM
            && counts_eq(&gate_2b0_compat.outcome_counts, &REF_COUNTS),
        gate_2b0_compat.outcome_class_digest.clone(),
    );

    // Gate 2A2 compatibility without frequency/bolometric flags.
    let gate_2a2_compat = run_worker(
        &root,
        DiagnosticRenderTier::Gate,
        "artifacts/gate-2b1-bolometric-radiance/gate-2a2-compat",
        authoritative_threads,
        false,
        false,
    )?;
    push(
        &mut checks,
        "gate_2a2_compat_no_frequency_field",
        gate_2a2_compat.disk_frequency_shift.is_none()
            && gate_2a2_compat.disk_bolometric_radiance.is_none(),
        "optional fields omitted".into(),
    );
    push(
        &mut checks,
        "gate_2a2_compat_lensed_ppm",
        gate_2a2_compat.lensed_ppm_digest == REF_OPAQUE_LENSED,
        gate_2a2_compat.lensed_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_2a2_compat_1b2_class",
        gate_2a2_compat.outcome_class_digest == REF_CLASS
            && gate_2a2_compat.categorical_ppm_digest == REF_PPM
            && gate_2a2_compat.rhs_pgm_digest == REF_PGM
            && counts_eq(&gate_2a2_compat.outcome_counts, &REF_COUNTS),
        gate_2a2_compat.outcome_class_digest.clone(),
    );
    push(
        &mut checks,
        "gate_2a2_compat_coordinate",
        gate_2a2_compat.coordinate_digest == REF_COORD,
        gate_2a2_compat.coordinate_digest.clone(),
    );
    push(
        &mut checks,
        "no_spectral_rgb_claims",
        no_forbidden_spectral_claims(&root)?,
        "bolometric g⁴ only; no spectra/RGB".into(),
    );

    let hard_fail = checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    let gate_ok = g0.render_tier == Some(DiagnosticRenderTier::Gate)
        && g1.render_tier == Some(DiagnosticRenderTier::Gate);
    let authoritative = !dirty && !hard_fail && self_release && gate_ok && ancestor_ok;
    let result = if hard_fail {
        "FAIL"
    } else if authoritative {
        "PASS"
    } else {
        "PASS_NON_AUTHORITATIVE"
    };

    let mut report = Gate2b1Eval {
        gate: "gate-2b1-bolometric-radiance".into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads: available,
        authoritative_threads,
        texture_spec_digest: texture_spec_digest.clone(),
        emission_spec_digest: emission_spec_digest.clone(),
        checks,
        smoke_thread_1: Some(smoke_thread_1),
        smoke_thread_bounded: Some(smoke_thread_bounded),
        gate_runs,
        gate_2b0_compat: Some(gate_2b0_compat),
        gate_2a2_compat: Some(gate_2a2_compat),
        content_digest_excluding_digest_field: String::new(),
    };
    let digest = eval_digest(&report);
    report.content_digest_excluding_digest_field = digest.clone();
    report.checks.push(Check {
        name: "artifact_digest_convention".into(),
        status: "PASS",
        detail: format!("digest={digest}"),
    });
    let hard_fail = report
        .checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    report.authoritative = !dirty
        && !hard_fail
        && report.build.is_optimized_release_execution()
        && gate_ok
        && ancestor_ok;
    report.result = if hard_fail {
        "FAIL".into()
    } else if report.authoritative {
        "PASS".into()
    } else {
        "PASS_NON_AUTHORITATIVE".into()
    };
    let mut for_hash = report.clone();
    for_hash.content_digest_excluding_digest_field.clear();
    report.content_digest_excluding_digest_field = eval_digest(&for_hash);

    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if hard_fail || report.result == "FAIL" {
        return Err("gate-2b1-bolometric-radiance evaluation FAIL".into());
    }
    Ok(())
}

fn bolo_convention_ok(c: &DiskBolometricConvention) -> bool {
    c == &DiskBolometricConvention::v1()
}

fn freq_convention_ok(c: &DiskFrequencyShiftConvention) -> bool {
    c.schema_version == 1
        && c.convention_id == "backward-covector-circular-disk-g-factor-v1"
        && c.photon_orientation == "stored-past-directed-covector"
        && c.measured_frequency_definition
            == "p-backward-covector-contract-future-timelike-velocity"
        && c.observer_frequency_source == ObserverFrequencySource::CameraLocalUnitPastNull
        && c.disk_velocity_model == DiskVelocityModel::ProgradeCircularGeodesic
        && c.equatorial_policy == "localized-radius-equatorial-surface-canonicalization-v1"
        && c.ratio_definition == "observer-frequency-over-emitter-frequency"
}

fn check_algebraic_corpus(checks: &mut Vec<Check>) {
    let spec = diagnostic_bolometric_emission_v1();
    let bounds = ResolvedDiskBounds::new(3.0, 20.0).expect("bounds");

    let i_in = sample_diagnostic_bolometric_emission(&spec, bounds, 3.0)
        .expect("I_em(3)")
        .value();
    let i_out = sample_diagnostic_bolometric_emission(&spec, bounds, 20.0)
        .expect("I_em(20)")
        .value();
    push(
        checks,
        "algebraic_emission_bounds",
        (i_in - 1.0).abs() < 1e-15 && (i_out - 0.003375).abs() < 1e-15,
        format!("I(3)={i_in} I(20)={i_out}"),
    );

    let mut mono_ok = true;
    let mut prev = f64::INFINITY;
    for r in [3.0_f64, 4.0, 6.0, 10.0, 20.0] {
        let i = sample_diagnostic_bolometric_emission(&spec, bounds, r)
            .expect("mono sample")
            .value();
        if !(i < prev) {
            mono_ok = false;
        }
        prev = i;
    }
    push(
        checks,
        "algebraic_emission_monotonic",
        mono_ok,
        "r=3,4,6,10,20".into(),
    );

    let em = BolometricSpecificIntensity::new(2.0).expect("em");
    let (f1, o1) =
        transport_bolometric_specific_intensity(em, FrequencyShift::new(1.0).unwrap()).unwrap();
    let (f2, o2) =
        transport_bolometric_specific_intensity(em, FrequencyShift::new(2.0).unwrap()).unwrap();
    let (f3, o3) =
        transport_bolometric_specific_intensity(em, FrequencyShift::new(0.5).unwrap()).unwrap();
    push(
        checks,
        "algebraic_g_fourth_transport",
        f1.value() == 1.0
            && o1.value() == 2.0
            && f2.value() == 16.0
            && o2.value() == 32.0
            && f3.value() == 0.0625
            && o3.value() == 0.125
            && canonical_g_fourth(1.0) == 1.0
            && canonical_g_fourth(2.0) == 16.0
            && canonical_g_fourth(0.5) == 0.0625,
        "g=1→1; g=2→16; g=0.5→0.0625".into(),
    );

    // Display independence: scientific digest unchanged by visualization.
    let grid = TraceGrid {
        width: 1,
        height: 1,
    };
    let fs = DiskFrequencyShiftSample {
        velocity_model: DiskVelocityModel::ProgradeCircularGeodesic,
        resolved_direction: EquatorialAngularDirection::PositivePhi,
        observer_frequency_source: ObserverFrequencySource::CameraLocalUnitPastNull,
        radius: 6.0,
        azimuth: 0.1,
        angular_velocity_bl: 0.05,
        emitter_four_velocity_bl: [1.0, 0.0, 0.0, 0.05],
        observer_frequency: 1.0,
        emitter_frequency: 1.0,
        g_factor: 1.0,
        log2_g: 0.0,
        disk_event_value: 1e-12,
        disk_radius_residual: 0.0,
    };
    let frame = DiskFrequencyShiftFrame::try_new(grid, vec![DiskFrequencyShiftPixel::DiskHit(fs)])
        .expect("fs frame");
    let bolo = build_disk_bolometric_frame(&frame, &spec, bounds).expect("bolo frame");
    let d_before = disk_bolometric_digest(
        &bolo,
        &DiskBolometricConvention::v1(),
        &spec,
        bounds,
        "synthetic-src",
        CANONICAL_DISK_EMISSION_MODEL,
        CANONICAL_DISK_EMISSION_CLAIM,
    )
    .expect("digest before");
    let display = bolometric_debug_display_v1();
    let _ = shade_observed_bolometric_debug(&bolo, &display).expect("shade");
    let d_after = disk_bolometric_digest(
        &bolo,
        &DiskBolometricConvention::v1(),
        &spec,
        bounds,
        "synthetic-src",
        CANONICAL_DISK_EMISSION_MODEL,
        CANONICAL_DISK_EMISSION_CLAIM,
    )
    .expect("digest after");
    push(
        checks,
        "algebraic_display_independence",
        d_before == d_after,
        d_before,
    );
}

#[derive(Clone, Copy)]
enum PresetMutation {
    AlteredClaim,
    UnsupportedModel,
}

#[allow(clippy::too_many_arguments)]
fn check_cli_negative(
    root: &Path,
    checks: &mut Vec<Check>,
    name: &str,
    surface_set: TraceSurfaceSet,
    mode: LensedCelestialMode,
    output_dir: &str,
    emit_freq: bool,
    emit_bolo: bool,
    mutation: Option<PresetMutation>,
) -> Result<(), Box<dyn std::error::Error>> {
    let abs = root.join(output_dir);
    let _ = std::fs::remove_dir_all(&abs);
    let mut preset_path = "presets/gargantua-baseline.toml".to_string();
    let mut temp_preset: Option<PathBuf> = None;
    if let Some(m) = mutation {
        let base = std::fs::read_to_string(root.join("presets/gargantua-baseline.toml"))?;
        let mutated = match m {
            PresetMutation::AlteredClaim => base.replace(
                CANONICAL_DISK_EMISSION_CLAIM,
                "altered claim incompatible with project diagnostic",
            ),
            PresetMutation::UnsupportedModel => base.replace(
                CANONICAL_DISK_EMISSION_MODEL,
                "unsupported_emission_model_x",
            ),
        };
        if mutated == base {
            return Err(format!("preset mutation {name} did not change file").into());
        }
        let tmp = root.join(format!(
            "artifacts/gate-2b1-bolometric-radiance/{name}-preset.toml"
        ));
        if let Some(parent) = tmp.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&tmp, mutated)?;
        preset_path = tmp
            .strip_prefix(root)
            .unwrap_or(&tmp)
            .to_string_lossy()
            .into_owned();
        temp_preset = Some(tmp);
    }
    let mut args = vec![
        "run",
        "--release",
        "-q",
        "-p",
        "xtask",
        "--",
        "render-lensed-celestial",
        "--preset",
        preset_path.as_str(),
        "--tier",
        "smoke",
        "--surface-set",
        surface_set.as_str(),
        "--mode",
        mode.as_str(),
        "--texture",
        TEXTURE_ID_V1,
        "--output-dir",
        output_dir,
        "--execution",
        "serial",
        "--require-release",
    ];
    if emit_freq {
        args.push("--emit-disk-frequency-shift");
    }
    if emit_bolo {
        args.push("--emit-disk-bolometric-radiance");
    }
    let out = Command::new("cargo")
        .current_dir(root)
        .args(&args)
        .output()?;
    let rejected = !out.status.success();
    let no_artifacts = !abs.exists()
        || (!abs.join("disk-bolometric-radiance-map.json").exists()
            && !abs.join("disk-frequency-shift-map.json").exists()
            && !abs.join("lensed-celestial-report.json").exists());
    if let Some(tmp) = temp_preset {
        let _ = std::fs::remove_file(tmp);
    }
    push(
        checks,
        name,
        rejected && no_artifacts,
        format!(
            "rejected={rejected} no_artifacts={no_artifacts} stderr={}",
            String::from_utf8_lossy(&out.stderr)
        ),
    );
    Ok(())
}

fn check_bolo_worker(
    checks: &mut Vec<Check>,
    label: &str,
    report: &LensedCelestialReport,
    require_gate_tier: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if require_gate_tier {
        push(
            checks,
            &format!("{label}_tier_gate"),
            report.render_tier == Some(DiagnosticRenderTier::Gate)
                && report.width == 128
                && report.height == 128
                && report.authority_class == RenderAuthorityClass::AuthoritativeCandidate,
            format!("{}x{}", report.width, report.height),
        );
    }
    push(
        checks,
        &format!("{label}_surface_mode"),
        report.surface_set == TraceSurfaceSet::OpaqueDiskHorizonEscape
            && report.mode == LensedCelestialMode::OpaqueDiskMask,
        format!("{:?} {:?}", report.surface_set, report.mode),
    );
    let Some(freq) = report.disk_frequency_shift.as_ref() else {
        push(
            checks,
            &format!("{label}_frequency_present"),
            false,
            "missing disk_frequency_shift".into(),
        );
        return Ok(());
    };
    let Some(bolo) = report.disk_bolometric_radiance.as_ref() else {
        push(
            checks,
            &format!("{label}_bolometric_present"),
            false,
            "missing disk_bolometric_radiance".into(),
        );
        return Ok(());
    };
    push(
        checks,
        &format!("{label}_pass_counts"),
        report.trace_invocations == 1
            && freq.observer_frequency_verification_passes == 1
            && freq.frequency_shift_passes == 1
            && bolo.bolometric_emission_passes == 1
            && bolo.bolometric_transport_passes == 1
            && bolo.bolometric_visualization_passes == 3
            && report.coordinate_passes == 1
            && report.texture_render_passes == 1,
        format!(
            "trace={} ver={} fs={} bolo={}/{}/{}",
            report.trace_invocations,
            freq.observer_frequency_verification_passes,
            freq.frequency_shift_passes,
            bolo.bolometric_emission_passes,
            bolo.bolometric_transport_passes,
            bolo.bolometric_visualization_passes
        ),
    );
    push(
        checks,
        &format!("{label}_mapping_complete"),
        freq.mapped_count == freq.disk_hit_count
            && freq.mapping_failure_count == 0
            && bolo.mapped_count == bolo.disk_hit_count
            && bolo.mapping_failure_count == 0
            && bolo.disk_hit_count == freq.disk_hit_count,
        format!(
            "freq_mapped={} bolo_mapped={} disk={}",
            freq.mapped_count, bolo.mapped_count, bolo.disk_hit_count
        ),
    );
    push(
        checks,
        &format!("{label}_observer_residual"),
        freq.maximum_observer_unit_frequency_residual <= OBS_TOL,
        format!("{}", freq.maximum_observer_unit_frequency_residual),
    );
    push(
        checks,
        &format!("{label}_transport_residual"),
        bolo.maximum_abs_transport_residual <= TRANSPORT_RESIDUAL_TOL,
        format!("{}", bolo.maximum_abs_transport_residual),
    );
    Ok(())
}

fn run_worker(
    root: &Path,
    tier: DiagnosticRenderTier,
    output_dir: &str,
    threads: usize,
    emit_freq: bool,
    emit_bolo: bool,
) -> Result<LensedCelestialReport, Box<dyn std::error::Error>> {
    let mut args = vec![
        "run",
        "--release",
        "-q",
        "-p",
        "xtask",
        "--",
        "render-lensed-celestial",
        "--preset",
        "presets/gargantua-baseline.toml",
        "--tier",
        tier.as_str(),
        "--surface-set",
        "opaque-disk-horizon-escape",
        "--mode",
        "opaque-disk-mask",
        "--texture",
        TEXTURE_ID_V1,
        "--output-dir",
        output_dir,
        "--execution",
        "parallel",
        "--threads",
        "", // placeholder
        "--require-release",
    ];
    let threads_s = threads.to_string();
    let thread_idx = args.iter().position(|a| *a == "--threads").unwrap() + 1;
    args[thread_idx] = threads_s.as_str();
    if emit_freq {
        args.push("--emit-disk-frequency-shift");
    }
    if emit_bolo {
        args.push("--emit-disk-bolometric-radiance");
    }
    let out = Command::new("cargo")
        .current_dir(root)
        .args(&args)
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "render-lensed-celestial failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    let dir = root.join(output_dir);
    let report: LensedCelestialReport =
        serde_json::from_slice(&std::fs::read(dir.join("lensed-celestial-report.json"))?)?;
    let build = read_build_execution_report(&dir)?;
    let exec = read_trace_execution_report(&dir)?;
    if build != report.build {
        return Err("build-execution.json disagrees with report.build".into());
    }
    if exec != report.execution {
        return Err("trace-execution.json disagrees with report.execution".into());
    }
    Ok(report)
}

fn counts_eq(a: &OutcomeCounts, b: &OutcomeCounts) -> bool {
    a.disk_hit == b.disk_hit
        && a.escaped == b.escaped
        && a.horizon_event == b.horizon_event
        && a.horizon_approach == b.horizon_approach
        && a.affine_limit == b.affine_limit
        && a.failed == b.failed
}

fn files_eq(
    root: &Path,
    dir_a: &str,
    dir_b: &str,
    name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let a = std::fs::read(root.join(dir_a).join(name))?;
    let b = std::fs::read(root.join(dir_b).join(name))?;
    Ok(a == b)
}

fn no_forbidden_spectral_claims(root: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let bolo = std::fs::read_to_string(root.join("crates/relativity-render/src/bolometric.rs"))?;
    let worker = std::fs::read_to_string(root.join("xtask/src/render_lensed_celestial.rs"))?;
    // Strip line comments so explicit absences in docs do not trip the scan.
    let blob = strip_line_comments(&format!("{bolo}\n{worker}")).to_lowercase();
    // g⁴ / g^4 are allowed (bolometric transport). g³ / g^3 are not.
    let forbidden = [
        "openexr",
        "acescg",
        "blackbody",
        "novikov",
        "wgpu",
        "egui",
        "g^3",
        "g³",
    ];
    Ok(forbidden.iter().all(|f| !blob.contains(f)))
}

fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                ""
            } else if let Some(idx) = line.find("//") {
                &line[..idx]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2b1Eval {
    Gate2b1Eval {
        gate: "gate-2b1-bolometric-radiance".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        texture_spec_digest: String::new(),
        emission_spec_digest: String::new(),
        checks,
        smoke_thread_1: None,
        smoke_thread_bounded: None,
        gate_runs: vec![],
        gate_2b0_compat: None,
        gate_2a2_compat: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut Gate2b1Eval) -> Result<(), Box<dyn std::error::Error>> {
    if report.content_digest_excluding_digest_field.is_empty() {
        let mut h = report.clone();
        h.content_digest_excluding_digest_field.clear();
        report.content_digest_excluding_digest_field = eval_digest(&h);
    }
    let dir = root.join("artifacts/gate-2b1-bolometric-radiance");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# Gate 2B1 bolometric-radiance evaluation\n\n");
    md.push_str(&format!("- result: `{}`\n", report.result));
    md.push_str(&format!("- authoritative: `{}`\n", report.authoritative));
    md.push_str(&format!("- commit: `{}`\n", report.commit));
    md.push_str(&format!(
        "- digest: `{}`\n",
        report.content_digest_excluding_digest_field
    ));
    md.push_str(&format!(
        "- emission_spec_digest: `{}`\n",
        report.emission_spec_digest
    ));
    if let Some(g) = report.gate_runs.first() {
        if let Some(b) = &g.disk_bolometric_radiance {
            md.push_str("\n## Gate bolometric summary\n");
            md.push_str(&format!("- bolometric_digest: `{}`\n", b.bolometric_digest));
            md.push_str(&format!(
                "- bolometric_json_digest: `{}`\n",
                b.bolometric_json_digest
            ));
            md.push_str(&format!(
                "- emitted_debug_ppm_digest: `{}`\n",
                b.emitted_debug_ppm_digest
            ));
            md.push_str(&format!(
                "- observed_debug_ppm_digest: `{}`\n",
                b.observed_debug_ppm_digest
            ));
            md.push_str(&format!(
                "- composite_ppm_digest: `{}`\n",
                b.composite_ppm_digest
            ));
            md.push_str(&format!(
                "- disk_hit/mapped/fail: {}/{}/{}\n",
                b.disk_hit_count, b.mapped_count, b.mapping_failure_count
            ));
            md.push_str(&format!(
                "- attenuated/boosted/unchanged: {}/{}/{}\n",
                b.attenuated_count, b.boosted_count, b.unchanged_count
            ));
            md.push_str(&format!(
                "- bounds: ({}, {}) source=`{}`\n",
                b.resolved_disk_bounds.inner_radius(),
                b.resolved_disk_bounds.outer_radius(),
                b.disk_bounds_source
            ));
            md.push_str(&format!(
                "- accepted_emission_model: `{}`\n",
                b.accepted_emission_model
            ));
            md.push_str(&format!(
                "- accepted_emission_claim: `{}`\n",
                b.accepted_emission_claim
            ));
            md.push_str(&format!(
                "- max |transport residual|: {}\n",
                b.maximum_abs_transport_residual
            ));
        }
        if let Some(f) = &g.disk_frequency_shift {
            md.push_str("\n## Gate frequency summary\n");
            md.push_str(&format!(
                "- frequency_shift_digest: `{}`\n",
                f.frequency_shift_digest
            ));
            md.push_str(&format!(
                "- frequency_shift_json_digest: `{}`\n",
                f.frequency_shift_json_digest
            ));
            md.push_str(&format!(
                "- g_factor_debug_ppm_digest: `{}`\n",
                f.g_factor_debug_ppm_digest
            ));
        }
    }
    md.push_str("\n## Checks\n");
    for c in &report.checks {
        md.push_str(&format!("- `{}`: {} — {}\n", c.name, c.status, c.detail));
    }
    std::fs::write(dir.join("evaluation.md"), md)?;
    std::fs::write(
        dir.join("evaluation.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;
    Ok(())
}

fn eval_digest(report: &Gate2b1Eval) -> String {
    #[derive(Serialize)]
    struct DigestCheck<'a> {
        name: &'a str,
        status: &'a str,
    }
    #[derive(Serialize)]
    struct Proj<'a> {
        gate: &'a str,
        result: &'a str,
        authoritative: bool,
        commit: &'a str,
        dirty: bool,
        build: &'a BuildExecutionMetadata,
        available_threads: usize,
        authoritative_threads: usize,
        texture_spec_digest: &'a str,
        emission_spec_digest: &'a str,
        checks: Vec<DigestCheck<'a>>,
        smoke_thread_1: Option<&'a LensedCelestialReport>,
        smoke_thread_bounded: Option<&'a LensedCelestialReport>,
        gate_runs: &'a [LensedCelestialReport],
        gate_2b0_compat: Option<&'a LensedCelestialReport>,
        gate_2a2_compat: Option<&'a LensedCelestialReport>,
        content_digest_excluding_digest_field: &'a str,
    }
    let s1 = report.smoke_thread_1.as_ref().map(strip_timing);
    let sb = report.smoke_thread_bounded.as_ref().map(strip_timing);
    let gates: Vec<_> = report.gate_runs.iter().map(strip_timing).collect();
    let compat_2b0 = report.gate_2b0_compat.as_ref().map(strip_timing);
    let compat_2a2 = report.gate_2a2_compat.as_ref().map(strip_timing);
    let proj = Proj {
        gate: &report.gate,
        result: &report.result,
        authoritative: report.authoritative,
        commit: &report.commit,
        dirty: report.dirty,
        build: &report.build,
        available_threads: report.available_threads,
        authoritative_threads: report.authoritative_threads,
        texture_spec_digest: &report.texture_spec_digest,
        emission_spec_digest: &report.emission_spec_digest,
        checks: report
            .checks
            .iter()
            .map(|c| DigestCheck {
                name: &c.name,
                status: c.status,
            })
            .collect(),
        smoke_thread_1: s1.as_ref(),
        smoke_thread_bounded: sb.as_ref(),
        gate_runs: &gates,
        gate_2b0_compat: compat_2b0.as_ref(),
        gate_2a2_compat: compat_2a2.as_ref(),
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize"))
}

fn strip_timing(r: &LensedCelestialReport) -> LensedCelestialReport {
    let mut c = r.clone();
    c.trace_wall_clock_seconds = None;
    c.mapping_wall_clock_seconds = None;
    c.render_wall_clock_seconds = None;
    if let Some(f) = c.disk_frequency_shift.as_mut() {
        f.verification_wall_clock_seconds = None;
        f.mapping_wall_clock_seconds = None;
    }
    if let Some(b) = c.disk_bolometric_radiance.as_mut() {
        b.emission_wall_clock_seconds = None;
        b.transport_wall_clock_seconds = None;
        b.visualization_wall_clock_seconds = None;
    }
    c
}

fn push(checks: &mut Vec<Check>, name: &str, ok: bool, detail: String) {
    checks.push(Check {
        name: name.into(),
        status: if ok { "PASS" } else { "FAIL" },
        detail,
    });
}

fn run_check(
    checks: &mut Vec<Check>,
    name: &str,
    cmd: &mut Command,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = cmd.output()?;
    push(
        checks,
        name,
        out.status.success(),
        if out.status.success() {
            "ok".into()
        } else {
            format!("stderr={}", String::from_utf8_lossy(&out.stderr))
        },
    );
    Ok(())
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    let detail = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((!detail.is_empty(), detail))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    if !out.status.success() {
        return Err(format!("git {:?} failed", args).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}
