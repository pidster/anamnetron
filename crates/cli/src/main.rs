//! `svt` -- CLI for software-visualizer-tool.

#![warn(missing_docs)]

use clap::Parser;

/// Software Visualizer Tool -- analyze, model, and visualize software architecture.
#[derive(Parser, Debug)]
#[command(name = "svt", version, about)]
struct Cli {
    /// Path to the project to analyze
    #[arg(short, long, default_value = ".")]
    path: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    println!(
        "software-visualizer-tool v{} -- target: {}",
        env!("CARGO_PKG_VERSION"),
        cli.path,
    );
    Ok(())
}
