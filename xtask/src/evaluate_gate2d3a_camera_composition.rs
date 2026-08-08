//! Gate 2D3A camera composition evaluator — Phase A stop or Phase B authoritative PASS.

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
/// Frozen Phase A search spec (D3A-A3 / D3A-C1) — exact pin, not merely hex-shaped.
const REF_CAMERA_SEARCH_SPEC: &str =
    "bc5b9257492310c612e2ac26d58926b761d31ff4acbd3fe5f2e77d98a3d9191b";

/// Frozen hero digests (gate 128²; CAMERA_DERIVED — not 2C1 scientific authority).
const REF_HERO_PRESENTATION_FRAME: &str =
    "fae0afdd2b16a1ff8c086303edbf633e675595f6e81620dde483e690e7266544";
const REF_HERO_SCENE_APPEARANCE: &str =
    "b3c8f30afd3575215a8c75d2c5e82a0710f739e4d330a88e575de7880ccede84";
const REF_HERO_CAMERA_SPEC: &str =
    "42d3e3f8cc5d7b11950439ab46a850d6f5e2865f8e37a29ba6570f01b9ad2578";

const OWNER_SELECTED_CANDIDATE: &str = "c024";

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
    owner_selected_candidate_id: Option<String>,
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
        spec_digest == REF_CAMERA_SEARCH_SPEC,
        format!("got={spec_digest} want={REF_CAMERA_SEARCH_SPEC}"),
    );

    // Baseline beauty with camera overlay (scientific pins — D3A-A2).
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

    let mut shortlist_ids: Vec<String> = Vec::new();
    if phase_present {
        let pa: serde_json::Value = serde_json::from_slice(&std::fs::read(&phase_a)?)?;
        push(
            &mut checks,
            "phase_a_baseline_exact_flag",
            pa["baseline_exact"].as_bool() == Some(true),
            format!("{}", pa["baseline_exact"]),
        );
        let status_ok = matches!(
            pa["status"].as_str(),
            Some("STOP_FOR_OWNER_HERO_SELECTION") | Some("CAMERA_PARAMETERIZATION_INSUFFICIENT")
        );
        push(
            &mut checks,
            "phase_a_status_recorded",
            status_ok,
            pa["status"].as_str().unwrap_or("").into(),
        );
        if let Some(arr) = pa["shortlist_ids"].as_array() {
            shortlist_ids = arr
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
        }
        push(
            &mut checks,
            "phase_a_shortlist_nonempty_or_insufficient",
            !shortlist_ids.is_empty()
                || pa["parameterization_insufficient"].as_bool() == Some(true),
            format!("shortlist_n={}", shortlist_ids.len()),
        );
        push(
            &mut checks,
            "phase_a_search_spec_digest_match",
            pa["camera_search_spec_digest"].as_str() == Some(spec_digest.as_str()),
            format!("{}", pa["camera_search_spec_digest"]),
        );
    }

    push(
        &mut checks,
        "documented_2d0_identity_digest",
        REF_IDENTITY_2D0.len() == 64,
        REF_IDENTITY_2D0.into(),
    );

    let mut owner_selected: Option<String> = None;

    if !hero_frozen {
        push(
            &mut checks,
            "a7_hero_not_frozen_before_owner_selection",
            true,
            "hero absent (expected in Phase A)".into(),
        );
    } else {
        let hero = load_camera_composition_preset(&hero_path)?;
        let hero_digest = camera_spec_digest(&hero);
        owner_selected = hero.source_candidate_id.clone();
        let selected = owner_selected.as_deref().unwrap_or("");

        push(
            &mut checks,
            "a7_hero_frozen_after_owner_selection",
            true,
            "hero preset present".into(),
        );
        push(
            &mut checks,
            "hero_role_is_hero_camera",
            hero.role == CameraRole::HeroCamera,
            hero.role.as_str().into(),
        );
        push(
            &mut checks,
            "hero_source_candidate_matches_owner",
            selected == OWNER_SELECTED_CANDIDATE,
            format!("source={selected} owner={OWNER_SELECTED_CANDIDATE}"),
        );
        push(
            &mut checks,
            "hero_source_in_phase_a_shortlist",
            shortlist_ids.iter().any(|id| id == selected),
            format!("selected={selected} shortlist={shortlist_ids:?}"),
        );

        let cand = candidates.iter().find(|c| c.id == selected);
        let params_ok = match cand {
            Some(c) => {
                (hero.observer.boyer_lindquist_r - c.r_over_m).abs() < 1e-12
                    && (hero.observer.boyer_lindquist_theta_degrees - c.theta_degrees).abs() < 1e-12
                    && (hero.observer.boyer_lindquist_phi_degrees - c.phi_degrees).abs() < 1e-12
                    && (hero.camera.horizontal_field_of_view_degrees - c.hfov_degrees).abs() < 1e-12
            }
            None => false,
        };
        push(
            &mut checks,
            "hero_params_match_selected_candidate",
            params_ok,
            format!("candidate={selected}"),
        );

        // Pin camera_spec_digest (must be real hex once placeholder replaced).
        let spec_pin_ok = REF_HERO_CAMERA_SPEC.len() == 64
            && REF_HERO_CAMERA_SPEC.chars().all(|c| c.is_ascii_hexdigit())
            && hero_digest == REF_HERO_CAMERA_SPEC;
        push(
            &mut checks,
            "hero_camera_spec_digest",
            spec_pin_ok,
            format!("got={hero_digest} want={REF_HERO_CAMERA_SPEC}"),
        );

        let hero_status = Command::new("cargo")
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
                "presets/camera/gargantua-hero-v1.toml",
                "--tier",
                "gate",
                "--output-dir",
                "artifacts/gate-2d3a-camera-composition/hero-eval",
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
            "hero_camera_render_ok",
            hero_status.success(),
            format!("exit={hero_status}"),
        );

        let hero_report_path =
            root.join("artifacts/gate-2d3a-camera-composition/hero-eval/appearance-report.json");
        let hero_report: serde_json::Value = if hero_report_path.is_file() {
            serde_json::from_slice(&std::fs::read(&hero_report_path)?)?
        } else {
            serde_json::json!({})
        };
        let h_frame = hero_report["presentation_frame_digest"]
            .as_str()
            .unwrap_or("");
        let h_scene = hero_report["scene_appearance_digest"]
            .as_str()
            .unwrap_or("");
        let h_pres = hero_report["presentation_spec_digest"]
            .as_str()
            .unwrap_or("");
        let h_note = hero_report["note"].as_str().unwrap_or("");

        push(
            &mut checks,
            "hero_presentation_frame_digest",
            h_frame == REF_HERO_PRESENTATION_FRAME,
            h_frame.into(),
        );
        push(
            &mut checks,
            "hero_scene_appearance_digest",
            h_scene == REF_HERO_SCENE_APPEARANCE,
            h_scene.into(),
        );
        push(
            &mut checks,
            "hero_frame_differs_from_baseline",
            h_frame != REF_SCENE_2D1 && !h_frame.is_empty(),
            format!("hero={h_frame} baseline={REF_SCENE_2D1}"),
        );
        push(
            &mut checks,
            "hero_inherits_presentation_spec",
            h_pres == REF_PRESENTATION_SPEC,
            h_pres.into(),
        );
        push(
            &mut checks,
            "hero_authority_label_camera_derived",
            h_note.contains("CAMERA_DERIVED") || h_note.contains("camera-derived"),
            h_note.into(),
        );

        // Composition metrics from Phase A gate contact (material move vs baseline class mix).
        let contact = root.join(format!(
            "artifacts/gate-2d3a-camera-composition/shortlist-contact/03-{OWNER_SELECTED_CANDIDATE}.json"
        ));
        // Rank may vary; find any shortlist-contact file for the selected id.
        let contact_path = if contact.is_file() {
            contact
        } else {
            std::fs::read_dir(&shortlist_dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains(OWNER_SELECTED_CANDIDATE))
                })
                .unwrap_or(contact)
        };
        if contact_path.is_file() {
            let c: serde_json::Value = serde_json::from_slice(&std::fs::read(&contact_path)?)?;
            let disk = c["gate_metrics"]["disk_hit_fraction"]
                .as_f64()
                .unwrap_or(1.0);
            let esc = c["gate_metrics"]["escaped_fraction"]
                .as_f64()
                .unwrap_or(0.0);
            // D3A-A6 / D3A-C2: class fractions are SEARCH_GUIDANCE evidence only — never gate truth.
            push_diagnostic(
                &mut checks,
                "hero_class_fractions_vs_baseline_diagnostic",
                format!(
                    "disk={disk:.4} esc={esc:.4} (baseline ~0.751/0.149); SEARCH_GUIDANCE_NOT_GATE_TRUTH"
                ),
            );
            let failed = c["gate_metrics"]["failed_count"].as_u64().unwrap_or(1);
            let affine = c["gate_metrics"]["affine_limit_count"]
                .as_u64()
                .unwrap_or(1);
            push(
                &mut checks,
                "hero_numerical_failures_zero",
                failed == 0 && affine == 0,
                format!("failed={failed} affine_limit={affine}"),
            );
        } else {
            push_diagnostic(
                &mut checks,
                "hero_class_fractions_vs_baseline_diagnostic",
                "missing shortlist-contact metrics; SEARCH_GUIDANCE_NOT_GATE_TRUTH".into(),
            );
            push(
                &mut checks,
                "hero_numerical_failures_zero",
                false,
                "missing shortlist-contact metrics".into(),
            );
        }

        let beauty =
            root.join("artifacts/gate-2d3a-camera-composition/hero-eval/beauty-scene-srgb16.png");
        push(
            &mut checks,
            "hero_beauty_artifact_present",
            beauty.is_file(),
            beauty.display().to_string(),
        );
    }

    let sci_ok = [
        "a2_baseline_physical_color_digest",
        "a2_baseline_payload_sha256",
    ]
    .iter()
    .all(|n| check_pass(&checks, n));
    let pres_ok = [
        "inherit_2d0_presentation_spec_digest",
        "a1_baseline_presentation_frame_digest",
    ]
    .iter()
    .all(|n| check_pass(&checks, n));

    let phase_a_complete = phase_present
        && checks
            .iter()
            .filter(|c| c.name.starts_with("phase_a_"))
            .all(|c| c.status == "PASS");

    // DIAGNOSTIC rows (D3A-A6 guidance evidence) must not veto authoritative PASS (D3A-C2).
    let all_pass = checks
        .iter()
        .filter(|c| c.status != "DIAGNOSTIC")
        .all(|c| c.status == "PASS");

    let (phase, result, authoritative, pending) = if hero_frozen {
        let auth = all_pass && !dirty && build.is_optimized_release_execution();
        (
            "PHASE_B",
            if auth {
                "PASS"
            } else if all_pass {
                "PASS_NON_AUTHORITATIVE"
            } else {
                "FAIL"
            },
            auth,
            false,
        )
    } else {
        let result = if all_pass && phase_a_complete {
            "PHASE_A_COMPLETE_AWAITING_OWNER_HERO_SELECTION"
        } else if all_pass && !phase_present {
            "PHASE_A_INFRA_PASS_RUN_CAMERA_SEARCH"
        } else {
            "FAIL"
        };
        ("PHASE_A", result, false, true)
    };

    let camera_pipeline = if hero_frozen {
        if check_pass(&checks, "hero_presentation_frame_digest")
            && check_pass(&checks, "a7_hero_frozen_after_owner_selection")
        {
            "CAMERA_PIPELINE_PHASE_B_PASS"
        } else {
            "CAMERA_PIPELINE_FAIL"
        }
    } else if check_pass(&checks, "a7_hero_not_frozen_before_owner_selection")
        && check_pass(&checks, "baseline_camera_role")
    {
        "CAMERA_PIPELINE_PHASE_A_PASS"
    } else {
        "CAMERA_PIPELINE_FAIL"
    };

    let mut report = Gate2d3aEval {
        gate: "gate-2d3a-camera-composition".into(),
        phase: phase.into(),
        result: result.into(),
        authoritative,
        commit: commit.trim().into(),
        dirty,
        dirty_detail,
        owner_hero_selection_pending: pending,
        hero_frozen,
        owner_selected_candidate_id: owner_selected,
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
        camera_pipeline: camera_pipeline.into(),
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
    if report.result == "PASS" || report.result == "PASS_NON_AUTHORITATIVE" {
        return Ok(());
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
            owner_selected_candidate_id: self.owner_selected_candidate_id.clone(),
            scientific_inheritance: self.scientific_inheritance.clone(),
            presentation_inheritance: self.presentation_inheritance.clone(),
            camera_pipeline: self.camera_pipeline.clone(),
            build: self.build.clone(),
            checks: self.checks.clone(),
            content_digest_excluding_digest_field: String::new(),
        }
    }
}

fn check_pass(checks: &[Check], name: &str) -> bool {
    checks
        .iter()
        .find(|c| c.name == name)
        .is_some_and(|c| c.status == "PASS")
}

fn push(checks: &mut Vec<Check>, name: &str, ok: bool, detail: String) {
    checks.push(Check {
        name: name.into(),
        status: if ok { "PASS" } else { "FAIL" }.into(),
        detail,
    });
}

fn push_diagnostic(checks: &mut Vec<Check>, name: &str, detail: String) {
    checks.push(Check {
        name: name.into(),
        status: "DIAGNOSTIC".into(),
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
