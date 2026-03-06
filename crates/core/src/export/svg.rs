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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interchange::parse_yaml;
    use crate::interchange_store::load_into_store;
    use crate::model::{Project, DEFAULT_PROJECT_ID};
    use crate::store::{CozoStore, GraphStore, StoreError};

    fn make_store_with_graph() -> (CozoStore, Version) {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
edges:
  - source: /app/core
    target: /app
    kind: depends
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let _ = store.create_project(&Project {
            id: DEFAULT_PROJECT_ID.to_string(),
            name: "Default".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            description: None,
            metadata: None,
        });
        let version = load_into_store(&mut store, DEFAULT_PROJECT_ID, &doc).unwrap();
        (store, version)
    }

    fn dot_is_available() -> bool {
        Command::new("dot").arg("-V").output().is_ok()
    }

    #[test]
    fn to_svg_returns_error_when_dot_not_installed() {
        if dot_is_available() {
            // Skip this test if dot is installed — we can't trigger the error
            return;
        }
        let (store, version) = make_store_with_graph();
        let result = to_svg(&store, version);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            StoreError::Internal(msg) => {
                assert!(
                    msg.contains("not found"),
                    "error should mention dot not found, got: {msg}"
                );
            }
            other => panic!("expected StoreError::Internal, got: {other:?}"),
        }
    }

    #[test]
    fn to_png_bytes_returns_error_when_dot_not_installed() {
        if dot_is_available() {
            return;
        }
        let (store, version) = make_store_with_graph();
        let result = to_png_bytes(&store, version);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            StoreError::Internal(msg) => {
                assert!(
                    msg.contains("not found"),
                    "error should mention dot not found, got: {msg}"
                );
            }
            other => panic!("expected StoreError::Internal, got: {other:?}"),
        }
    }

    #[test]
    #[ignore = "requires Graphviz `dot` to be installed"]
    fn to_svg_produces_valid_svg_output() {
        let (store, version) = make_store_with_graph();
        let svg = to_svg(&store, version).unwrap();
        assert!(
            svg.contains("<svg"),
            "output should contain <svg tag, got: {}",
            &svg[..svg.len().min(200)]
        );
        assert!(
            svg.contains("</svg>"),
            "output should contain closing </svg>"
        );
    }

    #[test]
    #[ignore = "requires Graphviz `dot` to be installed"]
    fn to_png_bytes_produces_non_empty_output() {
        let (store, version) = make_store_with_graph();
        let bytes = to_png_bytes(&store, version).unwrap();
        assert!(!bytes.is_empty(), "PNG output should not be empty");
        // PNG magic bytes
        assert_eq!(
            &bytes[..4],
            &[0x89, 0x50, 0x4E, 0x47],
            "output should start with PNG magic bytes"
        );
    }
}
