//! Gate 1A orchestration: inspect-point, inspect-initial-ray, evaluate.

use clap::{Parser, Subcommand};

mod corpus_report;
mod evaluate;
mod evaluate_gate1b0;
mod evaluate_gate1b1;
mod inspect;
mod integrate_ray;
mod preset;
mod spike_dop853;

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
    /// Gate evaluator (Gate 1A / Gate 1B0 / Gate 1B1 scope).
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
