//! Gate 1A orchestration: inspect-point, inspect-initial-ray, evaluate.

use clap::{Parser, Subcommand, ValueEnum};

mod build_meta;
mod corpus_report;
mod evaluate;
mod evaluate_gate1b0;
mod evaluate_gate1b1;
mod evaluate_gate1b2;
mod evaluate_gate2a0;
mod evaluate_gate2a0_parallel;
mod evaluate_gate2a0_trace_shade;
mod inspect;
mod integrate_ray;
mod preset;
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
    /// Gate evaluator (1A / 1B0–1B2 / 2A0-release / 2A0-parallel / 2A0-trace-shade).
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
    /// Trace once and shade many diagnostic styles (Gate 2A0-3).
    TraceShadeMany {
        #[arg(long)]
        preset: String,
        #[arg(long, default_value_t = 128)]
        width: u32,
        #[arg(long, default_value_t = 128)]
        height: u32,
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
    },
    /// Emit canonical Gate 1B1 corpus JSON (numerical records; for determinism).
    CorpusReport {
        #[arg(long)]
        scope: String,
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
            width,
            height,
            output_dir,
            require_release,
            execution,
            threads,
            styles,
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
                width,
                height,
                &output_dir,
                require_release,
                exec,
                threads,
                &styles,
            )
        }
        Commands::CorpusReport { scope } => {
            if scope == "gate-1b1" {
                corpus_report::run()
            } else {
                Err(format!("unsupported corpus-report scope {scope}").into())
            }
        }
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
