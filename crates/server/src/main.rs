//! `svt-server` -- Axum API service for Anamnetron.

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
use svt_core::model::{validate_project_id, Project, DEFAULT_PROJECT_ID};
use svt_core::store::{CozoStore, GraphStore};

use crate::state::AppState;

/// Anamnetron — API server.
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

    /// Project name/ID to use for imported data.
    #[arg(long, default_value = DEFAULT_PROJECT_ID)]
    project_name: String,

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

    // Validate project name
    validate_project_id(&args.project_name)
        .map_err(|e| anyhow::anyhow!("invalid --project-name: {e}"))?;

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

    // Ensure the project exists
    let project_id = &args.project_name;
    if !store.project_exists(project_id)? {
        let now = chrono::Utc::now().to_rfc3339();
        store.create_project(&Project {
            id: project_id.clone(),
            name: project_id.clone(),
            created_at: now,
            description: None,
            metadata: None,
        })?;
        info!(project = %project_id, "created project");
    }

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
        let version = interchange_store::load_into_store(&mut store, project_id, &doc)?;
        info!(version, project = %project_id, path = %design_path.display(), "imported design");
    }

    // Analyze project if provided (layers on top of existing data)
    if let Some(project_path) = &args.project {
        let summary = analyze_project(&mut store, project_id, project_path, None)
            .with_context(|| format!("failed to analyze {}", project_path.display()))?;
        info!(
            version = summary.version,
            project = %project_id,
            crates = summary.crates_analyzed,
            nodes = summary.nodes_created,
            edges = summary.edges_created,
            "analyzed project"
        );
    }

    let state = Arc::new(AppState {
        store: std::sync::RwLock::new(store),
        default_project: project_id.clone(),
    });

    let static_dir = std::path::PathBuf::from("web/dist");
    let app = routes::full_router(state, Some(static_dir));

    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind_addr).await?;
    info!(address = %bind_addr, "svt-server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
