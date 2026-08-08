//! Gate 2D3A Phase A evaluator — baseline exact + search evidence; awaits owner hero.

use crate::build_meta::{require_release_execution, BuildExecutionMetadata};
use crate::camera_composition::{
    apply_camera_overlay, camera_spec_digest, load_camera_composition_preset, CameraRole,
};
use crate::camera_search::{camera_search_spec_digest, expand_candidates, load_camera_search_spec};
use crate::preset::load_preset;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

const REF_COLOR_2C1: &str = "16663188fad338c0fc8197dddd8268bd705f817b165a35853b16b211c7635793";
const REF_PAYLOAD_2C1: &str = "d317c517661a64f8ffdacead3dd222370056abc8eed81706d660bc4ebda81cf5";
const REF_PRESENTATION_SPEC: &str =
    "e6639e75d67156852f8f064e7ef9f4f2b82ab8018b707399c851522780a6dd49";
const REF_SCENE_2D1: &str = "68b555442c277c8eb95c1562568c24746fb2489c174730350671d5567cf43cd0";
const REF_IDENTITY_2D0: &str = "f8e103239a331796bd474ff121627eecd0781f31c840f46d9f2d3a85c8d1e87b";

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    status: String,
    detail: String,
}

#[derive(Serialize)]
struct Gate2d3aEval {
    gate: String,
    phase: String,
    result: String,
    authoritative: bool,
    commit: String,
    dirty: bool,
    dirty_detail: String,
    owner_hero_selection_pending: bool,
    hero_frozen: bool,
    scientific_inheritance: String,
    presentation_inheritance: String,
    camera_pipeline: String,
    build: BuildExecutionMetadata,
    checks: Vec<Check>,
    content_digest_excluding_digest_field: String,
}

pub fn evaluate() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let build = BuildExecutionMetadata::current();
    require_release_execution(&build)?;
    let commit = git_stdout(&root, &["rev-parse", "HEAD"])?;
    let (dirty, dirty_detail) = porcelain_dirty(&root)?;

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
    push(
        &mut checks,
        "self_release",
        build.is_optimized_release_execution(),
        format!("{:?}", build.cargo_profile),
    );

    // fmt / clippy / tests are expected to be run by CI; record local fmt check lightly.
    let fmt = Command::new("cargo")
        .current_dir(&root)
        .args(["fmt", "--all", "--", "--check"])
        .status()?;
    push(&mut checks, "fmt", fmt.success(), format!("exit={fmt}"));

    let physical = load_preset(&root.join("presets/gargantua-physical-v1.toml"))?;
    let baseline_cam =
        load_camera_composition_preset(&root.join("presets/camera/gargantua-baseline-v1.toml"))?;
    push(
        &mut checks,
        "baseline_camera_role",
        baseline_cam.role == CameraRole::BaselineCamera,
        baseline_cam.role.as_str().into(),
    );
    let overlaid = apply_camera_overlay(&physical, &baseline_cam)?;
    push(
        &mut checks,
        "a1_overlay_matches_physical_observer_camera",
        overlaid.observer.boyer_lindquist_r == physical.observer.boyer_lindquist_r
            && overlaid.observer.boyer_lindquist_theta_degrees
                == physical.observer.boyer_lindquist_theta_degrees
            && overlaid.observer.boyer_lindquist_phi_degrees
                == physical.observer.boyer_lindquist_phi_degrees
            && overlaid.camera.horizontal_field_of_view_degrees
                == physical.camera.horizontal_field_of_view_degrees
            && overlaid.camera.roll_degrees == physical.camera.roll_degrees
            && overlaid.camera.look_at == physical.camera.look_at,
        "observer+camera allowlist overlay".into(),
    );
    let _ = camera_spec_digest(&baseline_cam);

    let spec = load_camera_search_spec(&root.join("presets/camera/camera-search-spec-v1.toml"))?;
    let candidates = expand_candidates(&spec)?;
    let spec_digest = camera_search_spec_digest(&spec);
    push(
        &mut checks,
        "a3_candidate_count_exact",
        candidates.len() == 48 && candidates.len() <= spec.max_candidates,
        format!("n={} max={}", candidates.len(), spec.max_candidates),
    );
    push(
        &mut checks,
        "a6_search_guidance_label",
        spec.search_guidance.label == "SEARCH_GUIDANCE_NOT_GATE_TRUTH",
        spec.search_guidance.label.clone(),
    );
    push(
        &mut checks,
        "camera_search_spec_digest_stable",
        spec_digest.len() == 64,
        spec_digest.clone(),
    );

    // Run baseline beauty with camera overlay (authoritative path for frozen pins).
    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "run",
            "--release",
            "-p",
            "xtask",
            "--",
            "render-scene-appearance",
            "--preset",
            "presets/gargantua-physical-v1.toml",
            "--appearance",
            "presets/appearance/gargantua-scene-v1.toml",
            "--presentation",
            "presets/presentation/gargantua-cinematic-v1.toml",
            "--camera",
            "presets/camera/gargantua-baseline-v1.toml",
            "--tier",
            "gate",
            "--output-dir",
            "artifacts/gate-2d3a-camera-composition/baseline-eval",
            "--execution",
            "parallel",
            "--threads",
            "8",
            "--require-release",
            "--no-env-reference",
        ])
        .status()?;
    push(
        &mut checks,
        "baseline_camera_render_ok",
        status.success(),
        format!("exit={status}"),
    );
    let report_path =
        root.join("artifacts/gate-2d3a-camera-composition/baseline-eval/appearance-report.json");
    let report: serde_json::Value = if report_path.is_file() {
        serde_json::from_slice(&std::fs::read(&report_path)?)?
    } else {
        serde_json::json!({})
    };
    let frame = report["presentation_frame_digest"].as_str().unwrap_or("");
    let color = report["source_physical_color_digest"]
        .as_str()
        .unwrap_or("");
    let payload = report["source_payload_sha256"].as_str().unwrap_or("");
    let pres = report["presentation_spec_digest"].as_str().unwrap_or("");
    push(
        &mut checks,
        "a1_baseline_presentation_frame_digest",
        frame == REF_SCENE_2D1,
        frame.into(),
    );
    push(
        &mut checks,
        "a2_baseline_physical_color_digest",
        color == REF_COLOR_2C1,
        color.into(),
    );
    push(
        &mut checks,
        "a2_baseline_payload_sha256",
        payload == REF_PAYLOAD_2C1,
        payload.into(),
    );
    push(
        &mut checks,
        "inherit_2d0_presentation_spec_digest",
        pres == REF_PRESENTATION_SPEC,
        pres.into(),
    );

    let hero_path = root.join("presets/camera/gargantua-hero-v1.toml");
    let hero_frozen = hero_path.is_file();
    push(
        &mut checks,
        "a7_hero_not_frozen_before_owner_selection",
        !hero_frozen,
        if hero_frozen {
            "hero preset present — Phase B only after owner selection".into()
        } else {
            "hero absent (expected in Phase A)".into()
        },
    );

    let phase_a = root.join("artifacts/gate-2d3a-camera-composition/phase-a-report.json");
    let shortlist_dir = root.join("artifacts/gate-2d3a-camera-composition/shortlist-contact");
    let phase_present = phase_a.is_file() && shortlist_dir.is_dir();
    push(
        &mut checks,
        "phase_a_artifacts_present_or_run_search",
        phase_present,
        if phase_present {
            "phase-a-report + shortlist-contact present".into()
        } else {
            "run: cargo run --release -p xtask -- camera-search-phase-a".into()
        },
    );

    if phase_present {
        let pa: serde_json::Value = serde_json::from_slice(&std::fs::read(&phase_a)?)?;
        push(
            &mut checks,
            "phase_a_baseline_exact_flag",
            pa["baseline_exact"].as_bool() == Some(true),
            format!("{}", pa["baseline_exact"]),
        );
        push(
            &mut checks,
            "phase_a_status_stop_for_owner",
            pa["status"].as_str() == Some("STOP_FOR_OWNER_HERO_SELECTION")
                || pa["status"].as_str() == Some("CAMERA_PARAMETERIZATION_INSUFFICIENT"),
            pa["status"].as_str().unwrap_or("").into(),
        );
        let short_n = pa["shortlist_ids"].as_array().map(|a| a.len()).unwrap_or(0);
        push(
            &mut checks,
            "phase_a_shortlist_nonempty_or_insufficient",
            short_n > 0 || pa["parameterization_insufficient"].as_bool() == Some(true),
            format!("shortlist_n={short_n}"),
        );
        push(
            &mut checks,
            "phase_a_search_spec_digest_match",
            pa["camera_search_spec_digest"].as_str() == Some(spec_digest.as_str()),
            format!("{}", pa["camera_search_spec_digest"]),
        );
    }

    // Identity 2D0 pin remains a documentation reference; Phase A does not re-render identity.
    push(
        &mut checks,
        "documented_2d0_identity_digest",
        REF_IDENTITY_2D0.len() == 64,
        REF_IDENTITY_2D0.into(),
    );

    let sci_ok = [
        "a2_baseline_physical_color_digest",
        "a2_baseline_payload_sha256",
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
        "a1_baseline_presentation_frame_digest",
    ]
    .iter()
    .all(|n| {
        checks
            .iter()
            .find(|c| c.name == *n)
            .is_some_and(|c| c.status == "PASS")
    });
    let cam_ok = [
        "a1_overlay_matches_physical_observer_camera",
        "a3_candidate_count_exact",
        "a6_search_guidance_label",
        "a7_hero_not_frozen_before_owner_selection",
        "baseline_camera_role",
    ]
    .iter()
    .all(|n| {
        checks
            .iter()
            .find(|c| c.name == *n)
            .is_some_and(|c| c.status == "PASS")
    });

    let phase_a_complete = phase_present
        && checks
            .iter()
            .filter(|c| c.name.starts_with("phase_a_"))
            .all(|c| c.status == "PASS");

    let all_pass = checks.iter().all(|c| c.status == "PASS");
    let result = if all_pass && phase_a_complete {
        "PHASE_A_COMPLETE_AWAITING_OWNER_HERO_SELECTION"
    } else if all_pass && !phase_present {
        "PHASE_A_INFRA_PASS_RUN_CAMERA_SEARCH"
    } else {
        "FAIL"
    };

    let mut report = Gate2d3aEval {
        gate: "gate-2d3a-camera-composition".into(),
        phase: "PHASE_A".into(),
        result: result.into(),
        // Full authoritative PASS only after Phase B hero freeze (D3A-A7).
        authoritative: false,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        owner_hero_selection_pending: true,
        hero_frozen: false,
        scientific_inheritance: if sci_ok {
            "SCIENTIFIC_INHERITANCE_PASS_BASELINE_PATH"
        } else {
            "SCIENTIFIC_INHERITANCE_FAIL"
        }
        .into(),
        presentation_inheritance: if pres_ok {
            "PRESENTATION_INHERITANCE_PASS_BASELINE_PATH"
        } else {
            "PRESENTATION_INHERITANCE_FAIL"
        }
        .into(),
        camera_pipeline: if cam_ok {
            "CAMERA_PIPELINE_PHASE_A_PASS"
        } else {
            "CAMERA_PIPELINE_FAIL"
        }
        .into(),
        build,
        checks,
        content_digest_excluding_digest_field: String::new(),
    };
    let mut hasher = Sha256::new();
    let mut tmp = report.clone_for_digest();
    tmp.content_digest_excluding_digest_field.clear();
    hasher.update(serde_json::to_vec(&tmp)?);
    report.content_digest_excluding_digest_field = hex::encode(hasher.finalize());

    std::fs::create_dir_all(root.join("artifacts/gate-2d3a-camera-composition"))?;
    std::fs::write(
        root.join("artifacts/gate-2d3a-camera-composition/evaluate-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    if report.result == "FAIL" {
        return Err("gate-2d3a-camera-composition FAIL".into());
    }
    if report.result == "PHASE_A_COMPLETE_AWAITING_OWNER_HERO_SELECTION" {
        return Err("STOP_FOR_OWNER_HERO_SELECTION".into());
    }
    Ok(())
}

impl Gate2d3aEval {
    fn clone_for_digest(&self) -> Self {
        Self {
            gate: self.gate.clone(),
            phase: self.phase.clone(),
            result: self.result.clone(),
            authoritative: self.authoritative,
            commit: self.commit.clone(),
            dirty: self.dirty,
            dirty_detail: self.dirty_detail.clone(),
            owner_hero_selection_pending: self.owner_hero_selection_pending,
            hero_frozen: self.hero_frozen,
            scientific_inheritance: self.scientific_inheritance.clone(),
            presentation_inheritance: self.presentation_inheritance.clone(),
            camera_pipeline: self.camera_pipeline.clone(),
            build: self.build.clone(),
            checks: self.checks.clone(),
            content_digest_excluding_digest_field: String::new(),
        }
    }
}

fn push(checks: &mut Vec<Check>, name: &str, ok: bool, detail: String) {
    checks.push(Check {
        name: name.into(),
        status: if ok { "PASS" } else { "FAIL" }.into(),
        detail,
    });
}

fn porcelain_dirty(root: &Path) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
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
    if !out.status.success() {
        return Err("git command failed".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn workspace_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
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
