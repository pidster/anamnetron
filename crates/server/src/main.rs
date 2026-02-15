//! `svt-server` -- Axum API service for software-visualizer-tool.

#![warn(missing_docs)]

use axum::{routing::get, Router};
use tokio::net::TcpListener;

async fn hello() -> &'static str {
    "software-visualizer-tool server"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = Router::new().route("/", get(hello));

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("svt-server listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
