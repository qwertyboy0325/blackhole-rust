//! Gate 2B0 frequency-shift kinematics evaluator.

use crate::build_meta::{
    is_optimized_release_execution, read_build_execution_report, require_release_execution,
    BuildExecutionMetadata,
};
use crate::render_lensed_celestial::LensedCelestialReport;
use crate::render_tier::{DiagnosticRenderTier, RenderAuthorityClass};
use crate::trace_outcome_map::read_trace_execution_report;
use relativity_core::{
    circular_equatorial_geodesic_bl, contract_covector_vector, covector_bl_to_ks,
    frequency_shift_ratio, measured_frequency_from_backward_covector,
    measured_frequency_from_future_covector, prograde_equatorial_direction, Covector,
    EquatorialAngularDirection, KerrParams, Vector,
};
use relativity_render::{
    procedural_coordinate_grid_v1, procedural_texture_spec_digest, DiskFrequencyShiftConvention,
    DiskVelocityModel, LensedCelestialMode, ObserverFrequencySource, TEXTURE_ID_V1,
};
use relativity_trace::{hex_sha, OutcomeCounts, TraceSurfaceSet};
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
const APPROVED_BASE: &str = "33a8248c6b92e13a2c6b90187c6741e89b7fb1ab";
const OBS_TOL: f64 = 1e-10;

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct Gate2b0Eval {
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
    checks: Vec<Check>,
    smoke_thread_1: Option<LensedCelestialReport>,
    smoke_thread_bounded: Option<LensedCelestialReport>,
    gate_runs: Vec<LensedCelestialReport>,
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
        return Err("gate-2b0-frequency-shift requires release evaluator".into());
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

    let out_root = root.join("artifacts/gate-2b0-frequency-shift");
    std::fs::create_dir_all(&out_root)?;

    let texture_spec = procedural_coordinate_grid_v1();
    let texture_spec_digest = procedural_texture_spec_digest(&texture_spec);
    push(
        &mut checks,
        "gate_2a2_texture_spec_identity",
        texture_spec_digest == REF_TEXTURE_SPEC,
        texture_spec_digest.clone(),
    );

    // CLI negatives: reject before artifacts.
    check_cli_negative(
        &root,
        &mut checks,
        "cli_reject_horizon_escape_only",
        TraceSurfaceSet::HorizonEscapeOnly,
        LensedCelestialMode::OpaqueDiskMask,
        "artifacts/gate-2b0-frequency-shift/cli-neg-horizon-escape",
    )?;
    check_cli_negative(
        &root,
        &mut checks,
        "cli_reject_disk_omitted",
        TraceSurfaceSet::OpaqueDiskHorizonEscape,
        LensedCelestialMode::DiskOmittedDiagnostic,
        "artifacts/gate-2b0-frequency-shift/cli-neg-disk-omitted",
    )?;

    let smoke_thread_1 = run_worker(
        &root,
        DiagnosticRenderTier::Smoke,
        "artifacts/gate-2b0-frequency-shift/smoke-thread-1",
        1,
        true,
    )?;
    check_freq_worker(&mut checks, "smoke1", &smoke_thread_1, false)?;

    let smoke_thread_bounded = run_worker(
        &root,
        DiagnosticRenderTier::Smoke,
        "artifacts/gate-2b0-frequency-shift/smoke-thread-bounded",
        smoke_threads,
        true,
    )?;
    check_freq_worker(&mut checks, "smoke_bounded", &smoke_thread_bounded, false)?;

    let s1 = smoke_thread_1
        .disk_frequency_shift
        .as_ref()
        .expect("smoke1 freq");
    let sb = smoke_thread_bounded
        .disk_frequency_shift
        .as_ref()
        .expect("smoke bounded freq");
    push(
        &mut checks,
        "smoke_thread_count_frequency_digest_identical",
        s1.frequency_shift_digest == sb.frequency_shift_digest
            && s1.frequency_shift_json_digest == sb.frequency_shift_json_digest
            && s1.g_factor_debug_ppm_digest == sb.g_factor_debug_ppm_digest,
        s1.frequency_shift_digest.clone(),
    );
    push(
        &mut checks,
        "smoke_thread_count_artifacts_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2b0-frequency-shift/smoke-thread-1",
            "artifacts/gate-2b0-frequency-shift/smoke-thread-bounded",
            "disk-frequency-shift-map.json",
        )? && files_eq(
            &root,
            "artifacts/gate-2b0-frequency-shift/smoke-thread-1",
            "artifacts/gate-2b0-frequency-shift/smoke-thread-bounded",
            "g-factor-debug.ppm",
        )?,
        "json+ppm".into(),
    );

    let mut gate_runs = Vec::new();
    for i in 0..2 {
        gate_runs.push(run_worker(
            &root,
            DiagnosticRenderTier::Gate,
            &format!("artifacts/gate-2b0-frequency-shift/gate-run-{i}"),
            authoritative_threads,
            true,
        )?);
    }
    check_freq_worker(&mut checks, "gate0", &gate_runs[0], true)?;
    check_freq_worker(&mut checks, "gate1", &gate_runs[1], true)?;

    let g0 = &gate_runs[0];
    let g1 = &gate_runs[1];
    let f0 = g0.disk_frequency_shift.as_ref().expect("gate0 freq");
    let f1 = g1.disk_frequency_shift.as_ref().expect("gate1 freq");

    push(
        &mut checks,
        "gate_workers_scientific_digest_identical",
        f0.frequency_shift_digest == f1.frequency_shift_digest,
        f0.frequency_shift_digest.clone(),
    );
    push(
        &mut checks,
        "gate_workers_json_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2b0-frequency-shift/gate-run-0",
            "artifacts/gate-2b0-frequency-shift/gate-run-1",
            "disk-frequency-shift-map.json",
        )?,
        f0.frequency_shift_json_digest.clone(),
    );
    push(
        &mut checks,
        "gate_workers_ppm_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2b0-frequency-shift/gate-run-0",
            "artifacts/gate-2b0-frequency-shift/gate-run-1",
            "g-factor-debug.ppm",
        )?,
        f0.g_factor_debug_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_regression_corpus_identical",
        f0.regression_corpus == f1.regression_corpus,
        format!("n={}", f0.regression_corpus.len()),
    );

    // Gate 1B2 / 2A1 / 2A2 identity on frequency-enabled gate run.
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
        "frequency_accounting",
        f0.disk_hit_count == 12307
            && f0.mapped_count == 12307
            && f0.mapping_failure_count == 0
            && f0.observer_frequency_verification_passes == 1
            && f0.frequency_shift_passes == 1
            && g0.trace_invocations == 1
            && g0.coordinate_passes == 1
            && g0.texture_render_passes == 1,
        format!(
            "disk={} mapped={} fail={} ver={} fs={} trace={}",
            f0.disk_hit_count,
            f0.mapped_count,
            f0.mapping_failure_count,
            f0.observer_frequency_verification_passes,
            f0.frequency_shift_passes,
            g0.trace_invocations
        ),
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
        convention_ok(&f0.convention)
            && f0.velocity_model == DiskVelocityModel::ProgradeCircularGeodesic
            && f0.resolved_direction == EquatorialAngularDirection::PositivePhi,
        f0.convention.convention_id.clone(),
    );

    // Gate 2A2 compatibility without frequency flag.
    let gate_2a2_compat = run_worker(
        &root,
        DiagnosticRenderTier::Gate,
        "artifacts/gate-2b0-frequency-shift/gate-2a2-compat",
        authoritative_threads,
        false,
    )?;
    push(
        &mut checks,
        "gate_2a2_compat_no_frequency_field",
        gate_2a2_compat.disk_frequency_shift.is_none(),
        "optional field omitted".into(),
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
        "no_emission_intensity_claims",
        no_forbidden_emission_claims(&root)?,
        "frequency kinematics only".into(),
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

    let mut report = Gate2b0Eval {
        gate: "gate-2b0-frequency-shift".into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads: available,
        authoritative_threads,
        texture_spec_digest: texture_spec_digest.clone(),
        checks,
        smoke_thread_1: Some(smoke_thread_1),
        smoke_thread_bounded: Some(smoke_thread_bounded),
        gate_runs,
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
        return Err("gate-2b0-frequency-shift evaluation FAIL".into());
    }
    Ok(())
}

fn convention_ok(c: &DiskFrequencyShiftConvention) -> bool {
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
    let p = Covector::new(1.0, -1.0, 0.0, 0.0);
    let u = Vector::new(1.0, 0.0, 0.0, 0.0);
    let nu = measured_frequency_from_backward_covector(&p, &u).unwrap();
    let g = frequency_shift_ratio(nu, nu).unwrap();
    push(
        checks,
        "algebraic_same_observer_emitter_g1",
        (g.value() - 1.0).abs() < 1e-15,
        format!("g={}", g.value()),
    );

    let k = Covector::new(-1.0, 1.0, 0.0, 0.0);
    let nb = measured_frequency_from_backward_covector(&p, &u).unwrap();
    let nf = measured_frequency_from_future_covector(&k, &u).unwrap();
    push(
        checks,
        "algebraic_past_future_equivalence",
        (nb.value() - nf.value()).abs() < 1e-15,
        format!("{} vs {}", nb.value(), nf.value()),
    );

    let u_em = {
        let beta = 0.5_f64;
        let gamma = 1.0 / (1.0 - beta * beta).sqrt();
        Vector::new(gamma, gamma * beta, 0.0, 0.0)
    };
    let g0 = frequency_shift_ratio(
        measured_frequency_from_backward_covector(&p, &u).unwrap(),
        measured_frequency_from_backward_covector(&p, &u_em).unwrap(),
    )
    .unwrap()
    .value();
    let g_scaled = frequency_shift_ratio(
        measured_frequency_from_backward_covector(&p.scale(3.0), &u).unwrap(),
        measured_frequency_from_backward_covector(&p.scale(3.0), &u_em).unwrap(),
    )
    .unwrap()
    .value();
    push(
        checks,
        "algebraic_positive_scaling_invariance",
        (g0 - g_scaled).abs() < 1e-14,
        format!("{g0} vs {g_scaled}"),
    );

    let mut doppler_ok = true;
    for beta in [-0.5_f64, -0.1, 0.0, 0.1, 0.5] {
        let gamma = 1.0 / (1.0 - beta * beta).sqrt();
        let u_em = Vector::new(gamma, gamma * beta, 0.0, 0.0);
        let g = frequency_shift_ratio(
            measured_frequency_from_backward_covector(&p, &u).unwrap(),
            measured_frequency_from_backward_covector(&p, &u_em).unwrap(),
        )
        .unwrap()
        .value();
        let expected = ((1.0 + beta) / (1.0 - beta)).sqrt();
        if (g - expected).abs() > 1e-12 {
            doppler_ok = false;
        }
    }
    push(
        checks,
        "algebraic_minkowski_sr_doppler",
        doppler_ok,
        "β corpus".into(),
    );

    let sch = KerrParams::new(1.0, 0.0).unwrap();
    let r = 10.0;
    let pos =
        circular_equatorial_geodesic_bl(&sch, r, EquatorialAngularDirection::PositivePhi).unwrap();
    let neg =
        circular_equatorial_geodesic_bl(&sch, r, EquatorialAngularDirection::NegativePhi).unwrap();
    let expected_omega = (1.0 / r.powi(3)).sqrt();
    push(
        checks,
        "algebraic_schwarzschild_omega",
        (pos.angular_velocity_bl - expected_omega).abs() < 1e-14
            && (neg.angular_velocity_bl + expected_omega).abs() < 1e-14
            && (pos.four_velocity_bl.t - neg.four_velocity_bl.t).abs() < 1e-14,
        format!("Ω+={}", pos.angular_velocity_bl),
    );

    let plus = KerrParams::new(1.0, 0.5).unwrap();
    let zero = KerrParams::new(1.0, 0.0).unwrap();
    let minus = KerrParams::new(1.0, -0.5).unwrap();
    push(
        checks,
        "algebraic_prograde_spin_policy",
        prograde_equatorial_direction(&plus) == EquatorialAngularDirection::PositivePhi
            && prograde_equatorial_direction(&zero) == EquatorialAngularDirection::PositivePhi
            && prograde_equatorial_direction(&minus) == EquatorialAngularDirection::NegativePhi,
        "a=±0.5,0".into(),
    );

    let mut blks_ok = true;
    let mut norm_ok = true;
    let spins = [0.0_f64, 0.5, 0.999, -0.5];
    let radii = [6.0_f64, 10.0, 20.0];
    for &a in &spins {
        let params = KerrParams::new(1.0, a).unwrap();
        let dir = prograde_equatorial_direction(&params);
        for &r in &radii {
            let Ok(orbit) = circular_equatorial_geodesic_bl(&params, r, dir) else {
                continue;
            };
            if orbit.normalization_residual.abs() >= 1e-12 || !(orbit.four_velocity_bl.t > 0.0) {
                norm_ok = false;
            }
            let bl = relativity_core::PositionBl::new(0.0, r, std::f64::consts::FRAC_PI_2, 0.4);
            let p_bl = Covector::new(1.0, 0.1, 0.0, -0.4);
            let p_ks = covector_bl_to_ks(&params, &bl, &p_bl).unwrap();
            let u_ks =
                relativity_core::vector_bl_to_ks(&params, &bl, &orbit.four_velocity_bl).unwrap();
            let nu_bl = contract_covector_vector(&p_bl, &orbit.four_velocity_bl);
            let nu_ks = contract_covector_vector(&p_ks, &u_ks);
            if (nu_bl - nu_ks).abs() >= 1e-10 {
                blks_ok = false;
            }
        }
    }
    push(
        checks,
        "algebraic_bl_ks_contraction_invariance",
        blks_ok,
        "equatorial corpus".into(),
    );
    push(
        checks,
        "algebraic_circular_normalization",
        norm_ok,
        "future-directed g(u,u)≈-1".into(),
    );
}

fn check_cli_negative(
    root: &Path,
    checks: &mut Vec<Check>,
    name: &str,
    surface_set: TraceSurfaceSet,
    mode: LensedCelestialMode,
    output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let abs = root.join(output_dir);
    let _ = std::fs::remove_dir_all(&abs);
    let out = Command::new("cargo")
        .current_dir(root)
        .args([
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
            "smoke",
            "--surface-set",
            surface_set.as_str(),
            "--mode",
            mode.as_str(),
            "--texture",
            TEXTURE_ID_V1,
            "--emit-disk-frequency-shift",
            "--output-dir",
            output_dir,
            "--execution",
            "serial",
            "--require-release",
        ])
        .output()?;
    let rejected = !out.status.success();
    let no_artifacts = !abs.exists()
        || (!abs.join("disk-frequency-shift-map.json").exists()
            && !abs.join("lensed-celestial-report.json").exists());
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

fn check_freq_worker(
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
    push(
        checks,
        &format!("{label}_pass_counts"),
        report.trace_invocations == 1
            && freq.observer_frequency_verification_passes == 1
            && freq.frequency_shift_passes == 1
            && report.coordinate_passes == 1
            && report.texture_render_passes == 1,
        format!(
            "trace={} ver={} fs={}",
            report.trace_invocations,
            freq.observer_frequency_verification_passes,
            freq.frequency_shift_passes
        ),
    );
    push(
        checks,
        &format!("{label}_mapping_complete"),
        freq.mapped_count == freq.disk_hit_count && freq.mapping_failure_count == 0,
        format!(
            "mapped={} disk={} fail={}",
            freq.mapped_count, freq.disk_hit_count, freq.mapping_failure_count
        ),
    );
    push(
        checks,
        &format!("{label}_observer_residual"),
        freq.maximum_observer_unit_frequency_residual <= OBS_TOL,
        format!("{}", freq.maximum_observer_unit_frequency_residual),
    );
    Ok(())
}

fn run_worker(
    root: &Path,
    tier: DiagnosticRenderTier,
    output_dir: &str,
    threads: usize,
    emit_frequency: bool,
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
    // Find threads slot.
    let thread_idx = args.iter().position(|a| *a == "--threads").unwrap() + 1;
    args[thread_idx] = threads_s.as_str();
    if emit_frequency {
        args.push("--emit-disk-frequency-shift");
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

fn no_forbidden_emission_claims(root: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let fs = std::fs::read_to_string(root.join("crates/relativity-render/src/frequency_shift.rs"))?;
    let worker = std::fs::read_to_string(root.join("xtask/src/render_lensed_celestial.rs"))?;
    let blob = format!("{fs}\n{worker}").to_lowercase();
    let forbidden = [
        "specific_intensity",
        "novikov",
        "blackbody",
        "g^3",
        "g³",
        "g^4",
        "g⁴",
        "openexr",
        "acescg",
        "wgpu",
        "egui",
    ];
    Ok(forbidden.iter().all(|f| !blob.contains(f)))
}

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2b0Eval {
    Gate2b0Eval {
        gate: "gate-2b0-frequency-shift".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        texture_spec_digest: String::new(),
        checks,
        smoke_thread_1: None,
        smoke_thread_bounded: None,
        gate_runs: vec![],
        gate_2a2_compat: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut Gate2b0Eval) -> Result<(), Box<dyn std::error::Error>> {
    if report.content_digest_excluding_digest_field.is_empty() {
        let mut h = report.clone();
        h.content_digest_excluding_digest_field.clear();
        report.content_digest_excluding_digest_field = eval_digest(&h);
    }
    let dir = root.join("artifacts/gate-2b0-frequency-shift");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# Gate 2B0 frequency-shift evaluation\n\n");
    md.push_str(&format!("- result: `{}`\n", report.result));
    md.push_str(&format!("- authoritative: `{}`\n", report.authoritative));
    md.push_str(&format!("- commit: `{}`\n", report.commit));
    md.push_str(&format!(
        "- digest: `{}`\n",
        report.content_digest_excluding_digest_field
    ));
    if let Some(g) = report.gate_runs.first() {
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
            md.push_str(&format!(
                "- disk_hit/mapped/fail: {}/{}/{}\n",
                f.disk_hit_count, f.mapped_count, f.mapping_failure_count
            ));
            md.push_str(&format!(
                "- red/blue/unity: {}/{}/{}\n",
                f.redshifted_count, f.blueshifted_count, f.exact_unity_count
            ));
            if let Some(m) = &f.minimum_g {
                md.push_str(&format!(
                    "- min g: {} @ ({},{})\n",
                    m.g_factor, m.col, m.row
                ));
            }
            if let Some(m) = &f.maximum_g {
                md.push_str(&format!(
                    "- max g: {} @ ({},{})\n",
                    m.g_factor, m.col, m.row
                ));
            }
            if let Some(m) = &f.closest_to_unity {
                md.push_str(&format!(
                    "- closest to unity: {} @ ({},{})\n",
                    m.g_factor, m.col, m.row
                ));
            }
            md.push_str(&format!(
                "- max |disk radius residual|: {}\n",
                f.maximum_abs_disk_radius_residual
            ));
            md.push_str(&format!(
                "- max observer unit-frequency residual: {}\n",
                f.maximum_observer_unit_frequency_residual
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

fn eval_digest(report: &Gate2b0Eval) -> String {
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
        checks: Vec<DigestCheck<'a>>,
        smoke_thread_1: Option<&'a LensedCelestialReport>,
        smoke_thread_bounded: Option<&'a LensedCelestialReport>,
        gate_runs: &'a [LensedCelestialReport],
        gate_2a2_compat: Option<&'a LensedCelestialReport>,
        content_digest_excluding_digest_field: &'a str,
    }
    let s1 = report.smoke_thread_1.as_ref().map(strip_timing);
    let sb = report.smoke_thread_bounded.as_ref().map(strip_timing);
    let gates: Vec<_> = report.gate_runs.iter().map(strip_timing).collect();
    let compat = report.gate_2a2_compat.as_ref().map(strip_timing);
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
        gate_2a2_compat: compat.as_ref(),
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
