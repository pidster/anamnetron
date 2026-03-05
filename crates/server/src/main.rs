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

use svt_core::model::{Project, DEFAULT_PROJECT_ID};
use svt_core::store::{CozoStore, GraphStore};

use crate::state::AppState;

/// Anamnetron — API server.
#[derive(Parser, Debug)]
#[command(name = "svt-server", version, about)]
struct Args {
    /// Path to a persistent store (SQLite-backed CozoDB).
    #[arg(long)]
    store: PathBuf,

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

    // Open persistent store
    let store_path = &args.store;
    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating store directory {}", parent.display()))?;
    }
    let mut store = CozoStore::new_persistent(store_path)?;
    info!(path = %store_path.display(), "using persistent store");

    // Ensure the default project exists
    let project_id = DEFAULT_PROJECT_ID;
    if !store.project_exists(project_id)? {
        let now = chrono::Utc::now().to_rfc3339();
        store.create_project(&Project {
            id: project_id.to_string(),
            name: project_id.to_string(),
            created_at: now,
            description: None,
            metadata: None,
        })?;
        info!(project = %project_id, "created project");
    }

    let state = Arc::new(AppState {
        store: std::sync::RwLock::new(store),
        default_project: project_id.to_string(),
    });

    let static_dir = std::path::PathBuf::from("web/dist");
    let app = routes::full_router(state, Some(static_dir));

    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind_addr).await?;
    info!(address = %bind_addr, "svt-server listening");
    axum::serve(listener, app).await?;

    Ok(())
}
