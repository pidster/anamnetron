//! `svt-server` -- Axum API service for software-visualizer-tool.

#![warn(missing_docs)]

mod error;
mod routes;
mod state;

use std::sync::Arc;

use tokio::net::TcpListener;

use svt_core::store::CozoStore;

use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = CozoStore::new_in_memory()?;
    let state = Arc::new(AppState {
        store,
        design_version: None,
        analysis_version: None,
    });

    let app = routes::api_router(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("svt-server listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
