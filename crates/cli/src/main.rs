//! `svt` -- CLI for Anamnetron.

#![warn(missing_docs)]

pub(crate) mod manifest;
pub(crate) mod plugin;
pub(crate) mod plugin_commands;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::store::{CozoStore, GraphStore};

/// Anamnetron -- analyze, model, and visualize software architecture.
#[derive(Parser, Debug)]
#[command(name = "svt", version, about)]
struct Cli {
    /// Project directory containing .svt/config.yaml (default: current directory).
    #[arg(long, default_value = ".", global = true)]
    project_dir: PathBuf,

    /// Directory containing plugin libraries (default: adjacent to svt binary).
    /// Also settable via SVT_PLUGIN_DIR environment variable.
    #[arg(long, global = true, env = "SVT_PLUGIN_DIR")]
    plugin_dir: Option<PathBuf>,

    /// Load additional plugin(s) from shared library paths.
    #[arg(long = "plugin", global = true)]
    plugins: Vec<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Import a design YAML/JSON file into the store.
    Import(ImportArgs),
    /// Run conformance checks on the current design.
    Check(CheckArgs),
    /// Analyze a Rust project and create an analysis snapshot.
    Analyze(AnalyzeArgs),
    /// Export graph as Mermaid, JSON, DOT, SVG, or PNG.
    Export(ExportArgs),
    /// Compare two snapshot versions and show what changed.
    Diff(DiffArgs),
    /// Manage and list loaded plugins.
    Plugin(PluginArgs),
    /// Push analysis data to a remote server.
    Push(PushArgs),
    /// Manage the graph store (info, compact, reset).
    Store(StoreArgs),
    /// Initialize a new .svt project directory.
    Init(InitArgs),
}

/// Resolved project configuration, combining config file and CLI overrides.
#[allow(dead_code)] // Fields used by upcoming tasks (import merge, analyze sources)
struct ResolvedConfig {
    /// The project directory root.
    project_dir: PathBuf,
    /// Path to the CozoDB store.
    store_path: PathBuf,
    /// Project ID for multi-tenancy.
    project_id: String,
    /// Loaded project config (if .svt/config.yaml exists).
    config: Option<svt_core::config::ProjectConfig>,
}

impl ResolvedConfig {
    /// Resolve config from CLI flags and optional config file.
    fn resolve(project_dir: &Path) -> Result<Self> {
        let project_dir = project_dir
            .canonicalize()
            .unwrap_or_else(|_| project_dir.to_path_buf());
        let store_path = project_dir.join(".svt").join("data").join("store");

        let config = svt_core::config::ProjectConfig::load_from_project_dir(&project_dir)
            .map_err(|e| anyhow::anyhow!("loading config: {e}"))?;

        let project_id = config
            .as_ref()
            .map(|c| c.project.clone())
            .unwrap_or_else(|| "default".to_string());

        Ok(Self {
            project_dir,
            store_path,
            project_id,
            config,
        })
    }
}

#[derive(clap::Args, Debug)]
struct ImportArgs {
    /// Import a specific file (overrides design files from config).
    #[arg(long)]
    file: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct CheckArgs {
    /// Design version to check (default: latest).
    #[arg(long)]
    design: Option<u64>,

    /// Analysis version to compare against (enables design vs analysis comparison).
    /// Use `--analysis` alone for the latest analysis, or `--analysis <N>` for a specific version.
    #[arg(long, num_args = 0..=1, default_missing_value = "0")]
    analysis: Option<u64>,

    /// Minimum severity to cause a non-zero exit code.
    #[arg(long, default_value = "error")]
    fail_on: String,

    /// Output format: human or json.
    #[arg(long, default_value = "human")]
    format: String,
}

#[derive(clap::Args, Debug)]
struct AnalyzeArgs {
    /// Path to analyze (overrides config sources). If omitted, reads from config or defaults to ".".
    path: Option<PathBuf>,

    /// Optional git commit ref to tag the snapshot.
    #[arg(long)]
    commit_ref: Option<String>,

    /// Enable incremental analysis: only re-analyze units with changed files.
    #[arg(long)]
    incremental: bool,
}

#[derive(clap::Args, Debug)]
struct ExportArgs {
    /// Output format: mermaid, json, dot, svg, or png.
    #[arg(long)]
    format: String,

    /// Snapshot version to export (default: latest design).
    #[arg(long)]
    version: Option<u64>,

    /// Output file path (default: stdout).
    #[arg(long, short)]
    output: Option<PathBuf>,
}

#[derive(clap::Args, Debug)]
struct DiffArgs {
    /// First snapshot version (base).
    #[arg(long)]
    from: u64,

    /// Second snapshot version (target).
    #[arg(long)]
    to: u64,

    /// Output format: human or json.
    #[arg(long, default_value = "human")]
    format: String,
}

#[derive(clap::Args, Debug)]
struct PushArgs {
    /// Remote server URL (overrides config server.url).
    #[arg(long)]
    server: Option<String>,
    /// Snapshot version to push (default: latest of the selected kind).
    #[arg(long)]
    version: Option<u64>,
    /// What to push: design, analysis, or all (default: analysis).
    #[arg(long, default_value = "analysis")]
    kind: String,
}

/// Arguments for the `svt plugin` subcommand.
#[derive(clap::Args, Debug)]
struct PluginArgs {
    #[command(subcommand)]
    command: PluginCommands,
}

/// Subcommands under `svt plugin`.
#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// List all loaded plugins and their contributions.
    List,
    /// Install a plugin from a local directory or manifest.
    Install(PluginInstallArgs),
    /// Remove an installed plugin by name.
    Remove(PluginRemoveArgs),
    /// Show information about a plugin from its manifest.
    Info(PluginInfoArgs),
}

/// Arguments for `svt plugin install`.
#[derive(clap::Args, Debug)]
struct PluginInstallArgs {
    /// Directory containing svt-plugin.toml + compiled library, or path to a manifest file.
    source: PathBuf,
    /// Overwrite existing plugin with the same name.
    #[arg(long)]
    force: bool,
}

/// Arguments for `svt plugin remove`.
#[derive(clap::Args, Debug)]
struct PluginRemoveArgs {
    /// Plugin name to remove (matches <name>.svt-plugin.toml).
    name: String,
}

/// Arguments for `svt plugin info`.
#[derive(clap::Args, Debug)]
struct PluginInfoArgs {
    /// Directory containing svt-plugin.toml, or path to a .toml manifest file.
    path: PathBuf,
}

/// Arguments for `svt init`.
#[derive(clap::Args, Debug)]
struct InitArgs {
    /// Project name (default: derived from git remote or directory name).
    #[arg(long)]
    project: Option<String>,
}

/// Arguments for the `svt store` subcommand.
#[derive(clap::Args, Debug)]
struct StoreArgs {
    #[command(subcommand)]
    command: StoreCommands,
}

/// Subcommands under `svt store`.
#[derive(Subcommand, Debug)]
enum StoreCommands {
    /// Show store information: schema version, snapshots, node/edge counts.
    Info,
    /// Remove old snapshot versions to reclaim space.
    Compact(CompactArgs),
    /// Delete the store and recreate it empty.
    Reset(ResetArgs),
}

/// Arguments for `svt store compact`.
#[derive(clap::Args, Debug)]
struct CompactArgs {
    /// Explicit list of version numbers to keep.
    /// Default: keep the latest design and latest analysis snapshots.
    #[arg(long)]
    keep: Vec<u64>,
}

/// Arguments for `svt store reset`.
#[derive(clap::Args, Debug)]
struct ResetArgs {
    /// Skip the confirmation prompt.
    #[arg(long)]
    force: bool,
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

/// Ensure a project exists in the store, creating it if necessary.
fn ensure_project(store: &mut CozoStore, project_id: &str) -> Result<()> {
    use svt_core::model::{validate_project_id, Project};
    validate_project_id(project_id).map_err(|e| anyhow::anyhow!("{}", e))?;
    if !store
        .project_exists(project_id)
        .map_err(|e| anyhow::anyhow!("{}", e))?
    {
        store
            .create_project(&Project {
                id: project_id.to_string(),
                name: project_id.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                description: None,
                metadata: None,
            })
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    Ok(())
}

/// Attempt to derive a project name from the git remote origin URL,
/// falling back to the current directory name.
fn derive_project_name() -> Option<String> {
    // Try git remote origin URL
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if output.status.success() {
        let url = String::from_utf8(output.stdout).ok()?.trim().to_string();
        // Extract repo name from SSH (git@...:user/repo.git) or HTTPS (https://...user/repo.git)
        let name = url
            .rsplit('/')
            .next()
            .or_else(|| url.rsplit(':').next())?
            .trim_end_matches(".git")
            .to_string();
        if !name.is_empty() {
            return Some(name.to_lowercase().replace(' ', "-"));
        }
    }
    // Fallback: current directory name
    std::env::current_dir()
        .ok()?
        .file_name()?
        .to_str()
        .map(|s| s.to_lowercase().replace(' ', "-"))
}

/// Parse a single design file into an interchange document.
fn parse_design_file(file: &Path) -> Result<interchange::InterchangeDocument> {
    let content =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;

    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");

    let doc = match ext {
        "yaml" | "yml" => {
            interchange::parse_yaml(&content).map_err(|e| anyhow::anyhow!("{}", e))?
        }
        "json" => interchange::parse_json(&content).map_err(|e| anyhow::anyhow!("{}", e))?,
        _ => bail!("Unsupported file format: .{ext}. Use .yaml, .yml, or .json"),
    };

    let warnings = interchange::validate_document(&doc).map_err(|e| anyhow::anyhow!("{}", e))?;
    for w in &warnings {
        eprintln!("  WARN  {}: {}", w.path, w.message);
    }

    Ok(doc)
}

fn run_import(
    store_path: &Path,
    project_id: &str,
    args: &ImportArgs,
    resolved: &ResolvedConfig,
) -> Result<()> {
    // Determine which files to import
    let files: Vec<PathBuf> = if let Some(ref file) = args.file {
        vec![file.clone()]
    } else if let Some(ref config) = resolved.config {
        if config.design.is_empty() {
            bail!("No design files specified. Use --file or add design files to .svt/config.yaml");
        }
        // Resolve paths relative to project dir
        config
            .design
            .iter()
            .map(|p| resolved.project_dir.join(p))
            .collect()
    } else {
        bail!("No design files specified. Use --file or create .svt/config.yaml with design files");
    };

    // Validate all files exist before importing
    for file in &files {
        if !file.exists() {
            bail!("Design file not found: {}", file.display());
        }
    }

    // Parse all files
    let mut merged_doc = parse_design_file(&files[0])?;
    for file in &files[1..] {
        let doc = parse_design_file(file)?;
        merged_doc.nodes.extend(doc.nodes);
        merged_doc.edges.extend(doc.edges);
        merged_doc.constraints.extend(doc.constraints);
    }

    let mut store = open_or_create_store(store_path)?;
    ensure_project(&mut store, project_id)?;
    let version = interchange_store::load_into_store(&mut store, project_id, &merged_doc)
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

    if files.len() == 1 {
        println!("Imported {} as version {}", files[0].display(), version);
    } else {
        println!(
            "Imported {} design files as version {}",
            files.len(),
            version
        );
    }
    println!(
        "  {} nodes, {} edges, {} constraints",
        node_count, edge_count, constraint_count
    );

    Ok(())
}

fn run_check(
    store_path: &Path,
    project_id: &str,
    args: &CheckArgs,
    loader: &plugin::PluginLoader,
) -> Result<()> {
    use svt_core::conformance::{self, ConstraintRegistry, ConstraintStatus};
    use svt_core::model::{Severity, SnapshotKind};

    let store = open_store(store_path)?;
    let mut registry = ConstraintRegistry::with_defaults();
    loader.register_constraints(&mut registry);

    let design_version = match args.design {
        Some(v) => v,
        None => store
            .latest_version(project_id, SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No design versions found in store"))?,
    };

    let report = if let Some(analysis_version_arg) = args.analysis {
        let analysis_version = if analysis_version_arg == 0 {
            // 0 means "use latest analysis version"
            store
                .latest_version(project_id, SnapshotKind::Analysis)
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("No analysis versions found in store"))?
        } else {
            analysis_version_arg
        };
        conformance::evaluate(&store, design_version, analysis_version, &registry)
            .map_err(|e| anyhow::anyhow!("{}", e))?
    } else {
        conformance::evaluate_design(&store, design_version, &registry)
            .map_err(|e| anyhow::anyhow!("{}", e))?
    };

    if args.format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| anyhow::anyhow!("{}", e))?
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

fn severity_at_or_above(
    actual: svt_core::model::Severity,
    threshold: svt_core::model::Severity,
) -> bool {
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

    if let Some(av) = report.analysis_version {
        println!(
            "Comparing design v{} vs analysis v{}...\n",
            report.design_version, av
        );
    } else {
        println!("Checking design v{}...\n", report.design_version);
    }

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
            println!("         - {}{}", v.source_path, target);
        }
    }

    if !report.unimplemented.is_empty() {
        println!(
            "\n  Unimplemented design nodes ({}):",
            report.unimplemented.len()
        );
        for node in &report.unimplemented {
            println!("    - {} ({:?})", node.canonical_path, node.kind);
        }
    }

    if !report.undocumented.is_empty() {
        println!(
            "\n  Undocumented analysis nodes ({}):",
            report.undocumented.len()
        );
        for node in &report.undocumented {
            println!("    - {} ({:?})", node.canonical_path, node.kind);
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

/// Try to detect the current git HEAD commit in the given directory.
fn detect_git_head(project_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8(o.stdout)
                .ok()
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
}

fn run_analyze(
    store_path: &Path,
    project_id: &str,
    args: &AnalyzeArgs,
    loader: &plugin::PluginLoader,
    resolved: &ResolvedConfig,
) -> Result<()> {
    // Resolve analyze path: CLI arg > config sources > default "."
    let analyze_path = if let Some(ref path) = args.path {
        path.clone()
    } else if let Some(ref config) = resolved.config {
        if let Some(source) = config.sources.first() {
            resolved.project_dir.join(&source.path)
        } else {
            resolved.project_dir.clone()
        }
    } else {
        PathBuf::from(".")
    };

    let mut store = open_or_create_store(store_path)?;
    ensure_project(&mut store, project_id)?;

    let commit_ref = args
        .commit_ref
        .clone()
        .or_else(|| detect_git_head(&analyze_path));

    let mut registry = svt_analyzer::orchestrator::OrchestratorRegistry::with_defaults();
    loader.register_language_parsers(&mut registry);

    let summary = if args.incremental {
        use svt_core::model::SnapshotKind;
        let previous = store
            .latest_version(project_id, SnapshotKind::Analysis)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        svt_analyzer::analyze_project_incremental_with_registry(
            &mut store,
            project_id,
            &analyze_path,
            commit_ref.as_deref(),
            previous,
            registry,
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?
    } else {
        svt_analyzer::analyze_project_with_registry(
            &mut store,
            project_id,
            &analyze_path,
            commit_ref.as_deref(),
            registry,
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?
    };

    println!("Analyzed {}\n", analyze_path.display());
    println!("  Created analysis snapshot v{}", summary.version);
    if let Some(ref cr) = commit_ref {
        println!("    commit: {}", cr);
    }
    println!(
        "    {} crates, {} TS packages, {} Go modules, {} Python packages, {} files analyzed",
        summary.crates_analyzed,
        summary.ts_packages_analyzed,
        summary.go_packages_analyzed,
        summary.python_packages_analyzed,
        summary.files_analyzed
    );
    println!(
        "    {} nodes, {} edges",
        summary.nodes_created, summary.edges_created
    );

    if summary.incremental {
        println!(
            "    incremental: {} units skipped, {} re-analyzed, {} nodes copied, {} edges copied",
            summary.units_skipped,
            summary.units_reanalyzed,
            summary.nodes_copied,
            summary.edges_copied,
        );
    }

    let total_method_calls = summary.method_calls_resolved + summary.method_calls_unresolved;
    if total_method_calls > 0 {
        println!(
            "    method calls: {} resolved, {} unresolved (of {} total)",
            summary.method_calls_resolved, summary.method_calls_unresolved, total_method_calls,
        );
    }

    if !summary.warnings.is_empty() {
        eprintln!("\n  {} warnings:", summary.warnings.len());
        for w in summary.warnings.iter().take(20) {
            eprintln!("    {} -- {}", w.source_ref, w.message);
        }
        if summary.warnings.len() > 20 {
            eprintln!("    ... and {} more", summary.warnings.len() - 20);
        }
    }

    Ok(())
}

fn run_export(
    store_path: &Path,
    project_id: &str,
    args: &ExportArgs,
    loader: &plugin::PluginLoader,
) -> Result<()> {
    use svt_core::export::ExportRegistry;
    use svt_core::model::SnapshotKind;

    let store = open_store(store_path)?;
    let mut registry = ExportRegistry::with_defaults();
    loader.register_exports(&mut registry);

    let version = match args.version {
        Some(v) => v,
        None => store
            .latest_version(project_id, SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No design versions found in store"))?,
    };

    // PNG requires an output file (binary format)
    if args.format == "png" {
        let output_path = args.output.as_ref().ok_or_else(|| {
            anyhow::anyhow!("PNG is a binary format. Please specify --output FILE")
        })?;
        let png_bytes = svt_core::export::svg::to_png_bytes(&store, version)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        std::fs::write(output_path, &png_bytes)
            .with_context(|| format!("writing to {}", output_path.display()))?;
        println!("Exported PNG to {}", output_path.display());
        return Ok(());
    }

    let exporter = registry.get(&args.format).ok_or_else(|| {
        let available: Vec<&str> = registry.names();
        anyhow::anyhow!(
            "Unknown format: '{}'. Available: {}",
            args.format,
            available.join(", ")
        )
    })?;
    let content = exporter
        .export(&store, version)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &content)
            .with_context(|| format!("writing to {}", output_path.display()))?;
        println!("Exported to {}", output_path.display());
    } else {
        print!("{content}");
    }

    Ok(())
}

fn run_diff(store_path: &Path, args: &DiffArgs) -> Result<()> {
    use svt_core::diff;

    let store = open_store(store_path)?;
    let result =
        diff::diff_snapshots(&store, args.from, args.to).map_err(|e| anyhow::anyhow!("{}", e))?;

    if args.format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&result).map_err(|e| anyhow::anyhow!("{}", e))?
        );
    } else {
        println!("Diff: v{} -> v{}\n", result.from_version, result.to_version);

        if result.node_changes.is_empty() && result.edge_changes.is_empty() {
            println!("  No changes.");
        }

        for change in &result.node_changes {
            let tag = match change.change {
                diff::ChangeKind::Added => "  + ",
                diff::ChangeKind::Removed => "  - ",
                diff::ChangeKind::Changed => "  ~ ",
            };
            print!(
                "{}{} ({:?}/{})",
                tag, change.canonical_path, change.kind, change.sub_kind
            );
            if !change.changed_fields.is_empty() {
                print!(" [{}]", change.changed_fields.join(", "));
            }
            println!();
        }

        if !result.edge_changes.is_empty() {
            println!();
            for change in &result.edge_changes {
                let tag = match change.change {
                    diff::ChangeKind::Added => "  + ",
                    diff::ChangeKind::Removed => "  - ",
                    diff::ChangeKind::Changed => "  ~ ",
                };
                println!(
                    "{}{} -> {} ({:?})",
                    tag, change.source_path, change.target_path, change.edge_kind
                );
            }
        }

        println!(
            "\n  {} added, {} removed, {} changed nodes; {} added, {} removed edges",
            result.summary.nodes_added,
            result.summary.nodes_removed,
            result.summary.nodes_changed,
            result.summary.edges_added,
            result.summary.edges_removed,
        );
    }

    Ok(())
}

/// Build a [`PluginLoader`](plugin::PluginLoader) from CLI flags and the plugins directory.
///
/// Plugins are loaded from two sources in order:
/// 1. Paths explicitly passed via `--plugin` flags.
/// 2. The plugins directory (from `--plugin-dir`, `SVT_PLUGIN_DIR`, or binary-adjacent).
///
/// Load failures are printed as warnings to stderr but do not abort execution.
fn build_plugin_loader(
    plugin_paths: &[PathBuf],
    plugin_dir_override: Option<&Path>,
) -> plugin::PluginLoader {
    let mut loader = plugin::PluginLoader::new();

    // 1. CLI-specified plugins
    for path in plugin_paths {
        if let Err(e) = loader.load_with_source(path, plugin::PluginSource::CliFlag) {
            eprintln!("  WARN  {e}");
        }
    }

    // 2. Plugins directory (override or binary-adjacent default)
    let plugins_dir = plugin_dir_override
        .map(PathBuf::from)
        .or_else(binary_adjacent_plugins_dir);
    if let Some(dir) = plugins_dir {
        for e in loader.scan_directory_with_source(&dir, plugin::PluginSource::BinaryAdjacent) {
            eprintln!("  WARN  {e}");
        }
    }

    loader
}

/// Get the `plugins/` directory adjacent to the currently running binary.
fn binary_adjacent_plugins_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|dir| dir.join("plugins"))
}

/// List all loaded plugins and their contributions to stdout.
fn run_plugin_list(loader: &plugin::PluginLoader) -> Result<()> {
    let loaded = loader.plugins();
    if loaded.is_empty() {
        println!("No plugins loaded.");
        return Ok(());
    }

    for lp in loaded {
        let p = lp.plugin();

        // Use manifest metadata if available for richer display
        if let Some(ref m) = lp.manifest {
            println!(
                "{} v{} (API v{}) [{}]",
                m.plugin.name, m.plugin.version, m.plugin.api_version, lp.source,
            );
            if !m.plugin.description.is_empty() {
                println!("  {}", m.plugin.description);
            }
        } else {
            println!(
                "{} v{} (API v{}) [{}]",
                p.name(),
                p.version(),
                p.api_version(),
                lp.source,
            );
        }

        let evaluators = p.constraint_evaluators();
        if !evaluators.is_empty() {
            println!("  Constraint evaluators:");
            for eval in &evaluators {
                println!("    - {}", eval.kind());
            }
        }
        let formats = p.export_formats();
        if !formats.is_empty() {
            println!("  Export formats:");
            for fmt in &formats {
                println!("    - {}", fmt.name());
            }
        }
        let parsers = p.language_parsers();
        if !parsers.is_empty() {
            println!("  Language parsers:");
            for (desc, _) in &parsers {
                println!(
                    "    - {} (manifests: {}, extensions: {})",
                    desc.language_id,
                    desc.manifest_files.join(", "),
                    desc.source_extensions.join(", "),
                );
            }
        }
        if evaluators.is_empty() && formats.is_empty() && parsers.is_empty() {
            println!("  (no contributions)");
        }
    }
    Ok(())
}

fn run_store_info(store_path: &Path) -> Result<()> {
    let store = open_store(store_path)?;
    let info = store.store_info().map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Store: {}", store_path.display());
    println!("  Schema version: {}", info.schema_version);
    println!("  Snapshots: {}", info.snapshot_count);

    if !info.snapshots.is_empty() {
        println!();
        println!(
            "  {:>7}  {:>10}  {:>5}  {:>5}  {:>12}  CREATED",
            "VERSION", "KIND", "NODES", "EDGES", "COMMIT"
        );
        for snap in &info.snapshots {
            let commit = snap
                .commit_ref
                .as_deref()
                .map(|c| if c.len() > 12 { &c[..12] } else { c })
                .unwrap_or("-");
            println!(
                "  {:>7}  {:>10}  {:>5}  {:>5}  {:>12}  {}",
                snap.version,
                format!("{:?}", snap.kind).to_lowercase(),
                snap.node_count,
                snap.edge_count,
                commit,
                snap.created_at,
            );
        }
    }

    Ok(())
}

fn run_store_compact(store_path: &Path, project_id: &str, args: &CompactArgs) -> Result<()> {
    use svt_core::model::SnapshotKind;

    let mut store = open_store(store_path)?;

    let keep_versions = if args.keep.is_empty() {
        // Default: keep latest design + latest analysis
        let mut versions = Vec::new();
        if let Some(v) = store
            .latest_version(project_id, SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
        {
            versions.push(v);
        }
        if let Some(v) = store
            .latest_version(project_id, SnapshotKind::Analysis)
            .map_err(|e| anyhow::anyhow!("{}", e))?
        {
            versions.push(v);
        }
        versions
    } else {
        args.keep.clone()
    };

    let before = store
        .list_snapshots(project_id)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();

    store
        .compact(project_id, &keep_versions)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let after = store
        .list_snapshots(project_id)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();

    let removed = before.saturating_sub(after);
    println!(
        "Compacted store: kept {} versions, removed {}",
        after, removed
    );

    Ok(())
}

fn run_store_reset(store_path: &Path, args: &ResetArgs) -> Result<()> {
    if !store_path.exists() {
        bail!(
            "Store not found at {}. Nothing to reset.",
            store_path.display()
        );
    }

    if !args.force {
        eprint!(
            "This will delete all data in {}. Continue? [y/N] ",
            store_path.display()
        );
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    std::fs::remove_file(store_path)
        .with_context(|| format!("deleting store at {}", store_path.display()))?;

    // Recreate empty store
    open_or_create_store(store_path)?;
    println!("Store reset: {}", store_path.display());

    Ok(())
}

/// Push a single snapshot version to a remote server.
fn push_version(store: &CozoStore, project_id: &str, version: u64, server_url: &str) -> Result<()> {
    let nodes = store
        .get_all_nodes(version)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let edges = store
        .get_all_edges(version, None)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let constraints = store
        .get_constraints(version)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let snapshots = store
        .list_snapshots(project_id)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let snapshot = snapshots
        .iter()
        .find(|s| s.version == version)
        .ok_or_else(|| anyhow::anyhow!("Snapshot v{} not found", version))?;

    let payload = serde_json::json!({
        "kind": snapshot.kind,
        "commit_ref": snapshot.commit_ref,
        "nodes": nodes,
        "edges": edges,
        "constraints": constraints,
    });

    let url = format!(
        "{}/api/projects/{}/push",
        server_url.trim_end_matches('/'),
        project_id
    );

    match ureq::post(&url).send_json(&payload) {
        Ok(_resp) => {
            println!("Pushed v{} ({:?}) to {}", version, snapshot.kind, url);
            Ok(())
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            bail!("push failed (HTTP {}): {}", code, body);
        }
        Err(e) => {
            bail!("push to {} failed: {}", url, e);
        }
    }
}

fn run_push(
    store_path: &Path,
    project_id: &str,
    args: &PushArgs,
    resolved: &ResolvedConfig,
) -> Result<()> {
    use svt_core::model::SnapshotKind;

    // Resolve server URL from flag or config
    let server_url = args
        .server
        .clone()
        .or_else(|| {
            resolved
                .config
                .as_ref()
                .and_then(|c| c.server.as_ref())
                .map(|s| s.url.clone())
        })
        .ok_or_else(|| {
            anyhow::anyhow!("No server URL. Use --server or set server.url in .svt/config.yaml")
        })?;

    let store = open_store(store_path)?;

    match args.kind.as_str() {
        "design" => {
            let version = match args.version {
                Some(v) => v,
                None => store
                    .latest_version(project_id, SnapshotKind::Design)
                    .map_err(|e| anyhow::anyhow!("{}", e))?
                    .ok_or_else(|| anyhow::anyhow!("No design versions found"))?,
            };
            push_version(&store, project_id, version, &server_url)?;
        }
        "analysis" => {
            let version = match args.version {
                Some(v) => v,
                None => store
                    .latest_version(project_id, SnapshotKind::Analysis)
                    .map_err(|e| anyhow::anyhow!("{}", e))?
                    .ok_or_else(|| anyhow::anyhow!("No analysis versions found"))?,
            };
            push_version(&store, project_id, version, &server_url)?;
        }
        "all" => {
            if let Some(design_v) = store
                .latest_version(project_id, SnapshotKind::Design)
                .map_err(|e| anyhow::anyhow!("{}", e))?
            {
                push_version(&store, project_id, design_v, &server_url)?;
            }
            if let Some(analysis_v) = store
                .latest_version(project_id, SnapshotKind::Analysis)
                .map_err(|e| anyhow::anyhow!("{}", e))?
            {
                push_version(&store, project_id, analysis_v, &server_url)?;
            }
        }
        other => bail!(
            "Unknown push kind: '{}'. Use design, analysis, or all",
            other
        ),
    }

    Ok(())
}

fn run_init(args: &InitArgs) -> Result<()> {
    let project_name = match &args.project {
        Some(name) => name.clone(),
        None => derive_project_name().unwrap_or_else(|| "my-project".to_string()),
    };

    // Create .svt/ and .svt/data/ directories
    std::fs::create_dir_all(".svt/data").context("creating .svt/data directory")?;

    // Write .svt/config.yaml
    let config_path = PathBuf::from(".svt/config.yaml");
    if config_path.exists() {
        bail!(".svt/config.yaml already exists. Aborting.");
    }

    // Check for common design file locations
    let design_file = ["design/architecture.yaml", "design/architecture.yml"]
        .iter()
        .find(|p| PathBuf::from(p).exists());

    let design_section = if let Some(path) = design_file {
        format!("design:\n  - {path}")
    } else {
        "# design:\n#   - design/architecture.yaml".to_string()
    };

    let yaml = format!(
        "\
# SVT project configuration
project: {project_name}
# name: {project_name}
# description: A brief description of the project

# Design model files (relative paths, .yaml/.yml/.json)
{design_section}

# Source directories to analyze
sources:
  - path: .
#   exclude:
#     - vendor
#     - node_modules

# Remote server (uncomment to enable push)
# server:
#   url: http://localhost:3000
"
    );
    std::fs::write(&config_path, &yaml).context("writing .svt/config.yaml")?;

    // Append .svt/data to .gitignore if not already present
    let gitignore_path = PathBuf::from(".gitignore");
    let needs_append = if gitignore_path.exists() {
        let content = std::fs::read_to_string(&gitignore_path).context("reading .gitignore")?;
        !content.lines().any(|line| line.trim() == ".svt/data")
    } else {
        true
    };

    if needs_append {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&gitignore_path)
            .context("opening .gitignore")?;
        writeln!(file, ".svt/data")?;
    }

    println!("Initialized project '{}' in .svt/", project_name);
    println!("  Created .svt/config.yaml");
    println!("  Created .svt/data/");

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let loader = build_plugin_loader(&cli.plugins, cli.plugin_dir.as_deref());

    // Resolve project config (config file + defaults)
    let resolved = ResolvedConfig::resolve(&cli.project_dir)?;

    match &cli.command {
        Commands::Import(args) => {
            run_import(&resolved.store_path, &resolved.project_id, args, &resolved)
        }
        Commands::Check(args) => {
            run_check(&resolved.store_path, &resolved.project_id, args, &loader)
        }
        Commands::Analyze(args) => run_analyze(
            &resolved.store_path,
            &resolved.project_id,
            args,
            &loader,
            &resolved,
        ),
        Commands::Export(args) => {
            run_export(&resolved.store_path, &resolved.project_id, args, &loader)
        }
        Commands::Diff(args) => run_diff(&resolved.store_path, args),
        Commands::Push(args) => {
            run_push(&resolved.store_path, &resolved.project_id, args, &resolved)
        }
        Commands::Plugin(args) => match &args.command {
            PluginCommands::List => run_plugin_list(&loader),
            PluginCommands::Install(install_args) => {
                plugin_commands::run_install(&install_args.source, install_args.force)
            }
            PluginCommands::Remove(remove_args) => plugin_commands::run_remove(&remove_args.name),
            PluginCommands::Info(info_args) => plugin_commands::run_info(&info_args.path),
        },
        Commands::Store(args) => match &args.command {
            StoreCommands::Info => run_store_info(&resolved.store_path),
            StoreCommands::Compact(compact_args) => {
                run_store_compact(&resolved.store_path, &resolved.project_id, compact_args)
            }
            StoreCommands::Reset(reset_args) => run_store_reset(&resolved.store_path, reset_args),
        },
        Commands::Init(args) => run_init(args),
    }
}
