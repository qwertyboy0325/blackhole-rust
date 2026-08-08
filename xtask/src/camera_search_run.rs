//! Gate 2D3A Phase A: deterministic camera candidate sweep → owner shortlist.

use crate::camera_composition::{
    camera_spec_digest, candidate_camera_preset, CameraCompositionPreset,
};
use crate::camera_search::{
    camera_search_spec_digest, expand_candidates, gate_reject_reason, load_camera_search_spec,
    select_shortlist, shortlist_key, smoke_reject_reason, CandidateStageResult, CompositionMetrics,
    SearchCandidate,
};
use crate::composition_metrics::composition_metrics_from_scene;
use crate::render_scene_appearance::{render, RenderedSceneAppearance};
use crate::trace_outcome_map::CliExecution;
use serde::Serialize;
use std::path::{Path, PathBuf};

const PHYSICAL: &str = "presets/gargantua-physical-v1.toml";
const APPEARANCE: &str = "presets/appearance/gargantua-scene-v1.toml";
const PRESENTATION: &str = "presets/presentation/gargantua-cinematic-v1.toml";
const SEARCH_SPEC: &str = "presets/camera/camera-search-spec-v1.toml";
const BASELINE_CAMERA: &str = "presets/camera/gargantua-baseline-v1.toml";

#[derive(Serialize)]
struct PhaseAReport {
    gate: &'static str,
    phase: &'static str,
    status: &'static str,
    stop_reason: &'static str,
    camera_search_spec_digest: String,
    candidate_count: usize,
    smoke_survivors: usize,
    gate_valid_count: usize,
    shortlist_ids: Vec<String>,
    baseline_presentation_frame_digest: String,
    baseline_exact: bool,
    parameterization_insufficient: bool,
    note: &'static str,
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

fn write_candidate_toml(
    path: &Path,
    cam: &CameraCompositionPreset,
) -> Result<(), Box<dyn std::error::Error>> {
    let text = format!(
        r#"schema_version = {}
camera_preset_id = "{}"
role = "{}"
description = {:?}

[observer]
motion = "{}"
boyer_lindquist_r = {}
boyer_lindquist_theta_degrees = {}
boyer_lindquist_phi_degrees = {}

[camera]
projection = "{}"
horizontal_field_of_view_degrees = {}
look_at = "{}"
roll_degrees = {}
"#,
        cam.schema_version,
        cam.camera_preset_id,
        cam.role.as_str(),
        cam.description.clone().unwrap_or_default(),
        cam.observer.motion,
        cam.observer.boyer_lindquist_r,
        cam.observer.boyer_lindquist_theta_degrees,
        cam.observer.boyer_lindquist_phi_degrees,
        cam.camera.projection,
        cam.camera.horizontal_field_of_view_degrees,
        cam.camera.look_at,
        cam.camera.roll_degrees,
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    Ok(())
}

fn metrics_from_rendered(
    rendered: &RenderedSceneAppearance,
    cam_digest: &str,
    authority_label: &'static str,
) -> CompositionMetrics {
    composition_metrics_from_scene(
        &rendered.scene_frame,
        &rendered.presented,
        &rendered.source_physical_color_digest,
        cam_digest,
        authority_label,
    )
}

fn render_candidate(
    root: &Path,
    out_rel: &str,
    cam: &CameraCompositionPreset,
    width: u32,
    height: u32,
    threads: Option<usize>,
) -> Result<RenderedSceneAppearance, Box<dyn std::error::Error>> {
    let cam_path = root.join(out_rel).join("camera-preset.toml");
    write_candidate_toml(&cam_path, cam)?;
    let cam_rel = pathdiff_rel(root, &cam_path)?;
    render(
        PHYSICAL,
        APPEARANCE,
        PRESENTATION,
        None,
        Some(width),
        Some(height),
        out_rel,
        true,
        CliExecution::Parallel,
        threads,
        false,
        false,
        Some(&cam_rel),
        true,
    )
}

fn pathdiff_rel(root: &Path, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

pub fn run_phase_a(threads: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    let out_root = root.join("artifacts/gate-2d3a-camera-composition");
    std::fs::create_dir_all(&out_root)?;

    let spec = load_camera_search_spec(&root.join(SEARCH_SPEC))?;
    let spec_digest = camera_search_spec_digest(&spec);
    let candidates = expand_candidates(&spec)?;
    println!(
        "Gate 2D3A Phase A: {} candidates, search_spec_digest={}",
        candidates.len(),
        spec_digest
    );

    // Baseline exact check (D3A-A1).
    let baseline = render(
        PHYSICAL,
        APPEARANCE,
        PRESENTATION,
        Some(crate::render_tier::DiagnosticRenderTier::Gate),
        None,
        None,
        "artifacts/gate-2d3a-camera-composition/baseline",
        true,
        CliExecution::Parallel,
        threads,
        false,
        false,
        Some(BASELINE_CAMERA),
        true,
    )?;
    const REF_SCENE: &str = "68b555442c277c8eb95c1562568c24746fb2489c174730350671d5567cf43cd0";
    let baseline_exact = baseline.report.presentation_frame_digest == REF_SCENE;
    if !baseline_exact {
        return Err(format!(
            "D3A-A1 FAIL: baseline overlay presentation_frame_digest={} expected {}",
            baseline.report.presentation_frame_digest, REF_SCENE
        )
        .into());
    }
    println!("baseline exact PASS ({REF_SCENE})");

    let mut results: Vec<CandidateStageResult> = Vec::with_capacity(candidates.len());
    let auth_threads =
        threads.or_else(|| std::thread::available_parallelism().ok().map(|n| n.get()));

    // Stage 1: smoke 32²
    for cand in &candidates {
        let cam = candidate_camera_preset(
            &cand.id,
            cand.r_over_m,
            cand.theta_degrees,
            cand.phi_degrees,
            cand.hfov_degrees,
        );
        let cam_digest = camera_spec_digest(&cam);
        let out_rel = format!("artifacts/gate-2d3a-camera-composition/smoke/{}", cand.id);
        print!("smoke {} ... ", cand.id);
        match render_candidate(
            &root,
            &out_rel,
            &cam,
            spec.smoke_width,
            spec.smoke_height,
            auth_threads,
        ) {
            Ok(rendered) => {
                let metrics = metrics_from_rendered(
                    &rendered,
                    &cam_digest,
                    "CAMERA_DERIVED_PRODUCTION_OUTPUT_NOT_SCIENTIFIC_AUTHORITY",
                );
                let reason = smoke_reject_reason(&metrics, &spec.smoke_hard_invalidity);
                let smoke_valid = reason.is_none();
                println!("{}", if smoke_valid { "ok" } else { "reject" });
                let _ = std::fs::write(
                    root.join(&out_rel).join("composition-metrics.json"),
                    serde_json::to_string_pretty(&metrics)?,
                );
                results.push(CandidateStageResult {
                    candidate: cand.clone(),
                    smoke: Some(metrics),
                    smoke_valid,
                    smoke_reject_reason: reason,
                    gate: None,
                    gate_valid: false,
                    gate_reject_reason: None,
                    shortlist_key: None,
                });
            }
            Err(e) => {
                println!("reject ({e})");
                results.push(CandidateStageResult {
                    candidate: cand.clone(),
                    smoke: None,
                    smoke_valid: false,
                    smoke_reject_reason: Some(format!("render_error:{e}")),
                    gate: None,
                    gate_valid: false,
                    gate_reject_reason: None,
                    shortlist_key: None,
                });
            }
        }
    }

    let smoke_survivors: Vec<SearchCandidate> = results
        .iter()
        .filter(|r| r.smoke_valid)
        .map(|r| r.candidate.clone())
        .collect();
    println!(
        "smoke survivors: {}/{}",
        smoke_survivors.len(),
        candidates.len()
    );

    // Stage 2: gate 128² on smoke survivors only (rule frozen in search spec via smoke filters).
    for cand in &smoke_survivors {
        let idx = results
            .iter()
            .position(|r| r.candidate.id == cand.id)
            .unwrap();
        let cam = candidate_camera_preset(
            &cand.id,
            cand.r_over_m,
            cand.theta_degrees,
            cand.phi_degrees,
            cand.hfov_degrees,
        );
        let cam_digest = camera_spec_digest(&cam);
        let out_rel = format!("artifacts/gate-2d3a-camera-composition/gate/{}", cand.id);
        print!("gate  {} ... ", cand.id);
        match render_candidate(
            &root,
            &out_rel,
            &cam,
            spec.gate_width,
            spec.gate_height,
            auth_threads,
        ) {
            Ok(rendered) => {
                let metrics = metrics_from_rendered(
                    &rendered,
                    &cam_digest,
                    "CAMERA_DERIVED_PRODUCTION_OUTPUT_NOT_SCIENTIFIC_AUTHORITY",
                );
                let reason = gate_reject_reason(&metrics, &spec.gate_hard_invalidity);
                let gate_valid = reason.is_none();
                let key = if gate_valid {
                    Some(shortlist_key(&metrics, &spec.search_guidance, &cand.id))
                } else {
                    None
                };
                println!("{}", if gate_valid { "valid" } else { "reject" });
                let _ = std::fs::write(
                    root.join(&out_rel).join("composition-metrics.json"),
                    serde_json::to_string_pretty(&metrics)?,
                );
                results[idx].gate = Some(metrics);
                results[idx].gate_valid = gate_valid;
                results[idx].gate_reject_reason = reason;
                results[idx].shortlist_key = key;
            }
            Err(e) => {
                println!("reject ({e})");
                results[idx].gate_valid = false;
                results[idx].gate_reject_reason = Some(format!("render_error:{e}"));
            }
        }
    }

    let gate_valid_count = results.iter().filter(|r| r.gate_valid).count();
    let shortlist = select_shortlist(&results, spec.shortlist_n);
    let shortlist_ids: Vec<String> = shortlist.iter().map(|r| r.candidate.id.clone()).collect();

    // Contact sheet evidence: copy shortlist beauties.
    let contact_dir = out_root.join("shortlist-contact");
    std::fs::create_dir_all(&contact_dir)?;
    for (rank, r) in shortlist.iter().enumerate() {
        let src = out_root
            .join("gate")
            .join(&r.candidate.id)
            .join("beauty-scene-srgb16.png");
        let dst = contact_dir.join(format!("{:02}-{}.png", rank + 1, r.candidate.id));
        if src.is_file() {
            std::fs::copy(&src, &dst)?;
        }
        let summary = serde_json::json!({
            "rank": rank + 1,
            "candidate_id": r.candidate.id,
            "family_hint": r.candidate.family_hint,
            "parameters": {
                "r_over_m": r.candidate.r_over_m,
                "theta_degrees": r.candidate.theta_degrees,
                "phi_degrees": r.candidate.phi_degrees,
                "hfov_degrees": r.candidate.hfov_degrees,
            },
            "gate_metrics": r.gate,
            "shortlist_key": r.shortlist_key,
            "authority_label": "CAMERA_DERIVED_PRODUCTION_OUTPUT_NOT_SCIENTIFIC_AUTHORITY",
        });
        std::fs::write(
            contact_dir.join(format!("{:02}-{}.json", rank + 1, r.candidate.id)),
            serde_json::to_string_pretty(&summary)?,
        )?;
    }

    let parameterization_insufficient = gate_valid_count == 0;
    let status = if parameterization_insufficient {
        "CAMERA_PARAMETERIZATION_INSUFFICIENT"
    } else {
        "STOP_FOR_OWNER_HERO_SELECTION"
    };

    let phase = PhaseAReport {
        gate: "gate-2d3a-camera-composition",
        phase: "PHASE_A",
        status,
        stop_reason: "D3A-A7 owner must select hero candidate ID before freeze",
        camera_search_spec_digest: spec_digest.clone(),
        candidate_count: candidates.len(),
        smoke_survivors: smoke_survivors.len(),
        gate_valid_count,
        shortlist_ids: shortlist_ids.clone(),
        baseline_presentation_frame_digest: baseline.report.presentation_frame_digest.clone(),
        baseline_exact,
        parameterization_insufficient,
        note: "Hero not frozen. Class-fraction bands remain SEARCH_GUIDANCE (D3A-A6). Frozen scientific pins proven only on baseline path (D3A-A2).",
    };

    std::fs::write(
        out_root.join("phase-a-report.json"),
        serde_json::to_string_pretty(&phase)?,
    )?;
    std::fs::write(
        out_root.join("candidate-results.json"),
        serde_json::to_string_pretty(&results)?,
    )?;
    std::fs::write(
        out_root.join("camera_search_spec_digest.txt"),
        format!("{spec_digest}\n"),
    )?;
    std::fs::write(
        out_root.join("OWNER_HERO_SELECTION.md"),
        format!(
            r#"# STOP_FOR_OWNER_HERO_SELECTION (D3A-A7)

Phase A complete. Baseline overlay is bit-exact.

## Shortlist (deterministic lex key; not a cinematic score)

{}

## How to select

Reply with one `candidate_id` from the shortlist (or any gate-valid id from `candidate-results.json`).

After selection, Phase B will freeze `presets/camera/gargantua-hero-v1.toml` and run clean authoritative evaluate.

Do **not** expand to free aim / principal-point if dissatisfied — that is `CAMERA_PARAMETERIZATION_INSUFFICIENT` (D3A-A8).
"#,
            shortlist_ids
                .iter()
                .enumerate()
                .map(|(i, id)| format!("{}. `{id}`", i + 1))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )?;

    println!("{}", serde_json::to_string_pretty(&phase)?);
    if parameterization_insufficient {
        return Err("CAMERA_PARAMETERIZATION_INSUFFICIENT".into());
    }
    Err("STOP_FOR_OWNER_HERO_SELECTION".into())
}
