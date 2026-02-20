//! SVG and PNG export via Graphviz `dot` command.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::model::Version;
use crate::store::{GraphStore, Result, StoreError};

/// Generate SVG output by piping DOT through Graphviz `dot -Tsvg`.
///
/// Requires the `dot` command to be installed on the system (from Graphviz).
pub fn to_svg(store: &dyn GraphStore, version: Version) -> Result<String> {
    let dot_source = super::dot::to_dot(store, version)?;
    pipe_through_dot(&dot_source, "svg")
}

/// Generate PNG output by piping DOT through Graphviz `dot -Tpng`.
///
/// Returns the PNG binary data as raw bytes.
pub fn to_png_bytes(store: &dyn GraphStore, version: Version) -> Result<Vec<u8>> {
    let dot_source = super::dot::to_dot(store, version)?;
    pipe_through_dot_bytes(&dot_source, "png")
}

/// Pipe DOT source through `dot -T<format>` and return the output as a string.
fn pipe_through_dot(dot_source: &str, format: &str) -> Result<String> {
    let bytes = pipe_through_dot_bytes(dot_source, format)?;
    String::from_utf8(bytes)
        .map_err(|e| StoreError::Internal(format!("dot produced invalid UTF-8: {e}")))
}

/// Pipe DOT source through `dot -T<format>` and return raw bytes.
fn pipe_through_dot_bytes(dot_source: &str, format: &str) -> Result<Vec<u8>> {
    let mut child = Command::new("dot")
        .arg(format!("-T{format}"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StoreError::Internal(
                    "Graphviz `dot` command not found. \
                     Install from https://graphviz.org/"
                        .to_string(),
                )
            } else {
                StoreError::Internal(format!("failed to run `dot`: {e}"))
            }
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(dot_source.as_bytes())
            .map_err(|e| StoreError::Internal(format!("failed to write to dot stdin: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| StoreError::Internal(format!("failed to read dot output: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(StoreError::Internal(format!(
            "dot command failed: {stderr}"
        )));
    }

    Ok(output.stdout)
}
