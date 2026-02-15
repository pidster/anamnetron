//! `svt` -- CLI for software-visualizer-tool.

#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::store::{CozoStore, GraphStore};

/// Software Visualizer Tool -- analyze, model, and visualize software architecture.
#[derive(Parser, Debug)]
#[command(name = "svt", version, about)]
struct Cli {
    /// Store location (default: .svt/store)
    #[arg(long, default_value = ".svt/store")]
    store: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Import a design YAML/JSON file into the store.
    Import(ImportArgs),
    /// Run conformance checks on the current design.
    Check(CheckArgs),
}

#[derive(clap::Args, Debug)]
struct ImportArgs {
    /// Path to the YAML or JSON file to import.
    file: PathBuf,
}

#[derive(clap::Args, Debug)]
struct CheckArgs {
    /// Design version to check (default: latest).
    #[arg(long)]
    design: Option<u64>,

    /// Minimum severity to cause a non-zero exit code.
    #[arg(long, default_value = "error")]
    fail_on: String,

    /// Output format: human or json.
    #[arg(long, default_value = "human")]
    format: String,
}

fn open_or_create_store(path: &Path) -> Result<CozoStore> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating store directory {}", parent.display()))?;
    }
    CozoStore::new_persistent(path).map_err(|e| anyhow::anyhow!("{}", e))
}

fn open_store(path: &Path) -> Result<CozoStore> {
    if !path.exists() {
        bail!(
            "Store not found at {}. Run `svt import` first.",
            path.display()
        );
    }
    CozoStore::new_persistent(path).map_err(|e| anyhow::anyhow!("{}", e))
}

fn run_import(store_path: &Path, args: &ImportArgs) -> Result<()> {
    let content = std::fs::read_to_string(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;

    let ext = args
        .file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let doc = match ext {
        "yaml" | "yml" => interchange::parse_yaml(&content)
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        "json" => interchange::parse_json(&content)
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        _ => bail!("Unsupported file format: .{ext}. Use .yaml, .yml, or .json"),
    };

    // Validate
    let warnings = interchange::validate_document(&doc)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    for w in &warnings {
        eprintln!("  WARN  {}: {}", w.path, w.message);
    }

    let mut store = open_or_create_store(store_path)?;
    let version = interchange_store::load_into_store(&mut store, &doc)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let node_count = store
        .get_all_nodes(version)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();
    let edge_count = store
        .get_all_edges(version, None)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();
    let constraint_count = store
        .get_constraints(version)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();

    println!("Imported {} as version {}", args.file.display(), version);
    println!(
        "  {} nodes, {} edges, {} constraints",
        node_count, edge_count, constraint_count
    );

    Ok(())
}

fn run_check(store_path: &Path, args: &CheckArgs) -> Result<()> {
    use svt_core::conformance::{self, ConstraintStatus};
    use svt_core::model::{Severity, SnapshotKind};

    let store = open_store(store_path)?;

    let version = match args.design {
        Some(v) => v,
        None => store
            .latest_version(SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No design versions found in store"))?,
    };

    let report = conformance::evaluate_design(&store, version)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if args.format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        );
    } else {
        print_human_report(&report);
    }

    // Determine exit code based on fail_on severity
    let fail_severity = match args.fail_on.as_str() {
        "warning" => Some(Severity::Warning),
        "info" => Some(Severity::Info),
        _ => Some(Severity::Error),
    };

    let has_failures = report.constraint_results.iter().any(|r| {
        r.status == ConstraintStatus::Fail
            && fail_severity
                .map(|s| severity_at_or_above(r.severity, s))
                .unwrap_or(false)
    });

    if has_failures {
        std::process::exit(1);
    }

    Ok(())
}

fn severity_at_or_above(actual: svt_core::model::Severity, threshold: svt_core::model::Severity) -> bool {
    severity_rank(actual) >= severity_rank(threshold)
}

fn severity_rank(s: svt_core::model::Severity) -> u8 {
    match s {
        svt_core::model::Severity::Info => 0,
        svt_core::model::Severity::Warning => 1,
        svt_core::model::Severity::Error => 2,
    }
}

fn print_human_report(report: &svt_core::conformance::ConformanceReport) {
    use svt_core::conformance::ConstraintStatus;

    println!("Checking design v{}...\n", report.design_version);

    for result in &report.constraint_results {
        let tag = match result.status {
            ConstraintStatus::Pass => "  PASS ",
            ConstraintStatus::Fail => "  FAIL ",
            ConstraintStatus::NotEvaluable => "  N/A  ",
        };
        println!("{} {}: {}", tag, result.constraint_name, result.message);

        for v in &result.violations {
            let target = v
                .target_path
                .as_deref()
                .map(|t| format!(" -> {}", t))
                .unwrap_or_default();
            println!("         {} {}{}", "-", v.source_path, target);
        }
    }

    println!();
    println!(
        "  {} passed, {} failed, {} warnings, {} not evaluable",
        report.summary.passed,
        report.summary.failed,
        report.summary.warned,
        report.summary.not_evaluable,
    );
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Import(args) => run_import(&cli.store, args),
        Commands::Check(args) => run_check(&cli.store, args),
    }
}
