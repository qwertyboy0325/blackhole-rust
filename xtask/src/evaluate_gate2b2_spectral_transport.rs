//! Gate 2B2 spectral transport evaluator.

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::render_tier::DiagnosticRenderTier;
use relativity_core::{
    i_lambda_from_i_nu, transport_i_nu, wavelength_from_frequency, Frequency, SpectralGrid,
};
use relativity_render::{
    compute_bolometric_closure, continuum_normalization, diagnostic_gaussian_line_v1,
    diagnostic_lognormal_continuum_v1, diagnostic_spectrum_spec_digest, evaluate_continuum_phi,
    evaluate_line_fixture, independent_i_nu_obs, spectral_grid_digest, CONTINUUM_SPECTRUM_ID,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const REF_FREQ: &str = "65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2";
const REF_BOLO: &str = "d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2";
const REF_EMISSION_SPEC: &str = "95347496d2ade139a6002bb5d2ef70a4ba4b77085eac4a7b6232a49f9fd1c0db";
const APPROVED_BASE: &str = "2f41bfecc2b04729c0205953585110b3274fabe9";

/// Frozen Gate 2B2 error budget (owner closure `5203577417`).
///
/// Derived from clean PASS evidence at `07c5111`: gate 128² × 64-bin max rel
/// closure ≈ `6.32e-4`. Envelope is ~3× that observed peak — not loosened for
/// float noise.
const CLOSURE_REL_TOL: f64 = 2e-3;
const CLOSURE_ABS_TOL: f64 = 2e-3;

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct SpectralRenderReport {
    gate: String,
    frequency_shift_digest: String,
    bolometric_digest: String,
    continuum_digest: String,
    spectral_grid_digest: String,
    spectral_digest: String,
    disk_hit_count: u64,
    closure: relativity_render::SpectralClosureMetrics,
}

#[derive(Serialize)]
struct Gate2b2Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    continuum_digest: String,
    spectral_grid_digest: String,
    checks: Vec<Check>,
    smoke_serial: Option<SpectralRenderReport>,
    smoke_parallel: Option<SpectralRenderReport>,
    gate_run: Option<SpectralRenderReport>,
    convergence: Vec<ConvergenceRow>,
    content_digest_excluding_digest_field: String,
}

#[derive(Serialize, Clone)]
struct ConvergenceRow {
    n_bins: u32,
    grid_id: String,
    max_rel_observed_closure_error: f64,
    max_rel_emitted_closure_error: f64,
    spectral_digest: String,
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
        return Err("gate-2b2-spectral-transport requires release evaluator".into());
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

    hermetic_spectral_checks(&mut checks)?;

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;
    let smoke_threads = available.clamp(1, 2);

    let out_root = root.join("artifacts/gate-2b2-spectral-transport");
    std::fs::create_dir_all(&out_root)?;

    let continuum = diagnostic_lognormal_continuum_v1();
    let continuum_digest = diagnostic_spectrum_spec_digest(&continuum);
    let grid_v1 = SpectralGrid::spectral_grid_v1()?;
    let grid_digest = spectral_grid_digest(&grid_v1)?;
    push(
        &mut checks,
        "continuum_spectrum_id",
        continuum.spectrum_id == CONTINUUM_SPECTRUM_ID,
        continuum_digest.clone(),
    );
    push(
        &mut checks,
        "spectral_grid_v1_bins",
        grid_v1.n_bins() == SpectralGrid::V1_N_BINS,
        format!("{} bins", grid_v1.n_bins()),
    );

    // CLI negatives
    push(
        &mut checks,
        "cli_reject_bad_spectrum",
        run_spectrum_cli(
            &root,
            "artifacts/gate-2b2-spectral-transport/cli-neg-spectrum",
            DiagnosticRenderTier::Smoke,
            1,
            "not-a-spectrum",
            SpectralGrid::V1_ID,
            true,
        )
        .is_err(),
        "expected failure".into(),
    );
    push(
        &mut checks,
        "cli_reject_bad_grid",
        run_spectrum_cli(
            &root,
            "artifacts/gate-2b2-spectral-transport/cli-neg-grid",
            DiagnosticRenderTier::Smoke,
            1,
            CONTINUUM_SPECTRUM_ID,
            "linear-grid-v0",
            true,
        )
        .is_err(),
        "expected failure".into(),
    );

    let smoke_serial = run_spectrum_cli(
        &root,
        "artifacts/gate-2b2-spectral-transport/smoke-serial",
        DiagnosticRenderTier::Smoke,
        1,
        CONTINUUM_SPECTRUM_ID,
        SpectralGrid::V1_ID,
        true,
    )?;
    let smoke_parallel = run_spectrum_cli(
        &root,
        "artifacts/gate-2b2-spectral-transport/smoke-parallel",
        DiagnosticRenderTier::Smoke,
        smoke_threads,
        CONTINUUM_SPECTRUM_ID,
        SpectralGrid::V1_ID,
        true,
    )?;
    check_report(&mut checks, "smoke_serial", &smoke_serial, false);
    check_report(&mut checks, "smoke_parallel", &smoke_parallel, false);
    push(
        &mut checks,
        "smoke_serial_parallel_spectral_digest_identical",
        smoke_serial.spectral_digest == smoke_parallel.spectral_digest
            && smoke_serial.frequency_shift_digest == smoke_parallel.frequency_shift_digest
            && smoke_serial.bolometric_digest == smoke_parallel.bolometric_digest,
        smoke_serial.spectral_digest.clone(),
    );
    push(
        &mut checks,
        "smoke_payload_byte_identical",
        files_eq(
            &root,
            "artifacts/gate-2b2-spectral-transport/smoke-serial",
            "artifacts/gate-2b2-spectral-transport/smoke-parallel",
            "spectral-i-nu-obs.f64le",
        )?,
        "spectral-i-nu-obs.f64le".into(),
    );

    let gate_run = run_spectrum_cli(
        &root,
        "artifacts/gate-2b2-spectral-transport/gate-run-0",
        DiagnosticRenderTier::Gate,
        authoritative_threads,
        CONTINUUM_SPECTRUM_ID,
        SpectralGrid::V1_ID,
        true,
    )?;
    check_report(&mut checks, "gate0", &gate_run, true);
    push(
        &mut checks,
        "gate_inherits_2b0_frequency_digest",
        gate_run.frequency_shift_digest == REF_FREQ,
        gate_run.frequency_shift_digest.clone(),
    );
    push(
        &mut checks,
        "gate_inherits_2b1_bolometric_digest",
        gate_run.bolometric_digest == REF_BOLO,
        gate_run.bolometric_digest.clone(),
    );
    // emission-spec digest is hashed into bolo digest; assert bolo path still canonical via REF_BOLO.
    let _ = REF_EMISSION_SPEC;

    write_line_shift_report(&root, &gate_run)?;

    let mut convergence = Vec::new();
    for n in [32u32, 64, 128, 256] {
        let grid_id = if n == SpectralGrid::V1_N_BINS {
            SpectralGrid::V1_ID.to_string()
        } else {
            format!("spectral-grid-explore-{n}")
        };
        let row_report = run_spectrum_cli(
            &root,
            &format!("artifacts/gate-2b2-spectral-transport/conv-{n}"),
            DiagnosticRenderTier::Smoke,
            smoke_threads,
            CONTINUUM_SPECTRUM_ID,
            &grid_id,
            true,
        )?;
        convergence.push(ConvergenceRow {
            n_bins: n,
            grid_id,
            max_rel_observed_closure_error: row_report.closure.max_rel_observed_closure_error,
            max_rel_emitted_closure_error: row_report.closure.max_rel_emitted_closure_error,
            spectral_digest: row_report.spectral_digest,
        });
    }
    if convergence.len() >= 4 {
        let e32 = convergence[0].max_rel_observed_closure_error;
        let e64 = convergence[1].max_rel_observed_closure_error;
        let e128 = convergence[2].max_rel_observed_closure_error;
        let e256 = convergence[3].max_rel_observed_closure_error;
        let r32_64 = if e64 > 0.0 { e32 / e64 } else { f64::INFINITY };
        let r64_128 = if e128 > 0.0 {
            e64 / e128
        } else {
            f64::INFINITY
        };
        let r128_256 = if e256 > 0.0 {
            e128 / e256
        } else {
            f64::INFINITY
        };
        push(
            &mut checks,
            "convergence_32_to_64_at_least_2x",
            e64 <= e32 / 2.0 + 1e-15,
            format!("32={e32:.6e} 64={e64:.6e} ratio={r32_64:.3}"),
        );
        push(
            &mut checks,
            "convergence_64_to_128_improves",
            e128 <= e64 + 1e-15,
            format!("64={e64:.6e} 128={e128:.6e}"),
        );
        push(
            &mut checks,
            "convergence_64_to_128_not_accelerating",
            r64_128 <= r32_64 * 1.25 + 1e-12,
            format!("r32_64={r32_64:.3} r64_128={r64_128:.3}"),
        );
        push(
            &mut checks,
            "convergence_128_to_256_improves",
            e256 <= e128 + 1e-15,
            format!("128={e128:.6e} 256={e256:.6e}"),
        );
        push(
            &mut checks,
            "convergence_128_to_256_not_accelerating",
            r128_256 <= r64_128 * 1.25 + 1e-12,
            format!("r64_128={r64_128:.3} r128_256={r128_256:.3}"),
        );
        // Grid selection / error-budget freeze: coarsest grid meeting budget with
        // required 32→64 improvement is spectral-grid-v1 (64 bins).
        push(
            &mut checks,
            "grid_selection_64_meets_error_budget",
            e64 <= CLOSURE_REL_TOL,
            format!("e64={e64:.6e} tol={CLOSURE_REL_TOL:.3e}"),
        );
        push(
            &mut checks,
            "grid_freeze_spectral_grid_v1_64",
            e64 <= e32 / 2.0 + 1e-15
                && e64 <= CLOSURE_REL_TOL
                && e128 <= e64 + 1e-15
                && e256 <= e128 + 1e-15,
            "authoritative spectral-grid-v1 = 64 bins".into(),
        );
    }

    push(
        &mut checks,
        "artifact_csv_has_spectral_bins",
        {
            let csv = std::fs::read_to_string(root.join(
                "artifacts/gate-2b2-spectral-transport/gate-run-0/selected-pixel-spectra.csv",
            ))?;
            let header = csv.lines().next().unwrap_or("");
            header.contains("m_capt")
                && header.contains("nu_0")
                && header.contains("i_nu_obs_0")
                && header.contains(&format!("i_nu_obs_{}", SpectralGrid::V1_N_BINS - 1))
        },
        "selected-pixel-spectra.csv includes ν and I_ν bins".into(),
    );
    push(
        &mut checks,
        "artifact_meta_m_capt_pgm_semantics",
        {
            let meta: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(
                "artifacts/gate-2b2-spectral-transport/gate-run-0/spectral-frame-meta.json",
            ))?)?;
            meta.get("bolometric_relative_error_pgm")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s.contains("M_capt"))
        },
        "meta declares truncation-aware PGM semantics".into(),
    );

    // Scope exclusions
    push(
        &mut checks,
        "scope_no_openexr_rgb_gpu",
        !crate_sources_mention_forbidden(&root)?,
        "spectral modules free of OpenEXR/CIE/GPU tokens".into(),
    );

    let authoritative = checks.iter().all(|c| c.status == "PASS") && !dirty && self_release;
    let result: String = if authoritative {
        "PASS".into()
    } else if checks.iter().any(|c| c.status == "FAIL") {
        "FAIL".into()
    } else {
        "NON_AUTHORITATIVE".into()
    };

    let mut report = Gate2b2Eval {
        gate: "gate-2b2-spectral-transport".into(),
        result: result.clone(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: available,
        authoritative_threads,
        continuum_digest,
        spectral_grid_digest: grid_digest,
        checks,
        smoke_serial: Some(smoke_serial),
        smoke_parallel: Some(smoke_parallel),
        gate_run: Some(gate_run),
        convergence,
        content_digest_excluding_digest_field: String::new(),
    };
    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.result != "PASS" {
        return Err(format!("gate-2b2-spectral-transport {}", report.result).into());
    }
    Ok(())
}

fn hermetic_spectral_checks(checks: &mut Vec<Check>) -> Result<(), Box<dyn std::error::Error>> {
    let spec = diagnostic_lognormal_continuum_v1();
    let n = continuum_normalization(&spec)?;
    push(
        checks,
        "continuum_norm_finite_positive",
        n.is_finite() && n > 0.0,
        format!("{n}"),
    );

    for g in [0.5_f64, 1.0, 2.0] {
        let line = diagnostic_gaussian_line_v1(1.0);
        let nu_obs = g; // peak at ν_em=1
        let i_em = evaluate_line_fixture(&line, nu_obs / g)?;
        let i_obs = transport_i_nu(i_em, g)?;
        push(
            checks,
            &format!("line_g3_identity_g_{g}"),
            (i_obs - i_em * g.powi(3)).abs() < 1e-14,
            format!("i_obs={i_obs}"),
        );
        let wrong_g4 = i_em * g.powi(4);
        push(
            checks,
            &format!("line_rejects_g4_pointwise_g_{g}"),
            g == 1.0 || (i_obs - wrong_g4).abs() > 1e-9,
            "g³ ≠ g⁴ for g≠1".into(),
        );
    }

    let g = 0.5;
    let nu_obs = Frequency::new(1.0)?;
    let i_nu_em = 2.5;
    let i_nu_obs = transport_i_nu(i_nu_em, g)?;
    let lam_obs = wavelength_from_frequency(nu_obs)?;
    let lam_em = wavelength_from_frequency(Frequency::new(nu_obs.value() / g)?)?;
    let i_lam_em = i_lambda_from_i_nu(i_nu_em, lam_em)?;
    let i_lam_obs = i_lambda_from_i_nu(i_nu_obs, lam_obs)?;
    push(
        checks,
        "wavelength_jacobian_g5",
        (i_lam_obs - i_lam_em * g.powi(5)).abs() < 1e-12,
        format!("{i_lam_obs}"),
    );

    let i = independent_i_nu_obs(1.0, 1.0, 1.0, &spec)?;
    let phi = evaluate_continuum_phi(&spec, 1.0)?;
    push(
        checks,
        "independent_pointwise_g1",
        (i - phi).abs() < 1e-14,
        format!("{i}"),
    );

    // Tiny synthetic "frame" closure via continuum integral only.
    let grid = SpectralGrid::log_spaced("hermetic", spec.nu_min, spec.nu_max, 64)?;
    let mut samples = Vec::new();
    for &c in grid.centers() {
        samples.push(evaluate_continuum_phi(&spec, c)?);
    }
    let integ = grid.integrate(&samples)?;
    push(
        checks,
        "phi_integral_near_one_64",
        (integ - 1.0).abs() < 1e-2,
        format!("{integ}"),
    );
    Ok(())
}

fn check_report(
    checks: &mut Vec<Check>,
    tag: &str,
    report: &SpectralRenderReport,
    require_ref: bool,
) {
    push(
        checks,
        &format!("{tag}_disk_hits_nonzero"),
        report.disk_hit_count > 0,
        format!("{}", report.disk_hit_count),
    );
    let c = &report.closure;
    let ok = c.max_rel_emitted_closure_error <= CLOSURE_REL_TOL
        && c.max_rel_observed_closure_error <= CLOSURE_REL_TOL
        && c.max_abs_emitted_closure_error <= CLOSURE_ABS_TOL
        && c.max_abs_observed_closure_error <= CLOSURE_ABS_TOL;
    push(
        checks,
        &format!("{tag}_bolometric_closure"),
        ok,
        format!(
            "rel_em={:.6e} rel_obs={:.6e} abs_em={:.6e} abs_obs={:.6e}",
            c.max_rel_emitted_closure_error,
            c.max_rel_observed_closure_error,
            c.max_abs_emitted_closure_error,
            c.max_abs_observed_closure_error
        ),
    );
    if require_ref {
        push(
            checks,
            &format!("{tag}_freq_ref"),
            report.frequency_shift_digest == REF_FREQ,
            report.frequency_shift_digest.clone(),
        );
        push(
            checks,
            &format!("{tag}_bolo_ref"),
            report.bolometric_digest == REF_BOLO,
            report.bolometric_digest.clone(),
        );
    }
}

fn run_spectrum_cli(
    root: &Path,
    output_dir: &str,
    tier: DiagnosticRenderTier,
    threads: usize,
    spectrum: &str,
    spectral_grid: &str,
    parallel_if_threads: bool,
) -> Result<SpectralRenderReport, Box<dyn std::error::Error>> {
    let out = root.join(output_dir);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out)?;
    let execution = if parallel_if_threads && threads > 1 {
        "parallel"
    } else {
        "serial"
    };
    let tier_s = match tier {
        DiagnosticRenderTier::Smoke => "smoke",
        DiagnosticRenderTier::Preview => "preview",
        DiagnosticRenderTier::Gate => "gate",
        DiagnosticRenderTier::Showcase => "showcase",
    };
    let status = {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(root).args([
            "run",
            "--release",
            "-p",
            "xtask",
            "--",
            "render-disk-spectrum",
            "--preset",
            "presets/gargantua-baseline.toml",
            "--tier",
            tier_s,
            "--spectrum",
            spectrum,
            "--spectral-grid",
            spectral_grid,
            "--output-dir",
            output_dir,
            "--execution",
            execution,
            "--require-release",
        ]);
        if execution == "parallel" {
            cmd.args(["--threads", &threads.to_string()]);
        }
        cmd.status()?
    };
    if !status.success() {
        return Err(format!("render-disk-spectrum failed for {output_dir}").into());
    }
    let report_path = out.join("spectral-render-report.json");
    let report: SpectralRenderReport = serde_json::from_slice(&std::fs::read(report_path)?)?;
    Ok(report)
}

fn write_line_shift_report(
    root: &Path,
    gate: &SpectralRenderReport,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for g in [0.5_f64, 1.0, 2.0] {
        let line = diagnostic_gaussian_line_v1(1.0);
        let nu_obs = g;
        let i_em = evaluate_line_fixture(&line, nu_obs / g)?;
        let i_obs = transport_i_nu(i_em, g)?;
        cases.push(serde_json::json!({
            "g": g,
            "nu_obs": nu_obs,
            "i_em_at_nu_em": i_em,
            "i_obs_g3": i_obs,
            "wrong_g4": i_em * g.powi(4),
            "amplitude_ok": (i_obs - i_em * g.powi(3)).abs() < 1e-14,
        }));
    }
    let doc = serde_json::json!({
        "gate": "gate-2b2-spectral-transport",
        "fixture": "diagnostic-gaussian-line-v1",
        "inherited_frequency_digest": gate.frequency_shift_digest,
        "inherited_bolometric_digest": gate.bolometric_digest,
        "synthetic_g_cases": cases,
        "note": "Full-frame selected pixels are in selected-pixel-spectra.csv; this report covers hermetic g³ amplitude identities.",
    });
    std::fs::write(
        root.join("artifacts/gate-2b2-spectral-transport/spectral-line-shift-report.json"),
        serde_json::to_vec_pretty(&doc)?,
    )?;
    Ok(())
}

fn crate_sources_mention_forbidden(root: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    // Do not scan this evaluator file — it necessarily names excluded tokens.
    let paths = [
        root.join("crates/relativity-core/src/spectral.rs"),
        root.join("crates/relativity-render/src/spectral.rs"),
        root.join("xtask/src/render_disk_spectrum.rs"),
    ];
    // Concrete integrations only — not claim-boundary names like physical_rgb_status.
    let forbidden = [
        "openexr", "OpenEXR", "wgpu::", "egui::", "cie_xyz", "CIE_XYZ",
    ];
    for p in paths {
        let text = std::fs::read_to_string(p)?;
        for tok in forbidden {
            if text.contains(tok) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn finalize(root: &Path, report: &mut Gate2b2Eval) -> Result<(), Box<dyn std::error::Error>> {
    report.content_digest_excluding_digest_field = content_digest(report)?;
    let out = root.join("artifacts/gate-2b2-spectral-transport");
    std::fs::create_dir_all(&out)?;
    std::fs::write(
        out.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# Gate 2B2 evaluation\n\n");
    md.push_str(&format!("result: **{}**\n\n", report.result));
    md.push_str(&format!("commit: `{}`\n\n", report.commit));
    md.push_str("| check | status | detail |\n| --- | --- | --- |\n");
    for c in &report.checks {
        md.push_str(&format!("| {} | {} | {} |\n", c.name, c.status, c.detail));
    }
    std::fs::write(out.join("evaluation.md"), md)?;
    Ok(())
}

fn content_digest(report: &Gate2b2Eval) -> Result<String, Box<dyn std::error::Error>> {
    let mut clone = serde_json::to_value(report)?;
    if let Some(obj) = clone.as_object_mut() {
        obj.remove("content_digest_excluding_digest_field");
        obj.remove("build"); // wall timing / host metadata
                             // Strip nested timing if any later.
    }
    let bytes = serde_json::to_vec(&clone)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(hex::encode(h.finalize()))
}

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> Gate2b2Eval {
    Gate2b2Eval {
        gate: "gate-2b2-spectral-transport".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        continuum_digest: String::new(),
        spectral_grid_digest: String::new(),
        checks,
        smoke_serial: None,
        smoke_parallel: None,
        gate_run: None,
        convergence: Vec::new(),
        content_digest_excluding_digest_field: String::new(),
    }
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

fn files_eq(root: &Path, a: &str, b: &str, file: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let ba = std::fs::read(root.join(a).join(file))?;
    let bb = std::fs::read(root.join(b).join(file))?;
    Ok(ba == bb)
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = git_stdout(root, &["status", "--porcelain"])?;
    let dirty = !out.trim().is_empty();
    Ok((dirty, out.trim().chars().take(200).collect()))
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

// Silence unused import if closure helper is only used via reports.
#[allow(dead_code)]
fn _touch_closure() {
    let _ = compute_bolometric_closure;
}
