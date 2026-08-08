//! Gate 2D0 cinematic presentation evaluator.

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::render_presentation::{load_presentation_spec, verify_beauty_png, write_beauty_png};
use crate::render_tier::DiagnosticRenderTier;
use relativity_render::{
    apply_exposure, authored_rgb16_bytes, khronos_pbr_neutral, luminance_axis_desat_v1,
    presentation_spec_digest, quantize_u16, srgb_oetf, DisplayEncodedRgb16, ExposureSpec,
    LinearRgb, PresentationMetrics, BIT_DEPTH_RGB16, CIE_TABLE_SHA256,
    GAMUT_MAPPER_ID_LUMINANCE_AXIS_DESAT_V1, PNG_GAMA_SRGB, SRGB_OETF_NUMERIC_ORACLE_V1,
    SRGB_OETF_ORACLE_ABS_TOL, TONE_MAPPER_ID_KHRONOS_PBR_NEUTRAL_V1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const APPROVED_BASE: &str = "c964c746fe3819627455a170e5e46b74731c0412";
const REF_FREQ_2B0: &str = "65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2";
const REF_EMISSION_2C0: &str = "5e3b15023df9bf3debed9666d65a3c762cfe83fe9885e7a5c8b3565dc19a383e";
const REF_SPECTRAL_2C0: &str = "136b1fbcc76beb08ea38aa24d16803d621da20bad5b7ebfecc7a13c260aa8dd1";
const REF_GRID_2C0: &str = "ceb3db28082bb357e50cac2635b221711bf79ea2806f2c25b60c61ca901162d5";
const REF_COLOR_2C1: &str = "16663188fad338c0fc8197dddd8268bd705f817b165a35853b16b211c7635793";
const REF_PAYLOAD_2C1: &str = "d317c517661a64f8ffdacead3dd222370056abc8eed81706d660bc4ebda81cf5";

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct PresentationReportFile {
    source_physical_color_digest: String,
    source_payload_sha256: String,
    presentation_spec_digest: String,
    presentation_frame_digest: String,
    middle_gray_luminance_cd_m2: f64,
    exposure_ev: f64,
    tone_mapper: String,
    gamut_mapper: String,
    png_srgb_intent: u8,
    png_gama: u32,
    png_roundtrip_ok: bool,
    metrics: PresentationMetrics,
}

#[derive(Serialize)]
struct Gate2d0Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    scientific_inheritance: String,
    presentation_pipeline: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    checks: Vec<Check>,
    smoke_serial: Option<PresentationReportFile>,
    smoke_parallel: Option<PresentationReportFile>,
    gate_run: Option<PresentationReportFile>,
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
        return Err("gate-2d0-presentation requires release evaluator".into());
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

    push_hermetic_presentation_checks(&mut checks);

    let out_root = root.join("artifacts/gate-2d0-presentation");
    std::fs::create_dir_all(&out_root)?;

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let auth_threads = available.clamp(1, 16);

    let smoke_serial = run_render(
        &root,
        "artifacts/gate-2d0-presentation/smoke-serial",
        DiagnosticRenderTier::Smoke,
        "serial",
        None,
    )?;
    let smoke_parallel = run_render(
        &root,
        "artifacts/gate-2d0-presentation/smoke-parallel",
        DiagnosticRenderTier::Smoke,
        "parallel",
        Some(2),
    )?;

    let digests_equal = smoke_serial.presentation_frame_digest
        == smoke_parallel.presentation_frame_digest
        && smoke_serial.presentation_spec_digest == smoke_parallel.presentation_spec_digest;
    push(
        &mut checks,
        "serial_parallel_presentation_digests",
        digests_equal,
        format!(
            "serial={} parallel={}",
            smoke_serial.presentation_frame_digest, smoke_parallel.presentation_frame_digest
        ),
    );

    let serial_png = root.join("artifacts/gate-2d0-presentation/smoke-serial/beauty-srgb16.png");
    let parallel_png =
        root.join("artifacts/gate-2d0-presentation/smoke-parallel/beauty-srgb16.png");
    let serial_bytes = decode_rgb16_bytes(&serial_png)?;
    let parallel_bytes = decode_rgb16_bytes(&parallel_png)?;
    push(
        &mut checks,
        "serial_parallel_rgb16_raster",
        serial_bytes == parallel_bytes,
        format!(
            "serial_bytes={} parallel_bytes={}",
            serial_bytes.len(),
            parallel_bytes.len()
        ),
    );

    let gate_run = run_render(
        &root,
        "artifacts/gate-2d0-presentation/gate-run-0",
        DiagnosticRenderTier::Gate,
        "parallel",
        Some(auth_threads),
    )?;

    // Scientific inheritance exact pins.
    push(
        &mut checks,
        "inherit_2c1_physical_color_digest",
        gate_run.source_physical_color_digest == REF_COLOR_2C1,
        gate_run.source_physical_color_digest.clone(),
    );
    push(
        &mut checks,
        "inherit_2c1_payload_sha256",
        gate_run.source_payload_sha256 == REF_PAYLOAD_2C1,
        gate_run.source_payload_sha256.clone(),
    );
    push(
        &mut checks,
        "inherit_cie_table_sha256",
        CIE_TABLE_SHA256 == "fa663e3535a7e0763a745993a1f0a192eb0275ac46ad2d1befd7626841e713c1",
        CIE_TABLE_SHA256.into(),
    );

    // Re-run Gate 2C1 evaluator inheritance via reading physical color render digests from
    // presentation report + verify frozen 2B0/2C0 by spawning gate-2c1 color report fields.
    // Presentation command already regenerates emission/spectral; pin via meta if present.
    let meta_path = root.join("artifacts/gate-2d0-presentation/gate-run-0/presentation-meta.json");
    let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(&meta_path)?)?;
    let freq = meta["source_frequency_digest"].as_str().unwrap_or("");
    let emission = meta["source_physical_emission_digest"]
        .as_str()
        .unwrap_or("");
    let spectral = meta["source_physical_spectral_digest"]
        .as_str()
        .unwrap_or("");
    let grid = meta["source_physical_spectral_grid_digest"]
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
        "inherit_2c0_spectral",
        spectral == REF_SPECTRAL_2C0,
        spectral.into(),
    );
    push(
        &mut checks,
        "inherit_2c0_spectral_grid",
        grid == REF_GRID_2C0,
        grid.into(),
    );

    push(
        &mut checks,
        "canonical_ev_zero",
        (gate_run.exposure_ev - 0.0).abs() < f64::EPSILON,
        format!("exposure_ev={}", gate_run.exposure_ev),
    );
    push(
        &mut checks,
        "canonical_tone_mapper",
        gate_run.tone_mapper == TONE_MAPPER_ID_KHRONOS_PBR_NEUTRAL_V1,
        gate_run.tone_mapper.clone(),
    );
    push(
        &mut checks,
        "canonical_gamut_mapper",
        gate_run.gamut_mapper == GAMUT_MAPPER_ID_LUMINANCE_AXIS_DESAT_V1,
        gate_run.gamut_mapper.clone(),
    );
    push(
        &mut checks,
        "png_srgb_intent_perceptual",
        gate_run.png_srgb_intent == 0,
        format!("{}", gate_run.png_srgb_intent),
    );
    push(
        &mut checks,
        "png_gama_45455",
        gate_run.png_gama == PNG_GAMA_SRGB,
        format!("{}", gate_run.png_gama),
    );
    push(
        &mut checks,
        "png_roundtrip_ok",
        gate_run.png_roundtrip_ok,
        format!("{}", gate_run.png_roundtrip_ok),
    );

    let beauty = root.join("artifacts/gate-2d0-presentation/gate-run-0/beauty-srgb16.png");
    let png_ok = verify_gate_png_file(&beauty)?;
    push(
        &mut checks,
        "gate_png_metadata_and_raster",
        png_ok,
        beauty.display().to_string(),
    );

    // Scope exclusions: no optional operators / auto-exp markers in preset.
    let preset_text =
        std::fs::read_to_string(root.join("presets/presentation/gargantua-cinematic-v1.toml"))?;
    let excluded = [
        "reinhard",
        "hable",
        "agx",
        "aces",
        "auto_exposure",
        "bloom",
        "glare",
        "hdr10",
        "pq",
        "hlg",
    ];
    let exclusion_ok = excluded
        .iter()
        .all(|k| !preset_text.to_lowercase().contains(k));
    push(
        &mut checks,
        "scope_exclusions_preset",
        exclusion_ok,
        "no deferred operators in presentation preset".into(),
    );

    let sci_ok = checks
        .iter()
        .any(|c| c.name.starts_with("inherit_") && c.status == "PASS")
        && checks
            .iter()
            .filter(|c| c.name.starts_with("inherit_"))
            .all(|c| c.status == "PASS");
    let pres_names = [
        "serial_parallel_presentation_digests",
        "serial_parallel_rgb16_raster",
        "canonical_ev_zero",
        "canonical_tone_mapper",
        "canonical_gamut_mapper",
        "png_srgb_intent_perceptual",
        "png_gama_45455",
        "png_roundtrip_ok",
        "gate_png_metadata_and_raster",
        "hermetic_exposure_middle_gray",
        "hermetic_gamut_hdr_identity",
        "hermetic_pbr_neutral_bounded",
        "hermetic_srgb_oetf_numeric_vectors",
        "hermetic_quantize",
        "scope_exclusions_preset",
    ];
    let pres_ok = pres_names.iter().all(|n| {
        checks
            .iter()
            .find(|c| c.name == *n)
            .is_some_and(|c| c.status == "PASS")
    });

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let mut report = Gate2d0Eval {
        gate: "gate-2d0-presentation".into(),
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
        presentation_pipeline: if pres_ok {
            "PRESENTATION_PIPELINE_PASS"
        } else {
            "PRESENTATION_PIPELINE_FAIL"
        }
        .into(),
        build,
        available_threads: available,
        authoritative_threads: auth_threads,
        checks,
        smoke_serial: Some(smoke_serial),
        smoke_parallel: Some(smoke_parallel),
        gate_run: Some(gate_run),
        content_digest_excluding_digest_field: String::new(),
    };
    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.result != "PASS" || !report.authoritative {
        return Err("gate-2d0-presentation FAIL".into());
    }
    Ok(())
}

fn push_hermetic_presentation_checks(checks: &mut Vec<Check>) {
    let e = ExposureSpec::new(2.411578982805191e9, 0.0).unwrap();
    let c = LinearRgb::new(
        2.411578982805191e9,
        2.411578982805191e9,
        2.411578982805191e9,
    )
    .unwrap();
    let o = apply_exposure(c, &e).unwrap();
    push(
        checks,
        "hermetic_exposure_middle_gray",
        (o.r - 0.18).abs() < 1e-12,
        format!("r={}", o.r),
    );

    let hdr = LinearRgb::new(0.2, 5.0, 12.0).unwrap();
    let (g, adj) = luminance_axis_desat_v1(hdr).unwrap();
    push(
        checks,
        "hermetic_gamut_hdr_identity",
        !adj && g == hdr,
        format!("adj={adj}"),
    );

    let (tm, _) = khronos_pbr_neutral(LinearRgb {
        r: 8.0,
        g: 8.0,
        b: 8.0,
    })
    .unwrap();
    push(
        checks,
        "hermetic_pbr_neutral_bounded",
        (0.0..=1.0).contains(&tm.r),
        format!("r={}", tm.r),
    );

    let mut srgb_oracle_ok = true;
    let mut srgb_oracle_detail = String::new();
    for &(x, expect) in SRGB_OETF_NUMERIC_ORACLE_V1 {
        match srgb_oetf(x) {
            Ok(y) if (y - expect).abs() <= SRGB_OETF_ORACLE_ABS_TOL => {}
            Ok(y) => {
                srgb_oracle_ok = false;
                srgb_oracle_detail = format!("x={x} got={y} expect={expect}");
                break;
            }
            Err(e) => {
                srgb_oracle_ok = false;
                srgb_oracle_detail = format!("x={x} err={e}");
                break;
            }
        }
    }
    if srgb_oracle_ok {
        srgb_oracle_detail = format!(
            "{} vectors ≤ {SRGB_OETF_ORACLE_ABS_TOL} abs",
            SRGB_OETF_NUMERIC_ORACLE_V1.len()
        );
    }
    push(
        checks,
        "hermetic_srgb_oetf_numeric_vectors",
        srgb_oracle_ok,
        srgb_oracle_detail,
    );

    push(
        checks,
        "hermetic_quantize",
        quantize_u16(0.0).unwrap() == 0 && quantize_u16(1.0).unwrap() == 65535,
        "0→0 1→65535".into(),
    );

    let _ = BIT_DEPTH_RGB16;
}

fn run_render(
    root: &Path,
    out: &str,
    tier: DiagnosticRenderTier,
    execution: &str,
    threads: Option<usize>,
) -> Result<PresentationReportFile, Box<dyn std::error::Error>> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root).args([
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
        match tier {
            DiagnosticRenderTier::Smoke => "smoke",
            DiagnosticRenderTier::Preview => "preview",
            DiagnosticRenderTier::Gate => "gate",
            DiagnosticRenderTier::Showcase => "showcase",
        },
        "--output-dir",
        out,
        "--execution",
        execution,
        "--require-release",
    ]);
    if let Some(t) = threads {
        cmd.args(["--threads", &t.to_string()]);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("render-presentation failed for {out}").into());
    }
    let report_path = root.join(out).join("presentation-report.json");
    let report: PresentationReportFile = serde_json::from_slice(&std::fs::read(report_path)?)?;
    Ok(report)
}

fn verify_gate_png_file(path: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    if info.width != 128 || info.height != 128 {
        return Ok(false);
    }
    if info.color_type != png::ColorType::Rgb || info.bit_depth != png::BitDepth::Sixteen {
        return Ok(false);
    }
    if info.srgb != Some(png::SrgbRenderingIntent::Perceptual) {
        return Ok(false);
    }
    if info.gama_chunk.map(|g| g.into_scaled()) != Some(PNG_GAMA_SRGB) {
        return Ok(false);
    }
    if info.chrm_chunk.is_some() || info.icc_profile.is_some() {
        return Ok(false);
    }
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("buf")?];
    let _ = reader.next_frame(&mut buf)?;
    Ok(true)
}

fn decode_rgb16_bytes(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("buf")?];
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
) -> Gate2d0Eval {
    Gate2d0Eval {
        gate: "gate-2d0-presentation".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        scientific_inheritance: "SCIENTIFIC_INHERITANCE_FAIL".into(),
        presentation_pipeline: "PRESENTATION_PIPELINE_FAIL".into(),
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

fn finalize(root: &Path, report: &mut Gate2d0Eval) -> Result<(), Box<dyn std::error::Error>> {
    report.content_digest_excluding_digest_field = String::new();
    let mut tmp = serde_json::to_value(&*report)?;
    if let Some(obj) = tmp.as_object_mut() {
        obj.remove("content_digest_excluding_digest_field");
    }
    let bytes = serde_json::to_vec(&tmp)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    report.content_digest_excluding_digest_field = format!("{:x}", h.finalize());

    let out = root.join("artifacts/gate-2d0-presentation");
    std::fs::create_dir_all(&out)?;
    std::fs::write(
        out.join("gate-2d0-evaluate.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let _ = (
        load_presentation_spec,
        write_beauty_png,
        verify_beauty_png,
        authored_rgb16_bytes,
        DisplayEncodedRgb16::BLACK,
        presentation_spec_digest,
    );
    Ok(())
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = git_stdout(root, &["status", "--porcelain"])?;
    let dirty = !out.trim().is_empty();
    Ok((dirty, out))
}

fn git_stdout(root: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git").current_dir(root).args(args).output()?;
    if !out.status.success() {
        return Err(format!("git {:?} failed", args).into());
    }
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
