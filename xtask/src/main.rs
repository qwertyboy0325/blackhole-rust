//! Gate 1A orchestration: inspect-point, inspect-initial-ray, evaluate.

use clap::{Parser, Subcommand, ValueEnum};

mod build_meta;
mod corpus_report;
mod diagnostic_scene;
mod e1_adaptive_sampling;
mod evaluate;
mod evaluate_e1_adaptive_sampling;
mod evaluate_gate1b0;
mod evaluate_gate1b1;
mod evaluate_gate1b2;
mod evaluate_gate2a0;
mod evaluate_gate2a0_parallel;
mod evaluate_gate2a0_preview_tiers;
mod evaluate_gate2a0_trace_shade;
mod evaluate_gate2a1_celestial;
mod evaluate_gate2a2_lensed_celestial;
mod evaluate_gate2b0_frequency_shift;
mod evaluate_gate2b1_bolometric_radiance;
mod evaluate_gate2b2_spectral_transport;
mod evaluate_r1_e0_oracle_corpus;
mod inspect;
mod integrate_ray;
mod oracle_benchmark;
mod preset;
mod reference_pipeline;
mod render_disk_spectrum;
mod render_lensed_celestial;
mod render_tier;
mod spike_dop853;
mod trace_outcome_map;
mod trace_shade_many;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ExecutionArg {
    Serial,
    Parallel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ShadeStyleArg {
    #[value(name = "gate1b2-categorical")]
    Gate1b2Categorical,
    #[value(name = "disk-suppressed")]
    DiskSuppressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SurfaceSetArg {
    #[value(name = "opaque-disk-horizon-escape")]
    OpaqueDiskHorizonEscape,
    #[value(name = "horizon-escape-only")]
    HorizonEscapeOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LensedModeArg {
    #[value(name = "opaque-disk-mask")]
    OpaqueDiskMask,
    #[value(name = "disk-omitted-diagnostic")]
    DiskOmittedDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TextureArg {
    #[value(name = "procedural-coordinate-grid-v1")]
    ProceduralCoordinateGridV1,
}

#[derive(Parser)]
#[command(name = "xtask", about = "blackhole-rust task runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deterministic geometry diagnostics at a Cartesian KS point.
    InspectPoint {
        #[arg(long)]
        mass: f64,
        #[arg(long)]
        spin: f64,
        #[arg(long)]
        x: f64,
        #[arg(long)]
        y: f64,
        #[arg(long)]
        z: f64,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Deterministic initial-ray diagnostics for a preset sensor sample.
    InspectInitialRay {
        #[arg(long)]
        preset: String,
        #[arg(long)]
        sensor_x: f64,
        #[arg(long)]
        sensor_y: f64,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Gate evaluator (1A / 1B0–1B2 / 2A0 scopes).
    Evaluate {
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        scope: String,
    },
    /// Run DOP853 spike experiments for one candidate.
    SpikeDop853 {
        #[arg(long)]
        candidate: String,
    },
    /// Integrate one camera ray; emit JSON diagnostics (no image).
    IntegrateRay {
        #[arg(long)]
        preset: String,
        #[arg(long)]
        sensor_x: f64,
        #[arg(long)]
        sensor_y: f64,
        #[arg(long, default_value_t = 100.0)]
        affine_limit: f64,
    },
    /// Gate 1B2 categorical outcome map (PPM) + cost PGM + JSON.
    TraceOutcomeMap {
        #[arg(long)]
        preset: String,
        #[arg(long, default_value_t = 128)]
        width: u32,
        #[arg(long, default_value_t = 128)]
        height: u32,
        #[arg(long)]
        output: String,
        /// Reject non-release builds before any tracing or artifact writes.
        #[arg(long, default_value_t = false)]
        require_release: bool,
        /// Camera-grid execution mode (default serial).
        #[arg(long, value_enum, default_value_t = ExecutionArg::Serial)]
        execution: ExecutionArg,
        /// Required for `--execution parallel`; rejected with serial.
        #[arg(long)]
        threads: Option<usize>,
    },
    /// Trace once and shade many diagnostic styles (Gate 2A0-3 / 2A0-4).
    TraceShadeMany {
        #[arg(long)]
        preset: String,
        /// Named diagnostic tier (`smoke`/`preview`/`gate`/`showcase`). Mutually exclusive with width/height.
        #[arg(long, value_enum)]
        tier: Option<render_tier::DiagnosticRenderTier>,
        /// Custom width (legacy default axis 128 if omitted with --height). Rejected with --tier.
        #[arg(long)]
        width: Option<u32>,
        /// Custom height (legacy default axis 128 if omitted with --width). Rejected with --tier.
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        output_dir: String,
        #[arg(long, default_value_t = false)]
        require_release: bool,
        #[arg(long, value_enum, default_value_t = ExecutionArg::Serial)]
        execution: ExecutionArg,
        #[arg(long)]
        threads: Option<usize>,
        /// Repeatable; order preserved; duplicates rejected.
        #[arg(long = "style", value_enum)]
        styles: Vec<ShadeStyleArg>,
        /// Derive finite celestial-boundary UV coordinates from the same TraceBundle (Gate 2A1).
        #[arg(long, default_value_t = false)]
        emit_celestial_coordinates: bool,
    },
    /// Trace once → Gate 2A1 coordinates → procedural celestial → lensed diagnostic PPM (Gate 2A2).
    RenderLensedCelestial {
        #[arg(long)]
        preset: String,
        #[arg(long, value_enum)]
        tier: Option<render_tier::DiagnosticRenderTier>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, value_enum)]
        surface_set: SurfaceSetArg,
        #[arg(long, value_enum)]
        mode: LensedModeArg,
        #[arg(long, value_enum)]
        texture: TextureArg,
        #[arg(long)]
        output_dir: String,
        #[arg(long, value_enum, default_value_t = ExecutionArg::Serial)]
        execution: ExecutionArg,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long, default_value_t = false)]
        require_release: bool,
        /// Emit disk-hit frequency-shift kinematics artifacts (Gate 2B0).
        /// Requires opaque-disk-horizon-escape + opaque-disk-mask.
        #[arg(long, default_value_t = false)]
        emit_disk_frequency_shift: bool,
        /// Emit diagnostic bolometric disk radiance + g⁴ transport (Gate 2B1).
        /// Requires --emit-disk-frequency-shift and opaque-disk mode/surface.
        #[arg(long, default_value_t = false)]
        emit_disk_bolometric_radiance: bool,
    },
    /// Gate 2B2: diagnostic disk spectral I_ν transport (g³).
    RenderDiskSpectrum {
        #[arg(long)]
        preset: String,
        #[arg(long, value_enum)]
        tier: Option<render_tier::DiagnosticRenderTier>,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, default_value = "diagnostic-lognormal-continuum-v1")]
        spectrum: String,
        #[arg(long, default_value = "spectral-grid-v1")]
        spectral_grid: String,
        #[arg(long)]
        output_dir: String,
        #[arg(long, value_enum, default_value_t = ExecutionArg::Serial)]
        execution: ExecutionArg,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long, default_value_t = false)]
        require_release: bool,
    },
    /// Emit canonical Gate 1B1 corpus JSON (numerical records; for determinism).
    CorpusReport {
        #[arg(long)]
        scope: String,
    },
    /// Generate the R1/E0 oracle benchmark corpus candidate lock and artifacts.
    OracleBenchmarkCorpus {
        #[arg(long)]
        manifest: String,
        #[arg(long)]
        output_dir: String,
        #[arg(long, value_enum)]
        execution: ExecutionArg,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long, default_value_t = false)]
        require_release: bool,
        /// When set, do not overwrite experiments/oracle-benchmark/corpus-lock-v1.json.
        #[arg(long, default_value_t = false)]
        skip_committed_lock_update: bool,
    },
    /// E1 physics-aware adaptive quadtree sampling experiment.
    AdaptiveSamplingExperiment {
        #[arg(long)]
        config: String,
        #[arg(long)]
        output_dir: String,
        #[arg(long, value_enum)]
        execution: ExecutionArg,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long, default_value_t = false)]
        require_release: bool,
        #[arg(long)]
        /// Comma-separated case ids to run (default: all).
        case: Option<String>,
        #[arg(long)]
        method: Option<String>,
        #[arg(long)]
        maximum_budget_level: Option<usize>,
        #[arg(long, default_value_t = false)]
        skip_ablations: bool,
        /// Reuse a verified E0 reference directory (skips in-process corpus regen).
        #[arg(long)]
        reference_dir: Option<String>,
        /// progressive (default) or cold budget-from-zero ladders.
        #[arg(long, default_value = "progressive")]
        ladder: String,
        /// full (writes reconstruction.ppm) or minimal (determinism evidence bundle only).
        #[arg(long, default_value = "full")]
        write_artifacts: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::InspectPoint {
            mass,
            spin,
            x,
            y,
            z,
            format,
        } => inspect::inspect_point(mass, spin, x, y, z, &format),
        Commands::InspectInitialRay {
            preset,
            sensor_x,
            sensor_y,
            format,
        } => inspect::inspect_initial_ray(&preset, sensor_x, sensor_y, &format),
        Commands::Evaluate { preset, scope } => {
            if scope == "gate-1b0" {
                evaluate_gate1b0::evaluate()
            } else if scope == "gate-1b1" {
                evaluate_gate1b1::evaluate()
            } else if scope == "gate-1b2" {
                evaluate_gate1b2::evaluate()
            } else if scope == "gate-2a0-release" {
                evaluate_gate2a0::evaluate()
            } else if scope == "gate-2a0-parallel" {
                evaluate_gate2a0_parallel::evaluate()
            } else if scope == "gate-2a0-trace-shade" {
                evaluate_gate2a0_trace_shade::evaluate()
            } else if scope == "gate-2a0-preview-tiers" {
                evaluate_gate2a0_preview_tiers::evaluate()
            } else if scope == "gate-2a1-celestial-directions" {
                evaluate_gate2a1_celestial::evaluate()
            } else if scope == "gate-2a2-lensed-celestial" {
                evaluate_gate2a2_lensed_celestial::evaluate()
            } else if scope == "gate-2b0-frequency-shift" {
                evaluate_gate2b0_frequency_shift::evaluate()
            } else if scope == "gate-2b1-bolometric-radiance" {
                evaluate_gate2b1_bolometric_radiance::evaluate()
            } else if scope == "gate-2b2-spectral-transport" {
                evaluate_gate2b2_spectral_transport::evaluate()
            } else if scope == "r1-e0-oracle-corpus" {
                evaluate_r1_e0_oracle_corpus::evaluate()
            } else if scope == "e1-adaptive-sampling" {
                evaluate_e1_adaptive_sampling::evaluate()
            } else {
                match preset {
                    Some(p) => evaluate::evaluate(&p, &scope),
                    None => Err("gate-1a evaluate requires --preset".into()),
                }
            }
        }
        Commands::IntegrateRay {
            preset,
            sensor_x,
            sensor_y,
            affine_limit,
        } => integrate_ray::run(&preset, sensor_x, sensor_y, affine_limit),
        Commands::TraceOutcomeMap {
            preset,
            width,
            height,
            output,
            require_release,
            execution,
            threads,
        } => {
            let exec = match execution {
                ExecutionArg::Serial => trace_outcome_map::CliExecution::Serial,
                ExecutionArg::Parallel => trace_outcome_map::CliExecution::Parallel,
            };
            trace_outcome_map::run(
                &preset,
                width,
                height,
                &output,
                require_release,
                exec,
                threads,
            )
        }
        Commands::TraceShadeMany {
            preset,
            tier,
            width,
            height,
            output_dir,
            require_release,
            execution,
            threads,
            styles,
            emit_celestial_coordinates,
        } => {
            let exec = match execution {
                ExecutionArg::Serial => trace_outcome_map::CliExecution::Serial,
                ExecutionArg::Parallel => trace_outcome_map::CliExecution::Parallel,
            };
            let styles: Vec<_> = styles
                .into_iter()
                .map(|s| match s {
                    ShadeStyleArg::Gate1b2Categorical => {
                        relativity_trace::DiagnosticShadeStyle::Gate1b2Categorical
                    }
                    ShadeStyleArg::DiskSuppressed => {
                        relativity_trace::DiagnosticShadeStyle::DiskSuppressed
                    }
                })
                .collect();
            trace_shade_many::run(
                &preset,
                tier,
                width,
                height,
                &output_dir,
                require_release,
                exec,
                threads,
                &styles,
                emit_celestial_coordinates,
            )
        }
        Commands::RenderLensedCelestial {
            preset,
            tier,
            width,
            height,
            surface_set,
            mode,
            texture,
            output_dir,
            execution,
            threads,
            require_release,
            emit_disk_frequency_shift,
            emit_disk_bolometric_radiance,
        } => {
            let exec = match execution {
                ExecutionArg::Serial => trace_outcome_map::CliExecution::Serial,
                ExecutionArg::Parallel => trace_outcome_map::CliExecution::Parallel,
            };
            let surface_set = match surface_set {
                SurfaceSetArg::OpaqueDiskHorizonEscape => {
                    relativity_trace::TraceSurfaceSet::OpaqueDiskHorizonEscape
                }
                SurfaceSetArg::HorizonEscapeOnly => {
                    relativity_trace::TraceSurfaceSet::HorizonEscapeOnly
                }
            };
            let mode = match mode {
                LensedModeArg::OpaqueDiskMask => {
                    relativity_render::LensedCelestialMode::OpaqueDiskMask
                }
                LensedModeArg::DiskOmittedDiagnostic => {
                    relativity_render::LensedCelestialMode::DiskOmittedDiagnostic
                }
            };
            let texture = match texture {
                TextureArg::ProceduralCoordinateGridV1 => relativity_render::TEXTURE_ID_V1,
            };
            render_lensed_celestial::run(
                &preset,
                tier,
                width,
                height,
                surface_set,
                mode,
                texture,
                &output_dir,
                require_release,
                exec,
                threads,
                emit_disk_frequency_shift,
                emit_disk_bolometric_radiance,
            )
        }
        Commands::RenderDiskSpectrum {
            preset,
            tier,
            width,
            height,
            spectrum,
            spectral_grid,
            output_dir,
            execution,
            threads,
            require_release,
        } => {
            let exec = match execution {
                ExecutionArg::Serial => trace_outcome_map::CliExecution::Serial,
                ExecutionArg::Parallel => trace_outcome_map::CliExecution::Parallel,
            };
            render_disk_spectrum::run(
                &preset,
                tier,
                width,
                height,
                &spectrum,
                &spectral_grid,
                &output_dir,
                require_release,
                exec,
                threads,
            )
        }
        Commands::CorpusReport { scope } => {
            if scope == "gate-1b1" {
                corpus_report::run()
            } else {
                Err(format!("unsupported corpus-report scope {scope}").into())
            }
        }
        Commands::OracleBenchmarkCorpus {
            manifest,
            output_dir,
            execution,
            threads,
            require_release,
            skip_committed_lock_update,
        } => oracle_benchmark::run(
            &manifest,
            &output_dir,
            match execution {
                ExecutionArg::Serial => trace_outcome_map::CliExecution::Serial,
                ExecutionArg::Parallel => trace_outcome_map::CliExecution::Parallel,
            },
            threads,
            require_release,
            !skip_committed_lock_update,
        ),
        Commands::AdaptiveSamplingExperiment {
            config,
            output_dir,
            execution,
            threads,
            require_release,
            case,
            method,
            maximum_budget_level,
            skip_ablations,
            reference_dir,
            ladder,
            write_artifacts,
        } => match method
            .as_deref()
            .map(|m| {
                e1_adaptive_sampling::MethodId::parse(m)
                    .ok_or_else(|| format!("unknown method {m}").into())
            })
            .transpose()
        {
            Ok(method) => {
                let ladder = match ladder.as_str() {
                    "progressive" => Ok(e1_adaptive_sampling::LadderMode::Progressive),
                    "cold" => Ok(e1_adaptive_sampling::LadderMode::Cold),
                    other => Err(format!("unknown ladder mode {other}").into()),
                };
                let write_artifacts = match write_artifacts.as_str() {
                    "full" => Ok(e1_adaptive_sampling::WriteArtifacts::Full),
                    "minimal" => Ok(e1_adaptive_sampling::WriteArtifacts::Minimal),
                    other => Err(format!("unknown write_artifacts {other}").into()),
                };
                match (ladder, write_artifacts) {
                    (Ok(ladder), Ok(write_artifacts)) => e1_adaptive_sampling::run(
                        &config,
                        &output_dir,
                        match execution {
                            ExecutionArg::Serial => trace_outcome_map::CliExecution::Serial,
                            ExecutionArg::Parallel => trace_outcome_map::CliExecution::Parallel,
                        },
                        threads,
                        require_release,
                        e1_adaptive_sampling::ExperimentFilters {
                            case,
                            method,
                            maximum_budget_level,
                            skip_ablations,
                        },
                        e1_adaptive_sampling::ExperimentOptions {
                            reference_dir: reference_dir.map(std::path::PathBuf::from),
                            ladder,
                            write_artifacts,
                        },
                    ),
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            Err(e) => Err(e),
        },
        Commands::SpikeDop853 { candidate } => {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .to_path_buf();
            let commit = std::process::Command::new("git")
                .current_dir(&root)
                .args(["rev-parse", "HEAD"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "unknown".into());
            let toolchain = std::process::Command::new("rustc")
                .arg("--version")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "unknown".into());
            let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".into());
            match spike_dop853::run(&candidate, &commit, &toolchain, &target) {
                Ok(report) => spike_dop853::write_report(&report, &root.join("artifacts/gate-1b0")),
                Err(e) => Err(e),
            }
        }
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
