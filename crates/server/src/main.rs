//! `svt-server` -- Axum API service for software-visualizer-tool.

#![warn(missing_docs)]

mod error;
mod routes;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context};
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

    if args.project.is_none() && args.design.is_none() {
        bail!("at least one of --project or --design is required");
    }

    let mut store = CozoStore::new_in_memory()?;
    let mut design_version = None;
    let mut analysis_version = None;

    // Import design if provided
    if let Some(design_path) = &args.design {
        let content = std::fs::read_to_string(design_path)
            .with_context(|| format!("failed to read {}", design_path.display()))?;
        let doc = interchange::parse_yaml(&content)
            .with_context(|| format!("failed to parse {}", design_path.display()))?;
        let version = interchange_store::load_into_store(&mut store, &doc)?;
        design_version = Some(version);
        info!(version, path = %design_path.display(), "imported design");
    }

    // Analyze project if provided
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

        // If no design was loaded, check for design version from a previous import
        if design_version.is_none() {
            design_version = store.latest_version(SnapshotKind::Design)?;
        }
    }

    let state = Arc::new(AppState {
        store,
        design_version,
        analysis_version,
    });

    let app = routes::api_router(state);

    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind_addr).await?;
    info!(address = %bind_addr, "svt-server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
