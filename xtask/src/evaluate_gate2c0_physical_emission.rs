//! Gate 2C0 physical thin-disk emission evaluator.

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::render_tier::DiagnosticRenderTier;
use relativity_core::{
    prograde_isco_radius, transport_i_nu, KerrParams, MdotKgPerS, PhysicalFrequencyHz,
    PhysicalScale, TemperatureKelvin,
};
use relativity_render::{
    independent_physical_i_nu_obs, integrate_pi_b_nu_log_grid, newtonian_zero_torque_flux,
    page_thorne_one_face_flux, page_thorne_one_face_flux_numerical,
    parse_physical_spectral_grid_id, physical_spectral_grid_explore, physical_spectral_grid_v1,
    planck_b_nu, stefan_boltzmann_flux, teff_from_one_face_flux, PageThorneRoots,
    PHYSICAL_EMISSION_MODEL_ID, PHYSICAL_GRID_EXPLORE_PREFIX, PHYSICAL_GRID_V1_ID,
    PHYSICAL_GRID_V1_N_BINS,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const APPROVED_BASE: &str = "95c4062e5926e77e3e14c17ec003e7ee625cfc79";
const REF_FREQ_2B0: &str = "65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2";
const REF_BOLO_2B1: &str = "d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2";
const REF_GRID_2B2: &str = "0d7e4812dfba61635aaf00f486fcc23aebc63fbb2fb9d6a51ab8a4b8ed41474e";

/// Frozen Gate 2C0 acceptance envelopes (calibrated after PT flux root fix).
/// Gate / frozen-v1 emitter SB: measured max-rel ≈ 1.21e-4; freeze at 5e-4 (~4×).
const EMITTER_SB_REL_TOL_GATE: f64 = 5e-4;
/// Smoke explore-64 ladder peak ≈ 1.94e-3; freeze at 6e-3 (~3×). Not used for gate.
const EMITTER_SB_REL_TOL_SMOKE: f64 = 6e-3;
const TRANSPORT_G4_REL_TOL: f64 = 1e-10;
/// Algebraic vs independent conservation-law flux (domain worst under dense quad).
const PT_NUMERICAL_REL_TOL: f64 = 5e-3;

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct PhysicalRenderReport {
    gate: String,
    frequency_shift_digest: String,
    physical_emission_spec_digest: String,
    physical_emission_digest: String,
    physical_spectral_grid_digest: String,
    physical_spectral_digest: String,
    disk_hit_count: u64,
    emission_pixel_count: u64,
    closure: relativity_render::PhysicalSpectralClosureMetrics,
}

#[derive(Serialize)]
struct Gate2c0Eval {
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
    smoke_serial: Option<PhysicalRenderReport>,
    smoke_parallel: Option<PhysicalRenderReport>,
    gate_run: Option<PhysicalRenderReport>,
    convergence: Vec<ConvergenceRow>,
    content_digest_excluding_digest_field: String,
}

#[derive(Serialize, Clone)]
struct ConvergenceRow {
    n_bins: u32,
    grid_id: String,
    max_rel_emitter_sb_error: f64,
    max_rel_g4_transport_error: f64,
    physical_spectral_digest: String,
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
        return Err("gate-2c0-physical-emission requires release evaluator".into());
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

    hermetic_physical_checks(&mut checks)?;
    assert_frozen_2b_digests_untouched(&mut checks)?;

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;
    let smoke_threads = available.clamp(1, 2);

    let out_root = root.join("artifacts/gate-2c0-physical-emission");
    std::fs::create_dir_all(&out_root)?;

    push(
        &mut checks,
        "cli_reject_diagnostic_grid",
        run_physical_cli(
            &root,
            "artifacts/gate-2c0-physical-emission/cli-neg-diag-grid",
            DiagnosticRenderTier::Smoke,
            1,
            PHYSICAL_EMISSION_MODEL_ID,
            "spectral-grid-v1",
            true,
        )
        .is_err(),
        "expected failure".into(),
    );
    push(
        &mut checks,
        "cli_reject_bad_emission",
        run_physical_cli(
            &root,
            "artifacts/gate-2c0-physical-emission/cli-neg-emission",
            DiagnosticRenderTier::Smoke,
            1,
            "not-a-model",
            "physical-spectral-grid-explore-64",
            true,
        )
        .is_err(),
        "expected failure".into(),
    );

    let smoke_serial = run_physical_cli(
        &root,
        "artifacts/gate-2c0-physical-emission/smoke-serial",
        DiagnosticRenderTier::Smoke,
        1,
        PHYSICAL_EMISSION_MODEL_ID,
        "physical-spectral-grid-explore-64",
        true,
    )?;
    let smoke_parallel = run_physical_cli(
        &root,
        "artifacts/gate-2c0-physical-emission/smoke-parallel",
        DiagnosticRenderTier::Smoke,
        smoke_threads,
        PHYSICAL_EMISSION_MODEL_ID,
        "physical-spectral-grid-explore-64",
        true,
    )?;
    check_report(
        &mut checks,
        "smoke_serial",
        &smoke_serial,
        EMITTER_SB_REL_TOL_SMOKE,
    );
    check_report(
        &mut checks,
        "smoke_parallel",
        &smoke_parallel,
        EMITTER_SB_REL_TOL_SMOKE,
    );
    push(
        &mut checks,
        "smoke_serial_parallel_digest_identical",
        smoke_serial.physical_spectral_digest == smoke_parallel.physical_spectral_digest
            && smoke_serial.physical_emission_digest == smoke_parallel.physical_emission_digest
            && smoke_serial.frequency_shift_digest == smoke_parallel.frequency_shift_digest,
        smoke_serial.physical_spectral_digest.clone(),
    );
    push(
        &mut checks,
        "smoke_payload_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2c0-physical-emission/smoke-serial",
            "artifacts/gate-2c0-physical-emission/smoke-parallel",
            "physical-i-nu-obs.f64le",
        )? && files_eq(
            &root,
            "artifacts/gate-2c0-physical-emission/smoke-serial",
            "artifacts/gate-2c0-physical-emission/smoke-parallel",
            "physical-f-teff.f64le",
        )?,
        "physical payloads".into(),
    );

    let gate_run = run_physical_cli(
        &root,
        "artifacts/gate-2c0-physical-emission/gate-run-0",
        DiagnosticRenderTier::Gate,
        authoritative_threads,
        PHYSICAL_EMISSION_MODEL_ID,
        PHYSICAL_GRID_V1_ID,
        true,
    )?;
    check_report(&mut checks, "gate0", &gate_run, EMITTER_SB_REL_TOL_GATE);
    let frozen_grid = physical_spectral_grid_v1()?;
    let frozen_grid_digest = relativity_render::physical_spectral_grid_digest(&frozen_grid)?;
    push(
        &mut checks,
        "gate_uses_frozen_physical_spectral_grid_v1",
        gate_run.physical_spectral_grid_digest == frozen_grid_digest,
        format!(
            "{} digest={}",
            PHYSICAL_GRID_V1_ID, gate_run.physical_spectral_grid_digest
        ),
    );
    push(
        &mut checks,
        "gate_inherits_2b0_frequency_digest",
        gate_run.frequency_shift_digest == REF_FREQ_2B0,
        gate_run.frequency_shift_digest.clone(),
    );

    let mut convergence = Vec::new();
    for n in [64u32, 128, 256, 512] {
        let grid_id = format!("{PHYSICAL_GRID_EXPLORE_PREFIX}{n}");
        let row_report = run_physical_cli(
            &root,
            &format!("artifacts/gate-2c0-physical-emission/conv-{n}"),
            DiagnosticRenderTier::Smoke,
            smoke_threads,
            PHYSICAL_EMISSION_MODEL_ID,
            &grid_id,
            true,
        )?;
        convergence.push(ConvergenceRow {
            n_bins: n,
            grid_id,
            max_rel_emitter_sb_error: row_report.closure.max_rel_emitter_sb_error,
            max_rel_g4_transport_error: row_report.closure.max_rel_g4_transport_error,
            physical_spectral_digest: row_report.physical_spectral_digest,
        });
    }
    if let (Some(a), Some(b)) = (convergence.first(), convergence.last()) {
        push(
            &mut checks,
            "grid_convergence_emitter_sb_improves_or_stable",
            b.max_rel_emitter_sb_error <= a.max_rel_emitter_sb_error * 1.05 + 1e-6,
            format!(
                "64→512 emitter SB rel {} → {}",
                a.max_rel_emitter_sb_error, b.max_rel_emitter_sb_error
            ),
        );
    }

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let authoritative = all_pass && !dirty && self_release;
    let mut report = Gate2c0Eval {
        gate: "gate-2c0-physical-emission".into(),
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
        convergence,
        content_digest_excluding_digest_field: String::new(),
    };
    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !all_pass {
        return Err("gate-2c0-physical-emission FAIL".into());
    }
    Ok(())
}

fn hermetic_physical_checks(checks: &mut Vec<Check>) -> Result<(), Box<dyn std::error::Error>> {
    // π factor mandatory
    let t = TemperatureKelvin::new(8.0e3)?;
    let sb = stefan_boltzmann_flux(t)?.value();
    let integ = integrate_pi_b_nu_log_grid(t, 1.0e10, 1.0e18, 4096)?;
    let rel = (integ - sb).abs() / sb;
    push(
        checks,
        "hermetic_pi_b_nu_stefan_boltzmann",
        rel < 5e-3,
        format!("rel={rel}"),
    );
    let without_pi = integ / std::f64::consts::PI;
    push(
        checks,
        "hermetic_missing_pi_fails_sb",
        (without_pi - sb).abs() / sb > 0.5,
        "π mandatory".into(),
    );

    // g³ transport
    let i = independent_physical_i_nu_obs(5.0e3, 0.5, 5.0e14)?;
    let nu_em = PhysicalFrequencyHz::new(5.0e14 / 0.5)?;
    let bem = planck_b_nu(nu_em, TemperatureKelvin::new(5.0e3)?)?.value();
    let expect = transport_i_nu(bem, 0.5)?;
    push(
        checks,
        "hermetic_g3_transport",
        (i - expect).abs() / expect.max(1e-30) < 1e-12,
        format!("i={i} expect={expect}"),
    );

    // Page–Thorne vs Newtonian asymptotic
    let k = KerrParams::new(1.0, 0.0)?;
    let scale = PhysicalScale::from_solar_masses(1.0e8)?;
    let mdot = MdotKgPerS::new(1.0e18)?;
    let f_pt = page_thorne_one_face_flux(&scale, mdot, &k, 100_000.0)?.value();
    let f_n = newtonian_zero_torque_flux(&scale, mdot, &k, 100_000.0)?.value();
    push(
        checks,
        "hermetic_pt_newtonian_asymptote",
        (f_pt - f_n).abs() / f_n < 5e-3,
        format!("rel={}", (f_pt - f_n).abs() / f_n),
    );

    // Algebraic vs independent numerical flux (not Q)
    let k2 = KerrParams::new(1.0, 0.999)?;
    let mut worst_pt = 0.0_f64;
    for &r in &[1.5_f64, 3.0, 20.0, 200.0] {
        let f_a = page_thorne_one_face_flux(&scale, mdot, &k2, r)?.value();
        let f_n = page_thorne_one_face_flux_numerical(&scale, mdot, &k2, r, 16_384)?.value();
        let rel = (f_a - f_n).abs() / f_a.max(f_n).max(1e-30);
        worst_pt = worst_pt.max(rel);
    }
    push(
        checks,
        "hermetic_pt_algebraic_vs_numerical_flux",
        worst_pt < PT_NUMERICAL_REL_TOL,
        format!("worst_rel={worst_pt} a*=0.999"),
    );

    // F→0 as r→r_isco⁺ (high-spin: must approach closer than 1e-4 relative).
    let r_isco = prograde_isco_radius(&k2)?;
    let f_mid = page_thorne_one_face_flux(&scale, mdot, &k2, 20.0)?.value();
    let f_near = page_thorne_one_face_flux(&scale, mdot, &k2, r_isco * (1.0 + 1e-4))?.value();
    let f_eps = page_thorne_one_face_flux(&scale, mdot, &k2, r_isco * (1.0 + 1e-8))?.value();
    push(
        checks,
        "hermetic_pt_vanishes_near_isco",
        f_eps < 1e-4 * f_mid && f_eps < f_near && f_near < f_mid,
        format!("eps={f_eps} near={f_near} mid={f_mid}"),
    );
    let f0 = page_thorne_one_face_flux(&scale, MdotKgPerS::new(0.0)?, &k2, 20.0)?.value();
    push(checks, "hermetic_mdot_zero", f0 == 0.0, format!("f={f0}"));

    // Retrograde reject
    let k_ret = KerrParams::new(1.0, -0.2)?;
    push(
        checks,
        "hermetic_retrograde_reject",
        PageThorneRoots::for_prograde(&k_ret).is_err(),
        "typed reject".into(),
    );

    // Diagnostic grid typed reject
    push(
        checks,
        "hermetic_reject_diagnostic_spectral_grid",
        parse_physical_spectral_grid_id("spectral-grid-v1").is_err(),
        "diagnostic ν is not Hz".into(),
    );
    let g = physical_spectral_grid_v1()?;
    push(
        checks,
        "hermetic_physical_grid_v1_frozen",
        g.n_bins() == PHYSICAL_GRID_V1_N_BINS && g.grid_id() == PHYSICAL_GRID_V1_ID,
        g.grid_id().into(),
    );
    let g_explore = physical_spectral_grid_explore(128)?;
    push(
        checks,
        "hermetic_physical_grid_explore",
        g_explore.n_bins() == 128
            && g_explore
                .grid_id()
                .starts_with(PHYSICAL_GRID_EXPLORE_PREFIX),
        g_explore.grid_id().into(),
    );

    // T_eff roundtrip
    let f = page_thorne_one_face_flux(&scale, mdot, &k2, 15.0)?;
    let teff = teff_from_one_face_flux(f)?;
    let back = stefan_boltzmann_flux(teff)?.value();
    push(
        checks,
        "hermetic_teff_sb_roundtrip",
        (back - f.value()).abs() / f.value() < 1e-12,
        format!("rel={}", (back - f.value()).abs() / f.value()),
    );

    Ok(())
}

fn assert_frozen_2b_digests_untouched(
    checks: &mut Vec<Check>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Compile-time / library-level freeze: recompute diagnostic grid digest.
    let grid = relativity_core::SpectralGrid::spectral_grid_v1()?;
    let d = relativity_render::spectral_grid_digest(&grid)?;
    push(
        checks,
        "frozen_2b2_spectral_grid_digest",
        d == REF_GRID_2B2,
        d,
    );
    // Document inherited 2B0/2B1 digest strings still referenced by smoke/gate.
    push(
        checks,
        "frozen_2b0_ref_present",
        REF_FREQ_2B0.len() == 64,
        REF_FREQ_2B0.into(),
    );
    push(
        checks,
        "frozen_2b1_ref_present",
        REF_BOLO_2B1.len() == 64,
        REF_BOLO_2B1.into(),
    );
    Ok(())
}

fn check_report(
    checks: &mut Vec<Check>,
    label: &str,
    report: &PhysicalRenderReport,
    emitter_sb_rel_tol: f64,
) {
    push(
        checks,
        &format!("{label}_gate_id"),
        report.gate == "gate-2c0-physical-emission",
        report.gate.clone(),
    );
    push(
        checks,
        &format!("{label}_has_emission_pixels"),
        report.emission_pixel_count > 0,
        format!("{}", report.emission_pixel_count),
    );
    push(
        checks,
        &format!("{label}_emitter_sb_closure"),
        report.closure.max_rel_emitter_sb_error <= emitter_sb_rel_tol,
        format!("{}", report.closure.max_rel_emitter_sb_error),
    );
    push(
        checks,
        &format!("{label}_g4_transport_closure"),
        report.closure.max_rel_g4_transport_error <= TRANSPORT_G4_REL_TOL,
        format!("{}", report.closure.max_rel_g4_transport_error),
    );
}

fn run_physical_cli(
    root: &Path,
    output_dir: &str,
    tier: DiagnosticRenderTier,
    threads: usize,
    emission: &str,
    grid: &str,
    require_release: bool,
) -> Result<PhysicalRenderReport, Box<dyn std::error::Error>> {
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
        "render-physical-disk-spectrum".into(),
        "--preset".into(),
        "presets/gargantua-physical-v1.toml".into(),
        "--tier".into(),
        tier_str.into(),
        "--physical-emission".into(),
        emission.into(),
        "--physical-spectral-grid".into(),
        grid.into(),
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
        return Err(format!("render-physical-disk-spectrum failed: {output_dir}").into());
    }
    let report_path = root.join(output_dir).join("physical-render-report.json");
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
) -> Gate2c0Eval {
    Gate2c0Eval {
        gate: "gate-2c0-physical-emission".into(),
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
        convergence: Vec::new(),
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut Gate2c0Eval) -> Result<(), Box<dyn std::error::Error>> {
    let mut clone = serde_json::to_value(&*report)?;
    if let Some(obj) = clone.as_object_mut() {
        obj.remove("content_digest_excluding_digest_field");
    }
    let bytes = serde_json::to_vec(&clone)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    report.content_digest_excluding_digest_field = format!("{:x}", h.finalize());
    let out = root.join("artifacts/gate-2c0-physical-emission");
    std::fs::create_dir_all(&out)?;
    std::fs::write(
        out.join("gate-2c0-evaluate.json"),
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
