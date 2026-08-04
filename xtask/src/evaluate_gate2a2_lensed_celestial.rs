//! Gate 2A2 first deterministic lensed celestial diagnostic evaluator.

use crate::build_meta::{
    is_optimized_release_execution, read_build_execution_report, require_release_execution,
    BuildExecutionMetadata,
};
use crate::render_lensed_celestial::LensedCelestialReport;
use crate::render_tier::{DiagnosticRenderTier, RenderAuthorityClass, ResolutionSource};
use crate::trace_outcome_map::read_trace_execution_report;
use relativity_render::{
    procedural_coordinate_grid_v1, procedural_texture_spec_digest,
    render_procedural_texture_reference, LensedCelestialMode, TEXTURE_ID_V1,
};
use relativity_trace::{
    encode_ppm, hex_sha, OutcomeCounts, TraceExecutionMode, TraceSurfaceSet,
    CELESTIAL_CONVENTION_ID,
};
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
const REF_COORD_JSON: &str = "e37b8f32990aa8dd95557899ccdc80fd5d38bec5ace7fccef18541b666cb61ca";
const REF_UV: &str = "4262eb4fe84937557cf3679fa390d2883151a2aaf25e9b973d6297acfe8f2107";
const APPROVED_BASE: &str = "bab17d21b9e5ff5d153a0f1a7dc7ec46e861df87";

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct Gate2a2Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    reference_texture_digest: String,
    texture_spec_digest: String,
    checks: Vec<Check>,
    smoke: Option<LensedCelestialReport>,
    opaque_gate_runs: Vec<LensedCelestialReport>,
    disk_omitted_gate_runs: Vec<LensedCelestialReport>,
    showcase: Option<LensedCelestialReport>,
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
        return Err("gate-2a2-lensed-celestial requires release evaluator".into());
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

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;
    let smoke_threads = available.clamp(1, 2);

    let out_root = root.join("artifacts/gate-2a2-lensed-celestial");
    std::fs::create_dir_all(&out_root)?;

    // ---- Reference texture (pure render, no tracing) ----
    let texture_spec = procedural_coordinate_grid_v1();
    let texture_spec_digest = procedural_texture_spec_digest(&texture_spec);
    push(
        &mut checks,
        "texture_spec_canonical_v1",
        texture_spec.texture_id == TEXTURE_ID_V1
            && texture_spec.schema_version == 1
            && texture_spec.longitude_sectors == 8
            && texture_spec.latitude_cells == 12
            && texture_spec.minor_longitude_divisions == 24
            && texture_spec.minor_latitude_divisions == 12
            && texture_spec.major_longitude_stride == 3
            && texture_spec.major_latitude_stride == 3
            && texture_spec.marker_radius_millidegrees == 7000,
        texture_spec_digest.to_string(),
    );
    let ref_frame = render_procedural_texture_reference(&texture_spec, 512, 256)?;
    let ref_ppm = encode_ppm(&ref_frame);
    let reference_texture_digest = hex_sha(&ref_ppm);
    std::fs::write(
        out_root.join("procedural-celestial-reference.ppm"),
        &ref_ppm,
    )?;
    let ref_json = serde_json::json!({
        "width": 512,
        "height": 256,
        "texture_id": TEXTURE_ID_V1,
        "texture_spec_digest": texture_spec_digest,
        "ppm_digest": reference_texture_digest,
        "sampling": "equirectangular-center-of-pixel",
        "claim": "diagnostic procedural atlas; not an input raster",
    });
    std::fs::write(
        out_root.join("procedural-celestial-reference.json"),
        serde_json::to_vec_pretty(&ref_json)?,
    )?;
    let ref_again = render_procedural_texture_reference(&texture_spec, 512, 256)?;
    push(
        &mut checks,
        "reference_texture_deterministic",
        encode_ppm(&ref_again) == ref_ppm,
        reference_texture_digest.clone(),
    );

    let smoke = run_worker(
        &root,
        DiagnosticRenderTier::Smoke,
        TraceSurfaceSet::HorizonEscapeOnly,
        LensedCelestialMode::DiskOmittedDiagnostic,
        "artifacts/gate-2a2-lensed-celestial/smoke-disk-omitted",
        smoke_threads,
    )?;
    check_worker(&mut checks, "smoke", &smoke, false)?;

    let mut opaque_gate_runs = Vec::new();
    for i in 0..2 {
        opaque_gate_runs.push(run_worker(
            &root,
            DiagnosticRenderTier::Gate,
            TraceSurfaceSet::OpaqueDiskHorizonEscape,
            LensedCelestialMode::OpaqueDiskMask,
            &format!("artifacts/gate-2a2-lensed-celestial/opaque-gate-run-{i}"),
            authoritative_threads,
        )?);
    }
    check_worker(&mut checks, "opaque0", &opaque_gate_runs[0], true)?;
    check_worker(&mut checks, "opaque1", &opaque_gate_runs[1], true)?;

    let mut disk_omitted_gate_runs = Vec::new();
    for i in 0..2 {
        disk_omitted_gate_runs.push(run_worker(
            &root,
            DiagnosticRenderTier::Gate,
            TraceSurfaceSet::HorizonEscapeOnly,
            LensedCelestialMode::DiskOmittedDiagnostic,
            &format!("artifacts/gate-2a2-lensed-celestial/disk-omitted-gate-run-{i}"),
            authoritative_threads,
        )?);
    }
    check_worker(&mut checks, "omitted0", &disk_omitted_gate_runs[0], true)?;
    check_worker(&mut checks, "omitted1", &disk_omitted_gate_runs[1], true)?;

    let showcase = run_worker(
        &root,
        DiagnosticRenderTier::Showcase,
        TraceSurfaceSet::HorizonEscapeOnly,
        LensedCelestialMode::DiskOmittedDiagnostic,
        "artifacts/gate-2a2-lensed-celestial/showcase-disk-omitted",
        authoritative_threads,
    )?;
    check_worker(&mut checks, "showcase", &showcase, false)?;
    push(
        &mut checks,
        "showcase_non_authoritative",
        showcase.authority_class == RenderAuthorityClass::NonAuthoritative
            && showcase.render_tier == Some(DiagnosticRenderTier::Showcase),
        format!("{:?}", showcase.authority_class),
    );

    // Numerical / convention identity
    push(
        &mut checks,
        "numerical_profile_matches_2a0",
        opaque_gate_runs[0].numerical_profile_digest == REF_NUMERICAL_PROFILE
            && disk_omitted_gate_runs[0].numerical_profile_digest == REF_NUMERICAL_PROFILE,
        opaque_gate_runs[0].numerical_profile_digest.clone(),
    );
    push(
        &mut checks,
        "gate_2a1_convention_identity",
        opaque_gate_runs[0].convention_id == CELESTIAL_CONVENTION_ID
            && opaque_gate_runs[0].resolved_boundary_radius == 80.0
            && opaque_gate_runs[0].radius_policy == "gate-1b2-diagnostic-radius-cap",
        opaque_gate_runs[0].convention_id.clone(),
    );

    // Opaque determinism across workers
    let o0 = &opaque_gate_runs[0];
    let o1 = &opaque_gate_runs[1];
    push(
        &mut checks,
        "opaque_workers_byte_identical",
        reports_mode_identical(o0, o1)
            && files_eq(
                &root,
                "artifacts/gate-2a2-lensed-celestial/opaque-gate-run-0",
                "artifacts/gate-2a2-lensed-celestial/opaque-gate-run-1",
                o0.mode.ppm_filename(),
            )?,
        format!("lensed={}", o0.lensed_ppm_digest),
    );

    // Disk-omitted determinism
    let d0 = &disk_omitted_gate_runs[0];
    let d1 = &disk_omitted_gate_runs[1];
    push(
        &mut checks,
        "disk_omitted_workers_byte_identical",
        reports_mode_identical(d0, d1)
            && files_eq(
                &root,
                "artifacts/gate-2a2-lensed-celestial/disk-omitted-gate-run-0",
                "artifacts/gate-2a2-lensed-celestial/disk-omitted-gate-run-1",
                d0.mode.ppm_filename(),
            )?,
        format!("lensed={}", d0.lensed_ppm_digest),
    );

    // Gate 1B2 + Gate 2A1 opaque references
    push(
        &mut checks,
        "opaque_gate_1b2_class",
        o0.outcome_class_digest == REF_CLASS,
        o0.outcome_class_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_1b2_categorical_ppm",
        o0.categorical_ppm_digest == REF_PPM,
        o0.categorical_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_1b2_rhs_pgm",
        o0.rhs_pgm_digest == REF_PGM,
        o0.rhs_pgm_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_1b2_counts",
        counts_eq(&o0.outcome_counts, &REF_COUNTS) && o0.outcome_counts.failed == 0,
        format!("{:?}", o0.outcome_counts),
    );
    push(
        &mut checks,
        "opaque_gate_2a1_coordinate_digest",
        o0.coordinate_digest == REF_COORD,
        o0.coordinate_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_2a1_coordinate_json_digest",
        o0.coordinate_json_digest == REF_COORD_JSON,
        o0.coordinate_json_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_gate_2a1_uv_debug_ppm_digest",
        o0.uv_debug_ppm_digest == REF_UV,
        o0.uv_debug_ppm_digest.clone(),
    );
    push(
        &mut checks,
        "opaque_escaped_mapped_accounting",
        o0.texture_sample_count == 2442
            && o0.outcome_counts.escaped == 2442
            && o0.mapping_failure_count == 0,
        format!(
            "escaped={} samples={} fail={}",
            o0.outcome_counts.escaped, o0.texture_sample_count, o0.mapping_failure_count
        ),
    );

    // Disk-omitted semantics
    let omitted_total = d0.outcome_counts.disk_hit
        + d0.outcome_counts.escaped
        + d0.outcome_counts.horizon_event
        + d0.outcome_counts.horizon_approach
        + d0.outcome_counts.affine_limit
        + d0.outcome_counts.failed;
    push(
        &mut checks,
        "disk_omitted_zero_disk_hit",
        d0.outcome_counts.disk_hit == 0 && d1.outcome_counts.disk_hit == 0,
        format!("{}", d0.outcome_counts.disk_hit),
    );
    push(
        &mut checks,
        "disk_omitted_failed_zero",
        d0.outcome_counts.failed == 0,
        format!("{}", d0.outcome_counts.failed),
    );
    push(
        &mut checks,
        "disk_omitted_pixel_accounting",
        omitted_total == (d0.width as u64) * (d0.height as u64)
            && d0.texture_sample_count == d0.outcome_counts.escaped
            && d0.texture_sample_count + d0.non_escaped_count
                == (d0.width as u64) * (d0.height as u64),
        format!("{:?}", d0.outcome_counts),
    );
    push(
        &mut checks,
        "disk_omitted_more_texture_samples_than_opaque",
        d0.texture_sample_count > o0.texture_sample_count,
        format!(
            "omitted={} opaque={}",
            d0.texture_sample_count, o0.texture_sample_count
        ),
    );
    push(
        &mut checks,
        "opaque_and_disk_omitted_images_differ",
        o0.lensed_ppm_digest != d0.lensed_ppm_digest,
        format!("{} vs {}", o0.lensed_ppm_digest, d0.lensed_ppm_digest),
    );
    push(
        &mut checks,
        "lensed_differs_from_categorical_and_uv",
        o0.lensed_ppm_digest != o0.categorical_ppm_digest
            && o0.lensed_ppm_digest != o0.uv_debug_ppm_digest
            && d0.lensed_ppm_digest != d0.categorical_ppm_digest
            && d0.lensed_ppm_digest != d0.uv_debug_ppm_digest,
        "ok".into(),
    );
    push(
        &mut checks,
        "texture_spec_digest_stable_across_workers",
        o0.texture_spec_digest == texture_spec_digest
            && d0.texture_spec_digest == texture_spec_digest
            && smoke.texture_spec_digest == texture_spec_digest,
        texture_spec_digest.clone(),
    );

    let no_forbidden = no_forbidden_claims(&root)?;
    push(
        &mut checks,
        "no_asymptotic_correction_or_radiometry",
        no_forbidden,
        "no asymptotic correction / radiometry / redshift / openexr / wgpu".into(),
    );

    let hard_fail = checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    let gate_ok = o0.render_tier == Some(DiagnosticRenderTier::Gate)
        && o0.width == 128
        && o0.height == 128
        && o0.resolution_source == ResolutionSource::NamedTier
        && o0.authority_class == RenderAuthorityClass::AuthoritativeCandidate
        && d0.render_tier == Some(DiagnosticRenderTier::Gate);
    let authoritative = !dirty && !hard_fail && self_release && gate_ok && ancestor_ok;
    let result = if hard_fail {
        "FAIL"
    } else if authoritative {
        "PASS"
    } else {
        "PASS_NON_AUTHORITATIVE"
    };

    let mut report = Gate2a2Eval {
        gate: "gate-2a2-lensed-celestial".into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads: available,
        authoritative_threads,
        reference_texture_digest: reference_texture_digest.clone(),
        texture_spec_digest: texture_spec_digest.clone(),
        checks,
        smoke: Some(smoke),
        opaque_gate_runs,
        disk_omitted_gate_runs,
        showcase: Some(showcase),
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
        return Err("gate-2a2-lensed-celestial evaluation FAIL".into());
    }
    Ok(())
}

fn reports_mode_identical(a: &LensedCelestialReport, b: &LensedCelestialReport) -> bool {
    a.trace_data_digest == b.trace_data_digest
        && a.outcome_class_digest == b.outcome_class_digest
        && a.coordinate_digest == b.coordinate_digest
        && a.coordinate_json_digest == b.coordinate_json_digest
        && a.uv_debug_ppm_digest == b.uv_debug_ppm_digest
        && a.lensed_ppm_digest == b.lensed_ppm_digest
        && a.categorical_ppm_digest == b.categorical_ppm_digest
        && a.rhs_pgm_digest == b.rhs_pgm_digest
        && counts_eq(&a.outcome_counts, &b.outcome_counts)
        && a.texture_sample_count == b.texture_sample_count
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

fn no_forbidden_claims(root: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let texture = std::fs::read_to_string(root.join("crates/relativity-render/src/texture.rs"))?;
    let lensed = std::fs::read_to_string(root.join("crates/relativity-render/src/lensed.rs"))?;
    let worker = std::fs::read_to_string(root.join("xtask/src/render_lensed_celestial.rs"))?;
    let blob = format!("{texture}\n{lensed}\n{worker}");
    let lower = blob.to_lowercase();
    let forbidden = [
        "asymptotic_direction",
        "direction_at_infinity",
        "infinity_uv",
        "gravitational_redshift",
        "doppler",
        "specific_intensity",
        "openexr",
        "wgpu",
        "egui",
    ];
    Ok(forbidden.iter().all(|f| !lower.contains(f)))
}

fn check_worker(
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
                && report.resolution_source == ResolutionSource::NamedTier,
            format!(
                "tier={:?} {}×{}",
                report.render_tier, report.width, report.height
            ),
        );
    }
    push(
        checks,
        &format!("{label}_trace_invocations"),
        report.trace_invocations == 1,
        format!("{}", report.trace_invocations),
    );
    push(
        checks,
        &format!("{label}_coordinate_passes"),
        report.coordinate_passes == 1,
        format!("{}", report.coordinate_passes),
    );
    push(
        checks,
        &format!("{label}_texture_render_passes"),
        report.texture_render_passes == 1,
        format!("{}", report.texture_render_passes),
    );
    push(
        checks,
        &format!("{label}_worker_build_release"),
        report.build.is_optimized_release_execution(),
        report.build.describe(),
    );
    push(
        checks,
        &format!("{label}_worker_execution_parallel"),
        report.execution.mode == TraceExecutionMode::Parallel,
        format!("{:?}", report.execution),
    );
    push(
        checks,
        &format!("{label}_mapping_failures_zero"),
        report.mapping_failure_count == 0
            && report.texture_sample_count == report.outcome_counts.escaped,
        format!(
            "escaped={} samples={} fail={}",
            report.outcome_counts.escaped,
            report.texture_sample_count,
            report.mapping_failure_count
        ),
    );
    Ok(())
}

fn run_worker(
    root: &Path,
    tier: DiagnosticRenderTier,
    surface_set: TraceSurfaceSet,
    mode: LensedCelestialMode,
    output_dir: &str,
    threads: usize,
) -> Result<LensedCelestialReport, Box<dyn std::error::Error>> {
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
            tier.as_str(),
            "--surface-set",
            surface_set.as_str(),
            "--mode",
            mode.as_str(),
            "--texture",
            TEXTURE_ID_V1,
            "--output-dir",
            output_dir,
            "--execution",
            "parallel",
            "--threads",
            &threads.to_string(),
            "--require-release",
        ])
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

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2a2Eval {
    Gate2a2Eval {
        gate: "gate-2a2-lensed-celestial".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        reference_texture_digest: String::new(),
        texture_spec_digest: String::new(),
        checks,
        smoke: None,
        opaque_gate_runs: vec![],
        disk_omitted_gate_runs: vec![],
        showcase: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut Gate2a2Eval) -> Result<(), Box<dyn std::error::Error>> {
    if report.content_digest_excluding_digest_field.is_empty() {
        let mut h = report.clone();
        h.content_digest_excluding_digest_field.clear();
        report.content_digest_excluding_digest_field = eval_digest(&h);
    }
    let dir = root.join("artifacts/gate-2a2-lensed-celestial");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# Gate 2A2 Lensed Celestial Evaluation\n\n");
    md.push_str(&format!("- Result: **{}**\n", report.result));
    md.push_str(&format!("- Authoritative: {}\n", report.authoritative));
    md.push_str(&format!("- Commit: `{}`\n", report.commit));
    md.push_str(&format!(
        "- Digest: `{}`\n",
        report.content_digest_excluding_digest_field
    ));
    md.push_str(&format!(
        "- Texture spec digest: `{}`\n",
        report.texture_spec_digest
    ));
    md.push_str(&format!(
        "- Reference texture digest: `{}`\n\n",
        report.reference_texture_digest
    ));
    md.push_str("## Checks\n\n");
    for c in &report.checks {
        md.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    if let Some(o) = report.opaque_gate_runs.first() {
        md.push_str("\n## Opaque gate-run-0\n\n");
        md.push_str(&format!("- lensed: `{}`\n", o.lensed_ppm_digest));
        md.push_str(&format!("- coordinate: `{}`\n", o.coordinate_digest));
        md.push_str(&format!("- class: `{}`\n", o.outcome_class_digest));
        md.push_str(&format!("- texture samples: {}\n", o.texture_sample_count));
    }
    if let Some(d) = report.disk_omitted_gate_runs.first() {
        md.push_str("\n## Disk-omitted gate-run-0\n\n");
        md.push_str(&format!("- lensed: `{}`\n", d.lensed_ppm_digest));
        md.push_str(&format!("- coordinate: `{}`\n", d.coordinate_digest));
        md.push_str(&format!("- class: `{}`\n", d.outcome_class_digest));
        md.push_str(&format!("- texture samples: {}\n", d.texture_sample_count));
        md.push_str(&format!("- outcomes: {:?}\n", d.outcome_counts));
    }
    if let Some(s) = &report.showcase {
        md.push_str("\n## Showcase\n\n");
        md.push_str("- path: `artifacts/gate-2a2-lensed-celestial/showcase-disk-omitted/`\n");
        md.push_str(&format!("- lensed: `{}`\n", s.lensed_ppm_digest));
    }
    std::fs::write(dir.join("evaluation.md"), md)?;
    std::fs::write(
        dir.join("evaluation.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;
    Ok(())
}

fn eval_digest(report: &Gate2a2Eval) -> String {
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
        reference_texture_digest: &'a str,
        texture_spec_digest: &'a str,
        checks: Vec<DigestCheck<'a>>,
        smoke: Option<&'a LensedCelestialReport>,
        opaque_gate_runs: &'a [LensedCelestialReport],
        disk_omitted_gate_runs: &'a [LensedCelestialReport],
        showcase: Option<&'a LensedCelestialReport>,
        content_digest_excluding_digest_field: &'a str,
    }
    let smoke = report.smoke.as_ref().map(strip_timing);
    let opaque: Vec<_> = report.opaque_gate_runs.iter().map(strip_timing).collect();
    let omitted: Vec<_> = report
        .disk_omitted_gate_runs
        .iter()
        .map(strip_timing)
        .collect();
    let showcase = report.showcase.as_ref().map(strip_timing);
    let proj = Proj {
        gate: &report.gate,
        result: &report.result,
        authoritative: report.authoritative,
        commit: &report.commit,
        dirty: report.dirty,
        build: &report.build,
        available_threads: report.available_threads,
        authoritative_threads: report.authoritative_threads,
        reference_texture_digest: &report.reference_texture_digest,
        texture_spec_digest: &report.texture_spec_digest,
        checks: report
            .checks
            .iter()
            .map(|c| DigestCheck {
                name: &c.name,
                status: c.status,
            })
            .collect(),
        smoke: smoke.as_ref(),
        opaque_gate_runs: &opaque,
        disk_omitted_gate_runs: &omitted,
        showcase: showcase.as_ref(),
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize"))
}

fn strip_timing(r: &LensedCelestialReport) -> LensedCelestialReport {
    let mut c = r.clone();
    c.trace_wall_clock_seconds = None;
    c.mapping_wall_clock_seconds = None;
    c.render_wall_clock_seconds = None;
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

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("no parent")?
        .to_path_buf())
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((!text.is_empty(), text))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    if !out.status.success() {
        return Err("git failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
