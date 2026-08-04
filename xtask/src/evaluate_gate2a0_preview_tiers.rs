//! Gate 2A0-4 named preview quality tiers evaluator.

use crate::build_meta::{
    is_optimized_release_execution, read_build_execution_report, require_release_execution,
    BuildExecutionMetadata,
};
use crate::render_tier::{
    DiagnosticRenderTier, RenderAuthorityClass, ResolutionSource, LEGACY_DEFAULT_AXIS,
};
use crate::trace_outcome_map::read_trace_execution_report;
use crate::trace_shade_many::TraceShadeReport;
use relativity_trace::{hex_sha, DiagnosticShadeStyle, OutcomeCounts, TraceExecutionMode};
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

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct TierTiming {
    label: String,
    width: u32,
    height: u32,
    ray_count: u64,
    thread_count: usize,
    trace_wall_clock_seconds: Option<f64>,
    shade_wall_clock_seconds: Option<f64>,
    rays_per_second: Option<f64>,
}

#[derive(Serialize, Clone)]
struct Gate2a0PreviewTiersEval {
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
    smoke: Option<TraceShadeReport>,
    preview: Option<TraceShadeReport>,
    gate_runs: Vec<TraceShadeReport>,
    showcase: Option<TraceShadeReport>,
    custom_authority_negative: Option<TraceShadeReport>,
    shared_numerical_profile_digest: Option<String>,
    disk_suppressed_changed_pixels: Option<u64>,
    tier_timings: Vec<TierTiming>,
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
        return Err("gate-2a0-preview-tiers requires release evaluator".into());
    }
    require_release_execution(&build)?;

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

    // Unit-level CLI resolution invariants (no subprocess).
    let legacy = crate::render_tier::resolve_render_plan(None, None, None)?;
    push(
        &mut checks,
        "legacy_default_resolution_128",
        legacy.width == LEGACY_DEFAULT_AXIS
            && legacy.height == LEGACY_DEFAULT_AXIS
            && legacy.resolution_source == ResolutionSource::LegacyDefault
            && legacy.authority_class == RenderAuthorityClass::NonAuthoritative,
        format!("{legacy:?}"),
    );

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let authoritative_threads = available;

    let out_root = root.join("artifacts/gate-2a0-preview-tiers");
    std::fs::create_dir_all(&out_root)?;

    let smoke = run_worker_tier(
        &root,
        Some(DiagnosticRenderTier::Smoke),
        None,
        None,
        "artifacts/gate-2a0-preview-tiers/smoke",
        authoritative_threads,
    )?;
    check_named_tier(
        &mut checks,
        "smoke",
        &smoke,
        DiagnosticRenderTier::Smoke,
        RenderAuthorityClass::NonAuthoritative,
    )?;

    let preview = run_worker_tier(
        &root,
        Some(DiagnosticRenderTier::Preview),
        None,
        None,
        "artifacts/gate-2a0-preview-tiers/preview",
        authoritative_threads,
    )?;
    check_named_tier(
        &mut checks,
        "preview",
        &preview,
        DiagnosticRenderTier::Preview,
        RenderAuthorityClass::NonAuthoritative,
    )?;

    let mut gate_runs = Vec::new();
    for i in 0..2 {
        gate_runs.push(run_worker_tier(
            &root,
            Some(DiagnosticRenderTier::Gate),
            None,
            None,
            &format!("artifacts/gate-2a0-preview-tiers/gate-run-{i}"),
            authoritative_threads,
        )?);
    }
    check_named_tier(
        &mut checks,
        "gate0",
        &gate_runs[0],
        DiagnosticRenderTier::Gate,
        RenderAuthorityClass::AuthoritativeCandidate,
    )?;
    check_named_tier(
        &mut checks,
        "gate1",
        &gate_runs[1],
        DiagnosticRenderTier::Gate,
        RenderAuthorityClass::AuthoritativeCandidate,
    )?;

    let showcase = run_worker_tier(
        &root,
        Some(DiagnosticRenderTier::Showcase),
        None,
        None,
        "artifacts/gate-2a0-preview-tiers/showcase",
        authoritative_threads,
    )?;
    check_named_tier(
        &mut checks,
        "showcase",
        &showcase,
        DiagnosticRenderTier::Showcase,
        RenderAuthorityClass::NonAuthoritative,
    )?;

    let custom = run_worker_tier(
        &root,
        None,
        Some(128),
        Some(128),
        "artifacts/gate-2a0-preview-tiers/custom-authority-negative",
        authoritative_threads,
    )?;
    push(
        &mut checks,
        "custom_128_resolution_source",
        custom.resolution_source == ResolutionSource::CustomDimensions
            && custom.width == 128
            && custom.height == 128
            && custom.render_tier.is_none(),
        format!(
            "source={:?} {}×{} tier={:?}",
            custom.resolution_source, custom.width, custom.height, custom.render_tier
        ),
    );
    push(
        &mut checks,
        "custom_128_non_authoritative",
        custom.authority_class == RenderAuthorityClass::NonAuthoritative,
        format!("{:?}", custom.authority_class),
    );
    check_worker_common(&mut checks, "custom", &custom)?;

    let profiles = [
        smoke.numerical_profile.digest.clone(),
        preview.numerical_profile.digest.clone(),
        gate_runs[0].numerical_profile.digest.clone(),
        gate_runs[1].numerical_profile.digest.clone(),
        showcase.numerical_profile.digest.clone(),
        custom.numerical_profile.digest.clone(),
    ];
    let shared = profiles[0].clone();
    let profile_ok = profiles.iter().all(|d| *d == shared);
    push(
        &mut checks,
        "shared_numerical_profile_digest",
        profile_ok,
        shared.clone(),
    );

    let one_sample = std::fs::read_to_string(root.join("crates/relativity-trace/src/trace.rs"))?
        .contains("sensor_at_pixel_center")
        && std::fs::read_to_string(root.join("crates/relativity-trace/src/trace.rs"))?
            .contains("one sample per pixel center");
    push(
        &mut checks,
        "one_sample_per_pixel_center",
        one_sample,
        "trace_grid documents/uses pixel-center sampling".into(),
    );

    let det_ok = gate_runs[0].trace_data_digest == gate_runs[1].trace_data_digest
        && gate_runs[0].outcome_class_digest == gate_runs[1].outcome_class_digest
        && gate_runs[0].rhs_pgm_digest == gate_runs[1].rhs_pgm_digest
        && gate_runs[0].shaded_outputs == gate_runs[1].shaded_outputs
        && counts_eq(&gate_runs[0].outcome_counts, &gate_runs[1].outcome_counts)
        && gate_runs[0].total_accepted_steps == gate_runs[1].total_accepted_steps
        && gate_runs[0].total_rejected_steps == gate_runs[1].total_rejected_steps
        && gate_runs[0].total_rhs_evaluations == gate_runs[1].total_rhs_evaluations;
    push(
        &mut checks,
        "gate_128_subprocess_determinism",
        det_ok,
        format!(
            "trace_data={} class={}",
            gate_runs[0].trace_data_digest, gate_runs[0].outcome_class_digest
        ),
    );

    let legacy_ppm = gate_runs[0]
        .shaded_outputs
        .iter()
        .find(|o| o.style == DiagnosticShadeStyle::Gate1b2Categorical)
        .ok_or("missing legacy style")?;
    let suppressed = gate_runs[0]
        .shaded_outputs
        .iter()
        .find(|o| o.style == DiagnosticShadeStyle::DiskSuppressed)
        .ok_or("missing disk-suppressed style")?;

    push(
        &mut checks,
        "gate_class_matches_1b2",
        gate_runs[0].outcome_class_digest == REF_CLASS,
        gate_runs[0].outcome_class_digest.clone(),
    );
    push(
        &mut checks,
        "gate_ppm_matches_1b2",
        legacy_ppm.ppm_digest == REF_PPM,
        legacy_ppm.ppm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_pgm_matches_1b2",
        gate_runs[0].rhs_pgm_digest == REF_PGM,
        gate_runs[0].rhs_pgm_digest.clone(),
    );
    push(
        &mut checks,
        "gate_counts_match_1b2",
        counts_eq(&gate_runs[0].outcome_counts, &REF_COUNTS)
            && gate_runs[0].outcome_counts.failed == 0,
        format!("{:?}", gate_runs[0].outcome_counts),
    );

    let dir0 = root.join("artifacts/gate-2a0-preview-tiers/gate-run-0");
    let legacy_bytes = std::fs::read(dir0.join(&legacy_ppm.filename))?;
    let supp_bytes = std::fs::read(dir0.join(&suppressed.filename))?;
    let (changed, non_disk_ok) = ppm_disk_diff(&legacy_bytes, &supp_bytes, 128, 128)?;
    push(
        &mut checks,
        "disk_suppressed_diff_equals_disk_hit_count",
        changed == gate_runs[0].outcome_counts.disk_hit && non_disk_ok,
        format!(
            "changed={changed} disk_hit={} non_disk_identical={non_disk_ok}",
            gate_runs[0].outcome_counts.disk_hit
        ),
    );

    let no_sky = !std::fs::read_to_string(root.join("crates/relativity-trace/src/shade.rs"))?
        .contains("celestial")
        && !std::fs::read_to_string(root.join("xtask/src/render_tier.rs"))?.contains("openexr");
    push(
        &mut checks,
        "no_celestial_sphere_or_radiometry",
        no_sky,
        "preview tiers remain diagnostic-only".into(),
    );

    let only_gate_auth = smoke.authority_class == RenderAuthorityClass::NonAuthoritative
        && preview.authority_class == RenderAuthorityClass::NonAuthoritative
        && showcase.authority_class == RenderAuthorityClass::NonAuthoritative
        && custom.authority_class == RenderAuthorityClass::NonAuthoritative
        && gate_runs[0].authority_class == RenderAuthorityClass::AuthoritativeCandidate;
    push(
        &mut checks,
        "only_explicit_gate_authoritative_candidate",
        only_gate_auth,
        "smoke/preview/showcase/custom non-auth; gate candidate".into(),
    );

    let gate_plan_ok = gate_runs[0].render_tier == Some(DiagnosticRenderTier::Gate)
        && gate_runs[0].width == 128
        && gate_runs[0].height == 128
        && gate_runs[0].resolution_source == ResolutionSource::NamedTier
        && gate_runs[0].authority_class == RenderAuthorityClass::AuthoritativeCandidate;
    push(
        &mut checks,
        "gate_authority_plan_complete",
        gate_plan_ok,
        format!(
            "tier={:?} {}×{} source={:?} auth={:?}",
            gate_runs[0].render_tier,
            gate_runs[0].width,
            gate_runs[0].height,
            gate_runs[0].resolution_source,
            gate_runs[0].authority_class
        ),
    );

    let tier_timings = vec![
        timing_of("smoke", &smoke),
        timing_of("preview", &preview),
        timing_of("gate-run-0", &gate_runs[0]),
        timing_of("showcase", &showcase),
        timing_of("custom-128", &custom),
    ];

    let hard_fail = checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    let authority_ok = gate_plan_ok && !hard_fail && self_release && !dirty;
    let authoritative = authority_ok;
    let result = if hard_fail {
        "FAIL"
    } else if authoritative {
        "PASS"
    } else {
        "PASS_NON_AUTHORITATIVE"
    };

    let mut report = Gate2a0PreviewTiersEval {
        gate: "gate-2a0-preview-tiers".into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads: available,
        authoritative_threads,
        checks,
        smoke: Some(smoke),
        preview: Some(preview),
        gate_runs,
        showcase: Some(showcase),
        custom_authority_negative: Some(custom),
        shared_numerical_profile_digest: Some(shared),
        disk_suppressed_changed_pixels: Some(changed),
        tier_timings,
        content_digest_excluding_digest_field: String::new(),
    };

    let digest = eval_digest(&report);
    report.content_digest_excluding_digest_field = digest.clone();
    let verify = eval_digest(&Gate2a0PreviewTiersEval {
        content_digest_excluding_digest_field: String::new(),
        ..report.clone()
    });
    report.checks.push(Check {
        name: "artifact_digest_convention".into(),
        status: if verify == digest { "PASS" } else { "FAIL" },
        detail: format!("digest={digest}"),
    });
    let hard_fail = report
        .checks
        .iter()
        .any(|c| c.status == "FAIL" && c.name != "worktree_clean");
    let gate0_ok = report
        .gate_runs
        .first()
        .map(|g| {
            g.render_tier == Some(DiagnosticRenderTier::Gate)
                && g.width == 128
                && g.height == 128
                && g.resolution_source == ResolutionSource::NamedTier
                && g.authority_class == RenderAuthorityClass::AuthoritativeCandidate
        })
        .unwrap_or(false);
    report.authoritative =
        !dirty && !hard_fail && report.build.is_optimized_release_execution() && gate0_ok;
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
        return Err("gate-2a0-preview-tiers evaluation FAIL".into());
    }
    Ok(())
}

fn timing_of(label: &str, r: &TraceShadeReport) -> TierTiming {
    TierTiming {
        label: label.into(),
        width: r.width,
        height: r.height,
        ray_count: u64::from(r.width) * u64::from(r.height),
        thread_count: r.execution.thread_count,
        trace_wall_clock_seconds: r.trace_wall_clock_seconds,
        shade_wall_clock_seconds: r.shade_wall_clock_seconds,
        rays_per_second: r.rays_per_second,
    }
}

fn check_named_tier(
    checks: &mut Vec<Check>,
    label: &str,
    report: &TraceShadeReport,
    expected: DiagnosticRenderTier,
    auth: RenderAuthorityClass,
) -> Result<(), Box<dyn std::error::Error>> {
    let (ew, eh) = expected.dimensions();
    push(
        checks,
        &format!("{label}_dimensions"),
        report.width == ew && report.height == eh,
        format!("{}×{} (expected {ew}×{eh})", report.width, report.height),
    );
    push(
        checks,
        &format!("{label}_named_tier_source"),
        report.render_tier == Some(expected)
            && report.resolution_source == ResolutionSource::NamedTier,
        format!(
            "tier={:?} source={:?}",
            report.render_tier, report.resolution_source
        ),
    );
    push(
        checks,
        &format!("{label}_authority_class"),
        report.authority_class == auth,
        format!("{:?}", report.authority_class),
    );
    check_worker_common(checks, label, report)?;
    Ok(())
}

fn check_worker_common(
    checks: &mut Vec<Check>,
    label: &str,
    report: &TraceShadeReport,
) -> Result<(), Box<dyn std::error::Error>> {
    push(
        checks,
        &format!("{label}_trace_invocations"),
        report.trace_invocations == 1,
        format!("trace_invocations={}", report.trace_invocations),
    );
    push(
        checks,
        &format!("{label}_shade_passes"),
        report.shade_passes == 2,
        format!("shade_passes={}", report.shade_passes),
    );
    let order_ok = report.styles
        == [
            DiagnosticShadeStyle::Gate1b2Categorical,
            DiagnosticShadeStyle::DiskSuppressed,
        ];
    push(
        checks,
        &format!("{label}_style_order"),
        order_ok,
        format!("{:?}", report.styles),
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
    Ok(())
}

fn run_worker_tier(
    root: &Path,
    tier: Option<DiagnosticRenderTier>,
    width: Option<u32>,
    height: Option<u32>,
    output_dir: &str,
    threads: usize,
) -> Result<TraceShadeReport, Box<dyn std::error::Error>> {
    let mut args = vec![
        "run".into(),
        "--release".into(),
        "-q".into(),
        "-p".into(),
        "xtask".into(),
        "--".into(),
        "trace-shade-many".into(),
        "--preset".into(),
        "presets/gargantua-baseline.toml".into(),
        "--output-dir".into(),
        output_dir.into(),
        "--execution".into(),
        "parallel".into(),
        "--threads".into(),
        threads.to_string(),
        "--style".into(),
        "gate1b2-categorical".into(),
        "--style".into(),
        "disk-suppressed".into(),
        "--require-release".into(),
    ];
    if let Some(t) = tier {
        args.push("--tier".into());
        args.push(t.as_str().into());
    }
    if let Some(w) = width {
        args.push("--width".into());
        args.push(w.to_string());
    }
    if let Some(h) = height {
        args.push("--height".into());
        args.push(h.to_string());
    }

    let out = Command::new("cargo")
        .current_dir(root)
        .args(&args)
        .output()?;
    if !out.status.success() {
        return Err(format!(
            "trace-shade-many failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    let dir = if Path::new(output_dir).is_absolute() {
        PathBuf::from(output_dir)
    } else {
        root.join(output_dir)
    };
    let report: TraceShadeReport =
        serde_json::from_slice(&std::fs::read(dir.join("trace-shade-report.json"))?)?;
    let build = read_build_execution_report(&dir)?;
    let exec = read_trace_execution_report(&dir)?;
    if build != report.build {
        return Err("build-execution.json disagrees with report.build".into());
    }
    if exec != report.execution {
        return Err("trace-execution.json disagrees with report.execution".into());
    }
    if report.trace_invocations != 1 {
        return Err("worker reported trace_invocations != 1".into());
    }
    Ok(report)
}

fn ppm_disk_diff(
    legacy: &[u8],
    suppressed: &[u8],
    width: u32,
    height: u32,
) -> Result<(u64, bool), Box<dyn std::error::Error>> {
    let header = format!("P6\n{width} {height}\n255\n");
    let hb = header.as_bytes();
    if !legacy.starts_with(hb) || !suppressed.starts_with(hb) {
        return Err("PPM header mismatch".into());
    }
    let a = &legacy[hb.len()..];
    let b = &suppressed[hb.len()..];
    if a.len() != b.len() || a.len() != (width as usize) * (height as usize) * 3 {
        return Err("PPM payload length mismatch".into());
    }
    let mut changed = 0u64;
    let mut non_disk_ok = true;
    for i in 0..(a.len() / 3) {
        let pa = [a[i * 3], a[i * 3 + 1], a[i * 3 + 2]];
        let pb = [b[i * 3], b[i * 3 + 1], b[i * 3 + 2]];
        if pa == [255, 128, 0] {
            if pb != [0, 0, 0] {
                non_disk_ok = false;
            } else {
                changed += 1;
            }
        } else if pa != pb {
            non_disk_ok = false;
        }
    }
    Ok((changed, non_disk_ok))
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
) -> Gate2a0PreviewTiersEval {
    Gate2a0PreviewTiersEval {
        gate: "gate-2a0-preview-tiers".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        checks,
        smoke: None,
        preview: None,
        gate_runs: vec![],
        showcase: None,
        custom_authority_negative: None,
        shared_numerical_profile_digest: None,
        disk_suppressed_changed_pixels: None,
        tier_timings: vec![],
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(
    root: &Path,
    report: &mut Gate2a0PreviewTiersEval,
) -> Result<(), Box<dyn std::error::Error>> {
    if report.content_digest_excluding_digest_field.is_empty() {
        let mut h = report.clone();
        h.content_digest_excluding_digest_field.clear();
        report.content_digest_excluding_digest_field = eval_digest(&h);
    }
    let dir = root.join("artifacts/gate-2a0-preview-tiers");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# Gate 2A0 Preview Tiers Evaluation\n\n");
    md.push_str(&format!("- Result: **{}**\n", report.result));
    md.push_str(&format!("- Authoritative: {}\n", report.authoritative));
    md.push_str(&format!("- Commit: `{}`\n", report.commit));
    md.push_str(&format!(
        "- Digest: `{}`\n",
        report.content_digest_excluding_digest_field
    ));
    if let Some(d) = &report.shared_numerical_profile_digest {
        md.push_str(&format!("- Numerical profile: `{d}`\n"));
    }
    md.push_str("\n## Checks\n\n");
    for c in &report.checks {
        md.push_str(&format!("- [{}] {}: {}\n", c.status, c.name, c.detail));
    }
    md.push_str("\n## Tier timings\n\n");
    for t in &report.tier_timings {
        md.push_str(&format!(
            "- {}: {}×{} rays={} trace={:?}s shade={:?}s rays/s={:?} threads={}\n",
            t.label,
            t.width,
            t.height,
            t.ray_count,
            t.trace_wall_clock_seconds,
            t.shade_wall_clock_seconds,
            t.rays_per_second,
            t.thread_count
        ));
    }
    std::fs::write(dir.join("evaluation.md"), md)?;
    std::fs::write(
        dir.join("evaluation.content_digest.sha256"),
        format!("{}\n", report.content_digest_excluding_digest_field),
    )?;
    Ok(())
}

fn eval_digest(report: &Gate2a0PreviewTiersEval) -> String {
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
        checks: Vec<DigestCheck<'a>>,
        smoke: Option<&'a TraceShadeReport>,
        preview: Option<&'a TraceShadeReport>,
        gate_runs: &'a [TraceShadeReport],
        showcase: Option<&'a TraceShadeReport>,
        custom_authority_negative: Option<&'a TraceShadeReport>,
        shared_numerical_profile_digest: Option<&'a str>,
        disk_suppressed_changed_pixels: Option<u64>,
        content_digest_excluding_digest_field: &'a str,
    }
    #[derive(Serialize)]
    struct DigestCheck<'a> {
        name: &'a str,
        status: &'a str,
    }
    let smoke = report.smoke.as_ref().map(strip_timing);
    let preview = report.preview.as_ref().map(strip_timing);
    let gate_runs: Vec<_> = report.gate_runs.iter().map(strip_timing).collect();
    let showcase = report.showcase.as_ref().map(strip_timing);
    let custom = report.custom_authority_negative.as_ref().map(strip_timing);
    let proj = Proj {
        gate: &report.gate,
        result: &report.result,
        authoritative: report.authoritative,
        commit: &report.commit,
        dirty: report.dirty,
        build: &report.build,
        available_threads: report.available_threads,
        authoritative_threads: report.authoritative_threads,
        checks: report
            .checks
            .iter()
            .map(|c| DigestCheck {
                name: &c.name,
                status: c.status,
            })
            .collect(),
        smoke: smoke.as_ref(),
        preview: preview.as_ref(),
        gate_runs: &gate_runs,
        showcase: showcase.as_ref(),
        custom_authority_negative: custom.as_ref(),
        shared_numerical_profile_digest: report.shared_numerical_profile_digest.as_deref(),
        disk_suppressed_changed_pixels: report.disk_suppressed_changed_pixels,
        content_digest_excluding_digest_field: "",
    };
    hex_sha(&serde_json::to_vec(&proj).expect("serialize"))
}

fn strip_timing(r: &TraceShadeReport) -> TraceShadeReport {
    let mut c = r.clone();
    c.trace_wall_clock_seconds = None;
    c.shade_wall_clock_seconds = None;
    c.rays_per_second = None;
    if let Some(cel) = c.celestial_coordinates.as_mut() {
        cel.mapping_wall_clock_seconds = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_detail_excluded_from_eval_digest() {
        let build = BuildExecutionMetadata {
            cargo_profile: "release".into(),
            opt_level: "3".into(),
            debug_assertions: false,
            target: "t".into(),
            toolchain: "t".into(),
        };
        let mut a = empty(
            &build,
            "c",
            false,
            String::new(),
            vec![Check {
                name: "x".into(),
                status: "PASS",
                detail: "trace=1.0s".into(),
            }],
        );
        let mut b = a.clone();
        b.checks[0].detail = "trace=99s".into();
        a.available_threads = 8;
        b.available_threads = 8;
        a.authoritative_threads = 8;
        b.authoritative_threads = 8;
        assert_eq!(eval_digest(&a), eval_digest(&b));
        b.checks[0].status = "FAIL";
        assert_ne!(eval_digest(&a), eval_digest(&b));
    }
}
