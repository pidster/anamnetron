//! `svt-server` -- Axum API service for software-visualizer-tool.

#![warn(missing_docs)]

mod error;
mod routes;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;

use svt_analyzer::analyze_project;
use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::model::SnapshotKind;
use svt_core::store::{CozoStore, GraphStore};

use crate::state::AppState;

/// Software Visualizer Tool — API server.
#[derive(Parser, Debug)]
#[command(name = "svt-server", version, about)]
struct Args {
    /// Path to a Rust project to analyze at startup.
    #[arg(long)]
    project: Option<PathBuf>,

    /// Path to a design YAML/JSON file to import at startup.
    #[arg(long)]
    design: Option<PathBuf>,

    /// Path to a persistent store (SQLite-backed CozoDB).
    /// If omitted, an in-memory store is used and data is lost on restart.
    #[arg(long)]
    store: Option<PathBuf>,

    /// Port to listen on.
    #[arg(long, default_value = "3000")]
    port: u16,

    /// Host to bind to.
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    // Create persistent or in-memory store
    let mut store = if let Some(store_path) = &args.store {
        if let Some(parent) = store_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating store directory {}", parent.display()))?;
        }
        let s = CozoStore::new_persistent(store_path)?;
        info!(path = %store_path.display(), "using persistent store");
        s
    } else {
        CozoStore::new_in_memory()?
    };

    // Read existing versions from persistent store
    let mut design_version = store.latest_version(SnapshotKind::Design)?;
    let mut analysis_version = store.latest_version(SnapshotKind::Analysis)?;

    // If no store and no flags, require at least one input source
    if args.store.is_none() && args.project.is_none() && args.design.is_none() {
        anyhow::bail!("at least one of --store, --project, or --design is required");
    }

    // Import design if provided (layers on top of existing data)
    if let Some(design_path) = &args.design {
        let content = std::fs::read_to_string(design_path)
            .with_context(|| format!("failed to read {}", design_path.display()))?;
        let doc = interchange::parse_yaml(&content)
            .with_context(|| format!("failed to parse {}", design_path.display()))?;
        let version = interchange_store::load_into_store(&mut store, &doc)?;
        design_version = Some(version);
        info!(version, path = %design_path.display(), "imported design");
    }

    // Analyze project if provided (layers on top of existing data)
    if let Some(project_path) = &args.project {
        let summary = analyze_project(&mut store, project_path, None)
            .with_context(|| format!("failed to analyze {}", project_path.display()))?;
        analysis_version = Some(summary.version);
        info!(
            version = summary.version,
            crates = summary.crates_analyzed,
            nodes = summary.nodes_created,
            edges = summary.edges_created,
            "analyzed project"
        );
    }

    if design_version.is_some() || analysis_version.is_some() {
        info!(
            design = ?design_version,
            analysis = ?analysis_version,
            "serving versions"
        );
    }

    let state = Arc::new(AppState {
        store,
        design_version,
        analysis_version,
    });

    let static_dir = std::path::PathBuf::from("web/dist");
    let app = routes::full_router(state, Some(static_dir));

    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind_addr).await?;
    info!(address = %bind_addr, "svt-server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
