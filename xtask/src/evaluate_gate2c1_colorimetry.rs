//! Gate 2C1 physical colorimetry evaluator.

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::render_tier::DiagnosticRenderTier;
use relativity_render::{
    blackbody_planckian_direction_ok, integrate_xyz_from_emission, payload_sha256,
    physical_spectral_grid_v1, Cie1931Table, IntegrationMeasure, XyzToRgbMatrix,
    CIE_OBSERVER_ID_V1, CIE_RELATIVE_ASSET_PATH, CIE_TABLE_MD5, CIE_TABLE_SHA256,
    PHYSICAL_GRID_V1_ID, PRODUCTION_BAND_ID, PRODUCTION_LAMBDA_MAX_NM, PRODUCTION_LAMBDA_MIN_NM,
    PRODUCTION_N_SAMPLES, SCENE_LINEAR_RGB_SPACE_ID,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const APPROVED_BASE: &str = "57659c6202b8d8642891b5d0d88bce7d8f82f470";
const REF_FREQ_2B0: &str = "65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2";
const REF_EMISSION_2C0: &str = "5e3b15023df9bf3debed9666d65a3c762cfe83fe9885e7a5c8b3565dc19a383e";
const REF_SPECTRAL_2C0: &str = "136b1fbcc76beb08ea38aa24d16803d621da20bad5b7ebfecc7a13c260aa8dd1";
const REF_GRID_2C0: &str = "ceb3db28082bb357e50cac2635b221711bf79ea2806f2c25b60c61ca901162d5";

/// ν↔λ trapezoid agreement envelope (calibrated on 1 nm CIE).
const NU_LAMBDA_REL_TOL: f64 = 1e-5;
/// Sampling ladder: 10 nm max rel Y vs 1 nm reference (blackbody 6500 K).
const LADDER_10NM_REL_TOL: f64 = 5e-2;

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ColorRenderReport {
    gate: String,
    architecture: String,
    frequency_shift_digest: String,
    physical_emission_digest: String,
    physical_spectral_grid_digest: Option<String>,
    physical_spectral_digest: Option<String>,
    physical_color_digest: String,
    payload_sha256: String,
    cie_table_sha256: String,
    rgb_matrix_digest: String,
    color_disk_hit_count: u64,
    metrics: relativity_render::ColorimetricMetrics,
}

#[derive(Serialize)]
struct Gate2c1Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    checks: Vec<Check>,
    smoke_serial: Option<ColorRenderReport>,
    smoke_parallel: Option<ColorRenderReport>,
    gate_run: Option<ColorRenderReport>,
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
        return Err("gate-2c1-colorimetry requires release evaluator".into());
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

    hermetic_color_checks(&root, &mut checks)?;

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;
    let smoke_threads = available.clamp(1, 2);

    let out_root = root.join("artifacts/gate-2c1-colorimetry");
    std::fs::create_dir_all(&out_root)?;

    push(
        &mut checks,
        "cli_reject_bad_observer",
        run_color_cli(
            &root,
            "artifacts/gate-2c1-colorimetry/cli-neg-observer",
            DiagnosticRenderTier::Smoke,
            1,
            "not-an-observer",
            SCENE_LINEAR_RGB_SPACE_ID,
            true,
        )
        .is_err(),
        "expected failure".into(),
    );
    push(
        &mut checks,
        "cli_reject_bad_rgb_space",
        run_color_cli(
            &root,
            "artifacts/gate-2c1-colorimetry/cli-neg-rgb",
            DiagnosticRenderTier::Smoke,
            1,
            CIE_OBSERVER_ID_V1,
            "acescg",
            true,
        )
        .is_err(),
        "expected failure".into(),
    );

    let smoke_serial = run_color_cli(
        &root,
        "artifacts/gate-2c1-colorimetry/smoke-serial",
        DiagnosticRenderTier::Smoke,
        1,
        CIE_OBSERVER_ID_V1,
        SCENE_LINEAR_RGB_SPACE_ID,
        true,
    )?;
    let smoke_parallel = run_color_cli(
        &root,
        "artifacts/gate-2c1-colorimetry/smoke-parallel",
        DiagnosticRenderTier::Smoke,
        smoke_threads,
        CIE_OBSERVER_ID_V1,
        SCENE_LINEAR_RGB_SPACE_ID,
        true,
    )?;
    check_report(&mut checks, "smoke_serial", &smoke_serial);
    check_report(&mut checks, "smoke_parallel", &smoke_parallel);
    push(
        &mut checks,
        "smoke_serial_parallel_digest_identical",
        smoke_serial.physical_color_digest == smoke_parallel.physical_color_digest
            && smoke_serial.physical_emission_digest == smoke_parallel.physical_emission_digest
            && smoke_serial.frequency_shift_digest == smoke_parallel.frequency_shift_digest
            && smoke_serial.payload_sha256 == smoke_parallel.payload_sha256,
        smoke_serial.physical_color_digest.clone(),
    );
    push(
        &mut checks,
        "smoke_payload_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2c1-colorimetry/smoke-serial",
            "artifacts/gate-2c1-colorimetry/smoke-parallel",
            "physical-xyz-rgb.f64le",
        )?,
        "physical-xyz-rgb.f64le".into(),
    );
    verify_run_payload_meta(
        &mut checks,
        &root,
        "artifacts/gate-2c1-colorimetry/smoke-serial",
        &smoke_serial,
        "smoke_serial",
    )?;
    verify_run_payload_meta(
        &mut checks,
        &root,
        "artifacts/gate-2c1-colorimetry/smoke-parallel",
        &smoke_parallel,
        "smoke_parallel",
    )?;

    let gate_run = run_color_cli(
        &root,
        "artifacts/gate-2c1-colorimetry/gate-run-0",
        DiagnosticRenderTier::Gate,
        authoritative_threads,
        CIE_OBSERVER_ID_V1,
        SCENE_LINEAR_RGB_SPACE_ID,
        true,
    )?;
    check_report(&mut checks, "gate0", &gate_run);
    push(
        &mut checks,
        "gate_inherits_2b0_frequency_digest",
        gate_run.frequency_shift_digest == REF_FREQ_2B0,
        gate_run.frequency_shift_digest.clone(),
    );
    push(
        &mut checks,
        "gate_frozen_2c0_emission_digest",
        gate_run.physical_emission_digest == REF_EMISSION_2C0,
        gate_run.physical_emission_digest.clone(),
    );
    push(
        &mut checks,
        "gate_frozen_2c0_spectral_digest",
        gate_run.physical_spectral_digest.as_deref() == Some(REF_SPECTRAL_2C0),
        format!("{:?}", gate_run.physical_spectral_digest),
    );
    let frozen_grid = physical_spectral_grid_v1()?;
    let frozen_grid_digest = relativity_render::physical_spectral_grid_digest(&frozen_grid)?;
    push(
        &mut checks,
        "gate_frozen_2c0_grid_digest",
        gate_run.physical_spectral_grid_digest.as_deref() == Some(frozen_grid_digest.as_str())
            && frozen_grid_digest == REF_GRID_2C0
            && frozen_grid.grid_id() == PHYSICAL_GRID_V1_ID,
        frozen_grid_digest.clone(),
    );
    push(
        &mut checks,
        "gate_cie_table_sha256",
        gate_run.cie_table_sha256 == CIE_TABLE_SHA256,
        gate_run.cie_table_sha256.clone(),
    );
    push(
        &mut checks,
        "gate_exr_present",
        root.join("artifacts/gate-2c1-colorimetry/gate-run-0/physical-color.exr")
            .is_file(),
        "physical-color.exr".into(),
    );
    push(
        &mut checks,
        "gate_a_vs_b_diagnostic_present",
        root.join("artifacts/gate-2c1-colorimetry/gate-run-0/diagnostic-a-vs-b.json")
            .is_file(),
        "diagnostic-a-vs-b.json".into(),
    );
    verify_run_payload_meta(
        &mut checks,
        &root,
        "artifacts/gate-2c1-colorimetry/gate-run-0",
        &gate_run,
        "gate0",
    )?;
    // Explicit gate-run-0 authority closure: file hash ↔ meta ↔ report.
    {
        let gate_dir = root.join("artifacts/gate-2c1-colorimetry/gate-run-0");
        let payload = std::fs::read(gate_dir.join("physical-xyz-rgb.f64le"))?;
        let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
            gate_dir.join("physical-colorimetry-meta.json"),
        )?)?;
        let computed = payload_sha256(&payload);
        let meta_sha = meta
            .get("payload_sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let schema_ok = meta.get("payload_schema").and_then(|v| v.as_u64()) == Some(2);
        let band_ok =
            meta.get("production_band_id").and_then(|v| v.as_str()) == Some(PRODUCTION_BAND_ID);
        push(
            &mut checks,
            "raw_payload_self_consistent",
            !computed.is_empty()
                && computed == meta_sha
                && computed == gate_run.payload_sha256
                && schema_ok
                && band_ok,
            format!("sha={computed} schema_ok={schema_ok} band_ok={band_ok}"),
        );
    }
    // Negatives are diagnostic, not failures.
    push(
        &mut checks,
        "gate_negative_rgb_recorded_not_failure",
        true,
        format!(
            "negative_components={}",
            gate_run.metrics.negative_rgb_component_count
        ),
    );

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let authoritative = all_pass && !dirty && self_release;
    let mut report = Gate2c1Eval {
        gate: "gate-2c1-colorimetry".into(),
        result: if all_pass { "PASS" } else { "FAIL" }.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads: available,
        authoritative_threads,
        checks,
        smoke_serial: Some(smoke_serial),
        smoke_parallel: Some(smoke_parallel),
        gate_run: Some(gate_run),
        content_digest_excluding_digest_field: String::new(),
    };
    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !all_pass {
        return Err("gate-2c1-colorimetry FAIL".into());
    }
    Ok(())
}

fn hermetic_color_checks(
    root: &Path,
    checks: &mut Vec<Check>,
) -> Result<(), Box<dyn std::error::Error>> {
    let asset_path = root.join(CIE_RELATIVE_ASSET_PATH);
    push(
        checks,
        "hermetic_cie_asset_present",
        asset_path.is_file(),
        asset_path.display().to_string(),
    );

    let table = Cie1931Table::load_official_v1_from_path(&asset_path)?;
    push(
        checks,
        "hermetic_cie_sha256",
        table.content_sha256 == CIE_TABLE_SHA256,
        table.content_sha256.clone(),
    );
    push(
        checks,
        "hermetic_cie_md5_pin",
        CIE_TABLE_MD5 == "17cca777db64b17170f06f67ce9d3ab7",
        CIE_TABLE_MD5.into(),
    );
    push(
        checks,
        "hermetic_cie_runtime_load_not_include_str",
        table.content_sha256 == CIE_TABLE_SHA256 && asset_path.is_file(),
        format!(
            "mode=runtime-vendored-asset path={}",
            CIE_RELATIVE_ASSET_PATH
        ),
    );

    let samples = table.production_subset()?;
    push(
        checks,
        "hermetic_production_471",
        samples.len() == PRODUCTION_N_SAMPLES && samples.len() == 471,
        format!("{}", samples.len()),
    );
    push(
        checks,
        "hermetic_production_band_360_830",
        PRODUCTION_LAMBDA_MIN_NM == 360
            && PRODUCTION_LAMBDA_MAX_NM == 830
            && samples.first().map(|s| s.lambda_nm) == Some(PRODUCTION_LAMBDA_MIN_NM)
            && samples.last().map(|s| s.lambda_nm) == Some(PRODUCTION_LAMBDA_MAX_NM)
            && PRODUCTION_BAND_ID == "cie-1931-360-830-1nm-v1",
        PRODUCTION_BAND_ID.into(),
    );

    let xyz_nu =
        integrate_xyz_from_emission(6500.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)?;
    let xyz_l =
        integrate_xyz_from_emission(6500.0, 1.0, &samples, IntegrationMeasure::WavelengthLambda)?;
    let rel_y = (xyz_nu.y - xyz_l.y).abs() / xyz_nu.y.max(1e-30);
    push(
        checks,
        "hermetic_nu_lambda_agreement",
        rel_y < NU_LAMBDA_REL_TOL,
        format!("rel_y={rel_y}"),
    );

    let mut prev_err = f64::INFINITY;
    for step in [10i32, 5, 2, 1] {
        let s = table.subsampled(step)?;
        let xyz = integrate_xyz_from_emission(6500.0, 1.0, &s, IntegrationMeasure::FrequencyNu)?;
        let err = (xyz.y - xyz_nu.y).abs() / xyz_nu.y;
        if step == 10 {
            push(
                checks,
                "hermetic_ladder_10nm_envelope",
                err < LADDER_10NM_REL_TOL,
                format!("rel={err}"),
            );
        }
        if step < 10 {
            push(
                checks,
                &format!("hermetic_ladder_{step}nm_improves"),
                err <= prev_err * 1.05 + 1e-12,
                format!("{prev_err} → {err}"),
            );
        }
        prev_err = err;
    }

    let m = XyzToRgbMatrix::rec709_d65_linear_v1();
    let rgb = m.apply(xyz_nu)?;
    let back = m.invert_apply(rgb)?;
    let scale = xyz_nu
        .x
        .abs()
        .max(xyz_nu.y.abs())
        .max(xyz_nu.z.abs())
        .max(1.0);
    let rt_rel =
        ((back.x - xyz_nu.x).abs() + (back.y - xyz_nu.y).abs() + (back.z - xyz_nu.z).abs()) / scale;
    push(
        checks,
        "hermetic_rgb_matrix_roundtrip",
        rt_rel < 1e-12,
        format!("rel_L1={rt_rel} digest={}", m.digest()),
    );

    match blackbody_planckian_direction_ok(&samples) {
        Ok((pts, dxy)) => {
            push(
                checks,
                "hermetic_blackbody_planckian_direction",
                pts.len() == 4 && dxy < 1e-5,
                format!("pts={} dxy={dxy}", pts.len()),
            );
        }
        Err(e) => {
            push(
                checks,
                "hermetic_blackbody_planckian_direction",
                false,
                e.to_string(),
            );
        }
    }

    // Blackbody luminance increases with T at g=1.
    let y3 = integrate_xyz_from_emission(3000.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)?.y;
    let y6 = integrate_xyz_from_emission(6500.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)?.y;
    let y10 =
        integrate_xyz_from_emission(10000.0, 1.0, &samples, IntegrationMeasure::FrequencyNu)?.y;
    push(
        checks,
        "hermetic_blackbody_y_increases_with_t",
        y3 < y6 && y6 < y10,
        format!("Y(3k)={y3} Y(6.5k)={y6} Y(10k)={y10}"),
    );

    Ok(())
}

fn verify_run_payload_meta(
    checks: &mut Vec<Check>,
    root: &Path,
    output_dir: &str,
    report: &ColorRenderReport,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = root.join(output_dir);
    let payload = std::fs::read(dir.join("physical-xyz-rgb.f64le"))?;
    let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        dir.join("physical-colorimetry-meta.json"),
    )?)?;
    let computed = payload_sha256(&payload);
    let meta_sha = meta
        .get("payload_sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    push(
        checks,
        &format!("{label}_payload_sha256_matches_file"),
        computed == meta_sha && computed == report.payload_sha256,
        format!(
            "file={computed} meta={meta_sha} report={}",
            report.payload_sha256
        ),
    );
    push(
        checks,
        &format!("{label}_payload_meta_schema_v2"),
        meta.get("payload_schema").and_then(|v| v.as_u64()) == Some(2)
            && meta.get("cie_load_mode").and_then(|v| v.as_str()) == Some("runtime-vendored-asset")
            && meta.get("cie_license").and_then(|v| v.as_str()) == Some("CC-BY-SA-4.0")
            && meta.get("production_band_id").and_then(|v| v.as_str()) == Some(PRODUCTION_BAND_ID),
        "payload_schema/cie_load_mode/cie_license/production_band_id".into(),
    );
    Ok(())
}

fn check_report(checks: &mut Vec<Check>, label: &str, report: &ColorRenderReport) {
    push(
        checks,
        &format!("{label}_gate_id"),
        report.gate == "gate-2c1-colorimetry",
        report.gate.clone(),
    );
    push(
        checks,
        &format!("{label}_architecture_b"),
        report.architecture == "B-emission-frame-cie-1nm",
        report.architecture.clone(),
    );
    push(
        checks,
        &format!("{label}_has_color_pixels"),
        report.color_disk_hit_count > 0,
        format!("{}", report.color_disk_hit_count),
    );
    push(
        checks,
        &format!("{label}_cie_digest"),
        report.cie_table_sha256 == CIE_TABLE_SHA256,
        report.cie_table_sha256.clone(),
    );
    push(
        checks,
        &format!("{label}_payload_sha256_present"),
        report.payload_sha256.len() == 64,
        report.payload_sha256.clone(),
    );
}

fn run_color_cli(
    root: &Path,
    output_dir: &str,
    tier: DiagnosticRenderTier,
    threads: usize,
    cie_observer: &str,
    rgb_space: &str,
    require_release: bool,
) -> Result<ColorRenderReport, Box<dyn std::error::Error>> {
    let tier_str = match tier {
        DiagnosticRenderTier::Smoke => "smoke",
        DiagnosticRenderTier::Preview => "preview",
        DiagnosticRenderTier::Gate => "gate",
        DiagnosticRenderTier::Showcase => "showcase",
    };
    let mut args = vec![
        "run".into(),
        "--release".into(),
        "-p".into(),
        "xtask".into(),
        "--".into(),
        "render-physical-color".into(),
        "--preset".into(),
        "presets/gargantua-physical-v1.toml".into(),
        "--tier".into(),
        tier_str.into(),
        "--cie-observer".into(),
        cie_observer.into(),
        "--rgb-space".into(),
        rgb_space.into(),
        "--output-dir".into(),
        output_dir.into(),
        "--execution".into(),
        if threads <= 1 {
            "serial".into()
        } else {
            "parallel".into()
        },
    ];
    if threads > 1 {
        args.push("--threads".into());
        args.push(threads.to_string());
    }
    if require_release {
        args.push("--require-release".into());
    }
    let status = Command::new("cargo")
        .current_dir(root)
        .args(&args)
        .status()?;
    if !status.success() {
        return Err(format!("render-physical-color failed: {output_dir}").into());
    }
    let report_path = root
        .join(output_dir)
        .join("physical-color-render-report.json");
    let text = std::fs::read_to_string(report_path)?;
    Ok(serde_json::from_str(&text)?)
}

fn files_eq(root: &Path, a: &str, b: &str, name: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let ba = std::fs::read(root.join(a).join(name))?;
    let bb = std::fs::read(root.join(b).join(name))?;
    Ok(ba == bb)
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
    let status = cmd.status()?;
    push(
        checks,
        name,
        status.success(),
        format!("exit={}", status.code().unwrap_or(-1)),
    );
    Ok(())
}

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2c1Eval {
    Gate2c1Eval {
        gate: "gate-2c1-colorimetry".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        checks,
        smoke_serial: None,
        smoke_parallel: None,
        gate_run: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut Gate2c1Eval) -> Result<(), Box<dyn std::error::Error>> {
    let mut clone = serde_json::to_value(&*report)?;
    if let Some(obj) = clone.as_object_mut() {
        obj.remove("content_digest_excluding_digest_field");
    }
    let bytes = serde_json::to_vec(&clone)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    report.content_digest_excluding_digest_field = format!("{:x}", h.finalize());
    let out = root.join("artifacts/gate-2c1-colorimetry");
    std::fs::create_dir_all(&out)?;
    std::fs::write(
        out.join("gate-2c1-evaluate.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    Ok(())
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = git_stdout(root, &["status", "--porcelain"])?;
    Ok((!out.trim().is_empty(), out))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    Ok(String::from_utf8(out.stdout)?)
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if dir.ends_with("xtask") {
        dir.pop();
    }
    Ok(dir)
}
