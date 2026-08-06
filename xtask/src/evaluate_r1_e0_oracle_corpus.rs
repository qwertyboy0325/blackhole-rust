//! R1/E0 authoritative oracle corpus evaluator.

use crate::build_meta::{
    is_optimized_release_execution, require_release_execution, BuildExecutionMetadata,
};
use crate::oracle_benchmark;
use crate::trace_outcome_map::CliExecution;
use relativity_oracle::{OracleChannelSet, OracleFrame, ORACLE_ID_V1, ORACLE_SCHEMA_VERSION};
use relativity_trace::{hex_sha, OutcomeCounts};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

const APPROVED_BASE: &str = "dcceef661574d21ce4c0aa8817fcf9d9fa1039a1";
const REF_CLASS: &str = "64462a83927b111ed808a38292e2d5b1393b4045b580f1b416b1dc001cd452c4";
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
const REF_FREQ: &str = "65df7b55da2d8ed31935252e2907e8bf1bb686452aacf49bb9f2469fb5a875c2";
const REF_BOLO: &str = "d3721de712ddafb660513b482f6c089cfc79be087f78ef1592e46cfdec0746b2";
const REF_COMPOSITE_PPM: &str = "7982aaa9cdd9f176850f4f6def2d2364bcf3bc6734c054f261332c53beda2a69";
const REF_TRACE_DATA: &str = "b2c60252aea519866370774d97a8d8c1b9c7d626d3429fc2a1ae4b57a0f691a9";
const MANIFEST_PATH: &str = "experiments/oracle-benchmark/corpus-v1.toml";
const COMMITTED_LOCK_PATH: &str = "experiments/oracle-benchmark/corpus-lock-v1.json";

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: &'static str,
    detail: String,
}

#[derive(Serialize, Clone)]
struct R1E0Eval {
    gate: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    build: BuildExecutionMetadata,
    available_threads: usize,
    authoritative_threads: usize,
    committed_lock_digest: String,
    regenerated_lock_digest: String,
    checks: Vec<Check>,
    content_digest_excluding_digest_field: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CorpusLock {
    schema_version: u32,
    corpus_id: String,
    oracle_schema_id: String,
    source_cases: Vec<LockedSourceCase>,
    crop_cases: Vec<LockedCropCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct LockedSourceCase {
    definition: LockedCaseDefinition,
    oracle_scientific_digest: String,
    reference_image_digest: String,
    outcome_counts: OutcomeCounts,
}

#[derive(Debug, Clone, Deserialize)]
struct LockedCaseDefinition {
    id: String,
    spin_a_over_m: f64,
    channel_set: OracleChannelSet,
}

#[derive(Debug, Clone, Deserialize)]
struct LockedCropCase {
    id: String,
    oracle_scientific_digest: String,
}

#[derive(Debug, Deserialize)]
struct ScientificSummary {
    oracle_scientific_digest: String,
    source_digests: SourceDigests,
    outcome_counts: OutcomeCounts,
}

#[derive(Debug, Deserialize)]
struct SourceDigests {
    numerical_profile_digest: String,
    trace_data_digest: String,
    outcome_class_digest: String,
    celestial_coordinate_digest: String,
    frequency_shift_digest: Option<String>,
    bolometric_digest: Option<String>,
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
        return Err("r1-e0-oracle-corpus requires release evaluator".into());
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

    let committed_lock_bytes = std::fs::read(root.join(COMMITTED_LOCK_PATH))?;
    let committed_lock_digest = hex_sha(&Sha256::digest(&committed_lock_bytes));
    let committed: CorpusLock = serde_json::from_slice(&committed_lock_bytes)?;
    push(
        &mut checks,
        "committed_lock_shape",
        committed.schema_version == 1
            && committed.source_cases.len() == 6
            && committed.crop_cases.len() == 2
            && committed.corpus_id == "e0-oracle-corpus-v1"
            && committed.oracle_schema_id
                == format!("{ORACLE_ID_V1}-schema-{ORACLE_SCHEMA_VERSION}"),
        format!(
            "sources={} crops={} schema={}",
            committed.source_cases.len(),
            committed.crop_cases.len(),
            committed.oracle_schema_id
        ),
    );
    push(
        &mut checks,
        "lower_spin_no_full_bolometric",
        committed.source_cases.iter().all(|s| {
            s.definition.spin_a_over_m >= 0.999
                || s.definition.channel_set != OracleChannelSet::FullBolometricDisk
        }),
        "lower-spin full-bolometric absent".into(),
    );

    let out_a = "artifacts/r1-e0-oracle-corpus/eval-run-a";
    let out_b = "artifacts/r1-e0-oracle-corpus/eval-run-b";
    let out_serial = "artifacts/r1-e0-oracle-corpus/eval-run-serial";
    let out_cli = "artifacts/r1-e0-oracle-corpus/eval-run-cli";
    let _ = std::fs::remove_dir_all(root.join("artifacts/r1-e0-oracle-corpus"));

    regenerate_corpus_in_process(out_a, CliExecution::Parallel, Some(authoritative_threads))?;
    regenerate_corpus_in_process(out_b, CliExecution::Parallel, Some(authoritative_threads))?;
    regenerate_corpus_in_process(out_serial, CliExecution::Serial, Some(1))?;
    regenerate_corpus_via_cli(&root, out_cli, authoritative_threads)?;

    let lock_a_bytes = std::fs::read(root.join(out_a).join("corpus-lock-v1.json"))?;
    let lock_b_bytes = std::fs::read(root.join(out_b).join("corpus-lock-v1.json"))?;
    let lock_serial_bytes = std::fs::read(root.join(out_serial).join("corpus-lock-v1.json"))?;
    let lock_cli_bytes = std::fs::read(root.join(out_cli).join("corpus-lock-v1.json"))?;
    let regenerated_lock_digest = hex_sha(&Sha256::digest(&lock_a_bytes));

    push(
        &mut checks,
        "lock_matches_committed",
        lock_a_bytes == committed_lock_bytes,
        format!("regenerated={regenerated_lock_digest} committed={committed_lock_digest}"),
    );
    push(
        &mut checks,
        "repeated_generation_determinism",
        lock_a_bytes == lock_b_bytes,
        format!(
            "a={} b={}",
            hex_sha(&Sha256::digest(&lock_a_bytes)),
            hex_sha(&Sha256::digest(&lock_b_bytes))
        ),
    );
    push(
        &mut checks,
        "thread_execution_determinism",
        lock_a_bytes == lock_serial_bytes,
        format!(
            "parallel={} serial={}",
            hex_sha(&Sha256::digest(&lock_a_bytes)),
            hex_sha(&Sha256::digest(&lock_serial_bytes))
        ),
    );
    push(
        &mut checks,
        "subprocess_cli_determinism",
        lock_a_bytes == lock_cli_bytes,
        format!(
            "in_process={} cli={}",
            hex_sha(&Sha256::digest(&lock_a_bytes)),
            hex_sha(&Sha256::digest(&lock_cli_bytes))
        ),
    );

    // Committed experiments lock must remain untouched by evaluate regenerations.
    let committed_after = std::fs::read(root.join(COMMITTED_LOCK_PATH))?;
    push(
        &mut checks,
        "committed_lock_untouched",
        committed_after == committed_lock_bytes,
        "experiments/oracle-benchmark/corpus-lock-v1.json unchanged".into(),
    );

    let opaque_summary: ScientificSummary = serde_json::from_slice(&std::fs::read(
        root.join(out_a)
            .join("cases/kerr0999-edge-opaque/scientific-summary.json"),
    )?)?;
    let opaque_lock = committed
        .source_cases
        .iter()
        .find(|s| s.definition.id == "kerr0999-edge-opaque")
        .ok_or("missing kerr0999-edge-opaque in committed lock")?;
    push(
        &mut checks,
        "inherited_gate_digests_opaque_edge",
        opaque_summary.source_digests.numerical_profile_digest == REF_NUMERICAL_PROFILE
            && opaque_summary.source_digests.outcome_class_digest == REF_CLASS
            && opaque_summary.source_digests.celestial_coordinate_digest == REF_COORD
            && opaque_summary.source_digests.trace_data_digest == REF_TRACE_DATA
            && opaque_summary
                .source_digests
                .frequency_shift_digest
                .as_deref()
                == Some(REF_FREQ)
            && opaque_summary.source_digests.bolometric_digest.as_deref() == Some(REF_BOLO)
            && opaque_summary.outcome_counts == REF_COUNTS
            && opaque_lock.reference_image_digest == REF_COMPOSITE_PPM
            && opaque_lock.oracle_scientific_digest == opaque_summary.oracle_scientific_digest
            && counts_eq(&opaque_lock.outcome_counts, &REF_COUNTS),
        format!(
            "bolo={} freq={} class={} image={}",
            opaque_summary
                .source_digests
                .bolometric_digest
                .as_deref()
                .unwrap_or("-"),
            opaque_summary
                .source_digests
                .frequency_shift_digest
                .as_deref()
                .unwrap_or("-"),
            opaque_summary.source_digests.outcome_class_digest,
            opaque_lock.reference_image_digest
        ),
    );

    let mut validated_frames = 0u64;
    for case in &committed.source_cases {
        let path = root
            .join(out_a)
            .join("cases")
            .join(&case.definition.id)
            .join("oracle-frame.json");
        let frame: OracleFrame = serde_json::from_slice(&std::fs::read(&path)?)?;
        frame.validate()?;
        if frame.scientific_digest != case.oracle_scientific_digest {
            return Err(format!(
                "oracle digest mismatch for {}: frame={} lock={}",
                case.definition.id, frame.scientific_digest, case.oracle_scientific_digest
            )
            .into());
        }
        validated_frames += 1;
    }
    for crop in &committed.crop_cases {
        let path = root
            .join(out_a)
            .join("crops")
            .join(&crop.id)
            .join("oracle-frame.json");
        let frame: OracleFrame = serde_json::from_slice(&std::fs::read(&path)?)?;
        frame.validate()?;
        if frame.scientific_digest != crop.oracle_scientific_digest {
            return Err(format!(
                "crop digest mismatch for {}: frame={} lock={}",
                crop.id, frame.scientific_digest, crop.oracle_scientific_digest
            )
            .into());
        }
        validated_frames += 1;
    }
    push(
        &mut checks,
        "oracle_frames_validate_against_lock",
        validated_frames == 8,
        format!("validated_frames={validated_frames}"),
    );

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let authoritative = all_pass && !dirty && self_release;
    let mut report = R1E0Eval {
        gate: "r1-e0-oracle-corpus".into(),
        result: if all_pass { "PASS" } else { "FAIL" }.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        build,
        available_threads: available,
        authoritative_threads,
        committed_lock_digest,
        regenerated_lock_digest,
        checks,
        content_digest_excluding_digest_field: String::new(),
    };
    finalize(&root, &mut report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.result != "PASS" {
        return Err("r1-e0-oracle-corpus evaluation FAIL".into());
    }
    Ok(())
}

fn regenerate_corpus_in_process(
    output_dir: &str,
    execution: CliExecution,
    threads: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    oracle_benchmark::run(MANIFEST_PATH, output_dir, execution, threads, true, false)
}

fn regenerate_corpus_via_cli(
    root: &Path,
    output_dir: &str,
    threads: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let committed_before = std::fs::read(root.join(COMMITTED_LOCK_PATH))?;
    let threads_s = threads.to_string();
    let args = [
        "run",
        "--release",
        "-q",
        "-p",
        "xtask",
        "--",
        "oracle-benchmark-corpus",
        "--manifest",
        MANIFEST_PATH,
        "--output-dir",
        output_dir,
        "--execution",
        "parallel",
        "--threads",
        threads_s.as_str(),
        "--require-release",
    ];
    let out = Command::new("cargo")
        .current_dir(root)
        .args(args)
        .output()?;
    std::fs::write(root.join(COMMITTED_LOCK_PATH), &committed_before)?;
    if !out.status.success() {
        return Err(format!(
            "oracle-benchmark-corpus subprocess failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(())
}

fn counts_eq(a: &OutcomeCounts, b: &OutcomeCounts) -> bool {
    a.disk_hit == b.disk_hit
        && a.escaped == b.escaped
        && a.horizon_event == b.horizon_event
        && a.horizon_approach == b.horizon_approach
        && a.affine_limit == b.affine_limit
        && a.failed == b.failed
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
    let ok = out.status.success();
    push(
        checks,
        name,
        ok,
        if ok {
            "ok".into()
        } else {
            format!(
                "status={} stderr={}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            )
        },
    );
    Ok(())
}

fn empty(
    build: &BuildExecutionMetadata,
    commit: &str,
    dirty: bool,
    dirty_detail: String,
    checks: Vec<Check>,
) -> R1E0Eval {
    R1E0Eval {
        gate: "r1-e0-oracle-corpus".into(),
        result: "FAIL".into(),
        authoritative: false,
        commit: commit.into(),
        dirty,
        dirty_detail,
        build: build.clone(),
        available_threads: 0,
        authoritative_threads: 0,
        committed_lock_digest: String::new(),
        regenerated_lock_digest: String::new(),
        checks,
        content_digest_excluding_digest_field: String::new(),
    }
}

fn finalize(root: &Path, report: &mut R1E0Eval) -> Result<(), Box<dyn std::error::Error>> {
    report.content_digest_excluding_digest_field = eval_digest(report);
    let dir = root.join("artifacts/r1-e0-oracle-corpus");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("evaluation.json"),
        serde_json::to_vec_pretty(report)?,
    )?;
    let mut md = String::new();
    md.push_str("# R1/E0 oracle corpus evaluation\n\n");
    md.push_str(&format!(
        "- result: `{}` authoritative=`{}` dirty=`{}`\n",
        report.result, report.authoritative, report.dirty
    ));
    md.push_str(&format!("- commit: `{}`\n", report.commit));
    md.push_str(&format!(
        "- committed_lock_digest: `{}`\n",
        report.committed_lock_digest
    ));
    md.push_str(&format!(
        "- regenerated_lock_digest: `{}`\n",
        report.regenerated_lock_digest
    ));
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

fn eval_digest(report: &R1E0Eval) -> String {
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
        committed_lock_digest: &'a str,
        regenerated_lock_digest: &'a str,
        checks: Vec<DigestCheck<'a>>,
    }
    let proj = Proj {
        gate: &report.gate,
        result: &report.result,
        authoritative: report.authoritative,
        commit: &report.commit,
        dirty: report.dirty,
        build: &report.build,
        available_threads: report.available_threads,
        authoritative_threads: report.authoritative_threads,
        committed_lock_digest: &report.committed_lock_digest,
        regenerated_lock_digest: &report.regenerated_lock_digest,
        checks: report
            .checks
            .iter()
            .map(|c| DigestCheck {
                name: &c.name,
                status: c.status,
            })
            .collect(),
    };
    hex_sha(&Sha256::digest(serde_json::to_vec(&proj).unwrap()))
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
        return Err(format!("git {:?} failed", args).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

fn workspace_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask manifest has no parent")?
        .to_path_buf())
}
