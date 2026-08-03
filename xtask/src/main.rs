//! Gate 1A orchestration: inspect-point, inspect-initial-ray, evaluate.

use clap::{Parser, Subcommand};

mod evaluate;
mod inspect;
mod preset;

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
    /// Gate evaluator (Gate 1A scope).
    Evaluate {
        #[arg(long)]
        preset: String,
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
        Commands::Evaluate { preset, scope } => evaluate::evaluate(&preset, &scope),
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
