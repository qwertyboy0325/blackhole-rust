//! Gate 2D1 scene appearance evaluator.

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::render_scene_appearance::SceneAppearanceReport;
use crate::render_tier::DiagnosticRenderTier;
use relativity_render::{presentation_spec_digest, CIE_TABLE_SHA256, PNG_GAMA_SRGB};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const APPROVED_BASE: &str = "b832e4778cdfad9f061970c71dbb1b82fdb31188";
const REF_COLOR_2C1: &str = "16663188fad338c0fc8197dddd8268bd705f817b165a35853b16b211c7635793";
const REF_PAYLOAD_2C1: &str = "d317c517661a64f8ffdacead3dd222370056abc8eed81706d660bc4ebda81cf5";
const REF_PRESENTATION_SPEC: &str =
    "e6639e75d67156852f8f064e7ef9f4f2b82ab8018b707399c851522780a6dd49";
const REF_PRESENTATION_FRAME: &str =
    "f8e103239a331796bd474ff121627eecd0781f31c840f46d9f2d3a85c8d1e87b";
const REF_FREQ_2B0: &str = "65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2";
const REF_EMISSION_2C0: &str = "5e3b15023df9bf3debed9666d65a3c762cfe83fe9885e7a5c8b3565dc19a383e";

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize)]
struct Gate2d1Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    scientific_inheritance: String,
    presentation_inheritance: String,
    appearance_pipeline: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    checks: Vec<Check>,
    identity_run: Option<SceneAppearanceReport>,
    gate_run: Option<SceneAppearanceReport>,
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
            format!("dirty: {dirty_detail}")
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
        return Err("gate-2d1-scene-appearance requires release evaluator".into());
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

    // Presentation inheritance: frozen 2D0 digests from shared post-exposure API.
    let pres = crate::render_presentation::load_presentation_spec(
        &root.join("presets/presentation/gargantua-cinematic-v1.toml"),
    )?;
    let spec_d = presentation_spec_digest(&pres)?;
    push(
        &mut checks,
        "inherit_2d0_presentation_spec_digest",
        spec_d == REF_PRESENTATION_SPEC,
        spec_d,
    );
    push(
        &mut checks,
        "inherit_cie_table_sha256",
        CIE_TABLE_SHA256 == "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1",
        CIE_TABLE_SHA256.into(),
    );

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let auth_threads = available.clamp(1, 16);
    std::fs::create_dir_all(root.join("artifacts/gate-2d1-scene-appearance"))?;

    // A5 identity scene must match Gate 2D0 exactly.
    let identity = run_scene(
        &root,
        "artifacts/gate-2d1-scene-appearance/identity",
        "presets/appearance/gargantua-scene-identity-v1.toml",
        DiagnosticRenderTier::Gate,
        "parallel",
        Some(auth_threads),
        false,
    )?;
    push(
        &mut checks,
        "identity_scene_flag",
        identity.identity_scene,
        format!("{}", identity.identity_scene),
    );
    push(
        &mut checks,
        "identity_presentation_frame_digest",
        identity.presentation_frame_digest == REF_PRESENTATION_FRAME,
        identity.presentation_frame_digest.clone(),
    );
    push(
        &mut checks,
        "identity_source_physical_color_digest",
        identity.source_physical_color_digest == REF_COLOR_2C1,
        identity.source_physical_color_digest.clone(),
    );
    push(
        &mut checks,
        "identity_payload_sha256",
        identity.source_payload_sha256 == REF_PAYLOAD_2C1,
        identity.source_payload_sha256.clone(),
    );

    // Compare identity beauty raster to Gate 2D0 gate beauty if present; else re-render 2D0.
    let d0_png = root.join("artifacts/gate-2d0-presentation/gate-run-0/beauty-srgb16.png");
    if !d0_png.is_file() {
        let _ = Command::new("cargo")
            .current_dir(&root)
            .args([
                "run",
                "--release",
                "-p",
                "xtask",
                "--",
                "render-presentation",
                "--preset",
                "presets/gargantua-physical-v1.toml",
                "--presentation",
                "presets/presentation/gargantua-cinematic-v1.toml",
                "--tier",
                "gate",
                "--output-dir",
                "artifacts/gate-2d0-presentation/gate-run-0",
                "--execution",
                "parallel",
                "--threads",
                &auth_threads.to_string(),
                "--require-release",
            ])
            .status()?;
    }
    let id_png = root.join("artifacts/gate-2d1-scene-appearance/identity/beauty-scene-srgb16.png");
    let d0_bytes = std::fs::read(&d0_png)?;
    let id_bytes = std::fs::read(&id_png)?;
    // Compare decoded RGB16 via roundtrip already in report; also compare file payloads after
    // stripping is hard — re-decode both with same path using verify via bit compare of authored
    // raster from reports is stronger: re-read via png decode helper.
    let d0_raster = decode_rgb16_bytes(&d0_png)?;
    let id_raster = decode_rgb16_bytes(&id_png)?;
    push(
        &mut checks,
        "identity_rgb16_bit_exact_vs_2d0",
        d0_raster == id_raster,
        format!(
            "d0_bytes={} id_bytes={} file_d0={} file_id={}",
            d0_raster.len(),
            id_raster.len(),
            d0_bytes.len(),
            id_bytes.len()
        ),
    );

    let gate_run = run_scene(
        &root,
        "artifacts/gate-2d1-scene-appearance/gate-run-0",
        "presets/appearance/gargantua-scene-v1.toml",
        DiagnosticRenderTier::Gate,
        "parallel",
        Some(auth_threads),
        true,
    )?;
    let gate_serial = run_scene(
        &root,
        "artifacts/gate-2d1-scene-appearance/gate-serial",
        "presets/appearance/gargantua-scene-v1.toml",
        DiagnosticRenderTier::Smoke,
        "serial",
        None,
        false,
    )?;
    let gate_parallel = run_scene(
        &root,
        "artifacts/gate-2d1-scene-appearance/gate-parallel",
        "presets/appearance/gargantua-scene-v1.toml",
        DiagnosticRenderTier::Smoke,
        "parallel",
        Some(2),
        false,
    )?;
    push(
        &mut checks,
        "serial_parallel_scene_digests",
        gate_serial.scene_appearance_digest == gate_parallel.scene_appearance_digest
            && gate_serial.presentation_frame_digest == gate_parallel.presentation_frame_digest,
        format!(
            "serial={} parallel={}",
            gate_serial.scene_appearance_digest, gate_parallel.scene_appearance_digest
        ),
    );

    let meta_path =
        root.join("artifacts/gate-2d1-scene-appearance/gate-run-0/scene-appearance-meta.json");
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(&meta_path)?)?;
    let freq = meta["source_frequency_digest"].as_str().unwrap_or("");
    let emission = meta["source_physical_emission_digest"]
        .as_str()
        .unwrap_or("");
    push(
        &mut checks,
        "inherit_2b0_frequency",
        freq == REF_FREQ_2B0,
        freq.into(),
    );
    push(
        &mut checks,
        "inherit_2c0_emission",
        emission == REF_EMISSION_2C0,
        emission.into(),
    );
    push(
        &mut checks,
        "inherit_2c1_physical_color_digest_gate",
        gate_run.source_physical_color_digest == REF_COLOR_2C1,
        gate_run.source_physical_color_digest.clone(),
    );
    push(
        &mut checks,
        "inherit_2c1_payload_sha256_gate",
        gate_run.source_payload_sha256 == REF_PAYLOAD_2C1,
        gate_run.source_payload_sha256.clone(),
    );
    push(
        &mut checks,
        "mean_claim_annular_only",
        meta["mean_preservation_claim"] == "ANNULAR_APPEARANCE_MEAN_PRESERVING",
        meta["mean_preservation_claim"].to_string(),
    );
    push(
        &mut checks,
        "finite_boundary_celestial_convention",
        meta["celestial_convention"] == "finite-oblate-ks-boundary-uv-v1",
        meta["celestial_convention"].to_string(),
    );
    push(
        &mut checks,
        "png_roundtrip_ok",
        gate_run.png_roundtrip_ok,
        format!("{}", gate_run.png_roundtrip_ok),
    );
    push(
        &mut checks,
        "gate_not_identity",
        !gate_run.identity_scene,
        format!("{}", gate_run.identity_scene),
    );

    let beauty =
        root.join("artifacts/gate-2d1-scene-appearance/gate-run-0/beauty-scene-srgb16.png");
    push(
        &mut checks,
        "beauty_artifact_present",
        beauty.is_file(),
        beauty.display().to_string(),
    );

    let app_preset =
        std::fs::read_to_string(root.join("presets/appearance/gargantua-scene-v1.toml"))?;
    push(
        &mut checks,
        "a1_angular_sigma_present",
        app_preset.contains("angular_sigma_rad"),
        "angular_sigma_rad in appearance preset".into(),
    );
    push(
        &mut checks,
        "a2_radial_envelope_id",
        app_preset.contains("raised-cosine-radial-envelope-v1"),
        "raised-cosine-radial-envelope-v1".into(),
    );
    let banned = [
        "energy conserving",
        "luminosity preserving",
        "flux preserving",
    ];
    push(
        &mut checks,
        "a3_no_luminosity_claim_in_preset",
        banned
            .iter()
            .all(|b| !app_preset.to_lowercase().contains(b)),
        "no luminosity/energy claim wording".into(),
    );

    let sci_ok = [
        "inherit_2b0_frequency",
        "inherit_2c0_emission",
        "inherit_2c1_physical_color_digest_gate",
        "inherit_2c1_payload_sha256_gate",
        "inherit_cie_table_sha256",
        "identity_source_physical_color_digest",
        "identity_payload_sha256",
    ]
    .iter()
    .all(|n| {
        checks
            .iter()
            .find(|c| c.name == *n)
            .is_some_and(|c| c.status == "PASS")
    });
    let pres_ok = [
        "inherit_2d0_presentation_spec_digest",
        "identity_presentation_frame_digest",
        "identity_rgb16_bit_exact_vs_2d0",
    ]
    .iter()
    .all(|n| {
        checks
            .iter()
            .find(|c| c.name == *n)
            .is_some_and(|c| c.status == "PASS")
    });
    let app_ok = [
        "serial_parallel_scene_digests",
        "mean_claim_annular_only",
        "finite_boundary_celestial_convention",
        "png_roundtrip_ok",
        "beauty_artifact_present",
        "a1_angular_sigma_present",
        "a2_radial_envelope_id",
        "a3_no_luminosity_claim_in_preset",
        "gate_not_identity",
        "identity_scene_flag",
    ]
    .iter()
    .all(|n| {
        checks
            .iter()
            .find(|c| c.name == *n)
            .is_some_and(|c| c.status == "PASS")
    });

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let mut report = Gate2d1Eval {
        gate: "gate-2d1-scene-appearance".into(),
        result: if all_pass { "PASS" } else { "FAIL" }.into(),
        authoritative: !dirty && self_release && all_pass,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        scientific_inheritance: if sci_ok {
            "SCIENTIFIC_INHERITANCE_PASS"
        } else {
            "SCIENTIFIC_INHERITANCE_FAIL"
        }
        .into(),
        presentation_inheritance: if pres_ok {
            "PRESENTATION_INHERITANCE_PASS"
        } else {
            "PRESENTATION_INHERITANCE_FAIL"
        }
        .into(),
        appearance_pipeline: if app_ok {
            "APPEARANCE_PIPELINE_PASS"
        } else {
            "APPEARANCE_PIPELINE_FAIL"
        }
        .into(),
        build,
        available_threads: available,
        authoritative_threads: auth_threads,
        checks,
        identity_run: Some(identity),
        gate_run: Some(gate_run),
        content_digest_excluding_digest_field: String::new(),
    };
    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.result != "PASS" || !report.authoritative {
        return Err("gate-2d1-scene-appearance FAIL".into());
    }
    Ok(())
}

fn run_scene(
    root: &Path,
    out: &str,
    appearance: &str,
    tier: DiagnosticRenderTier,
    execution: &str,
    threads: Option<usize>,
    write_env_reference: bool,
) -> Result<SceneAppearanceReport, Box<dyn std::error::Error>> {
    let tier_s = match tier {
        DiagnosticRenderTier::Smoke => "smoke",
        DiagnosticRenderTier::Preview => "preview",
        DiagnosticRenderTier::Gate => "gate",
        DiagnosticRenderTier::Showcase => "showcase",
    };
    let mut args = vec![
        "run",
        "--release",
        "-p",
        "xtask",
        "--",
        "render-scene-appearance",
        "--preset",
        "presets/gargantua-physical-v1.toml",
        "--appearance",
        appearance,
        "--presentation",
        "presets/presentation/gargantua-cinematic-v1.toml",
        "--tier",
        tier_s,
        "--output-dir",
        out,
        "--execution",
        execution,
        "--require-release",
    ];
    let threads_s = threads.map(|t| t.to_string());
    if let Some(ref t) = threads_s {
        args.push("--threads");
        args.push(t);
    }
    if write_env_reference {
        args.push("--write-env-reference");
    } else {
        args.push("--no-env-reference");
    }
    let status = Command::new("cargo")
        .current_dir(root)
        .args(&args)
        .status()?;
    if !status.success() {
        return Err(format!("render-scene-appearance failed for {out}").into());
    }
    let report_path = root.join(out).join("appearance-report.json");
    let report_bytes = std::fs::read(report_path)?;
    let report: SceneAppearanceReport = serde_json::from_slice(&report_bytes)?;
    Ok(report)
}

fn decode_rgb16_bytes(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("PNG buffer")?];
    let info = reader.next_frame(&mut buf)?;
    Ok(buf[..info.buffer_size()].to_vec())
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
    push(checks, name, status.success(), format!("exit={status}"));
    Ok(())
}

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2d1Eval {
    Gate2d1Eval {
        gate: "gate-2d1-scene-appearance".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        scientific_inheritance: "SCIENTIFIC_INHERITANCE_FAIL".into(),
        presentation_inheritance: "PRESENTATION_INHERITANCE_FAIL".into(),
        appearance_pipeline: "APPEARANCE_PIPELINE_FAIL".into(),
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        checks,
        identity_run: None,
        gate_run: None,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut Gate2d1Eval) -> Result<(), Box<dyn std::error::Error>> {
    let mut clone = serde_json::to_value(&*report)?;
    if let Some(obj) = clone.as_object_mut() {
        obj.remove("content_digest_excluding_digest_field");
    }
    let bytes = serde_json::to_vec(&clone)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    report.content_digest_excluding_digest_field = format!("{:x}", h.finalize());
    let out = root.join("artifacts/gate-2d1-scene-appearance/evaluate-report.json");
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(&out, serde_json::to_string_pretty(report)?)?;
    Ok(())
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // Ignore untracked artifacts/
    let tracked = text
        .lines()
        .filter(|l| !l.starts_with("?? artifacts/") && !l.contains(" artifacts/"))
        .filter(|l| {
            let path = l.get(3..).unwrap_or("");
            !path.starts_with("artifacts/")
        })
        .collect::<Vec<_>>();
    Ok((!tracked.is_empty(), tracked.join("; ")))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    Ok(String::from_utf8(out.stdout)?)
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("xtask").is_dir() {
            return Ok(dir);
        }
        dir = dir
            .parent()
            .ok_or("workspace root not found")?
            .to_path_buf();
    }
}

// Silence unused import warning if PNG_GAMA unused in this file.
#[allow(dead_code)]
fn _png_gama() -> u32 {
    PNG_GAMA_SRGB
}
