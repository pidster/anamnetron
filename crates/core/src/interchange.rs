//! Interchange format: YAML/JSON import and export wire types.
//!
//! This module defines the serialization types and parsing functions for
//! the `svt/v1` interchange format. Always compiled, WASM-safe.

use serde::{Deserialize, Serialize};

use crate::model::{EdgeKind, NodeKind, Provenance, Severity, SnapshotKind, Version};

/// Errors during interchange parsing or validation.
#[derive(Debug, thiserror::Error)]
pub enum InterchangeError {
    /// YAML or JSON parse error.
    #[error("parse error: {0}")]
    Parse(String),

    /// Unsupported format version.
    #[error("unsupported format: expected 'svt/v1', got '{0}'")]
    UnsupportedFormat(String),

    /// Document validation error.
    #[error("validation error: {0}")]
    Validation(String),
}

/// A warning produced during document validation (non-fatal).
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// The path or element the warning relates to.
    pub path: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Top-level interchange document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeDocument {
    /// Format version string, must be `"svt/v1"`.
    pub format: String,
    /// Snapshot kind (design, analysis, import).
    pub kind: SnapshotKind,
    /// Optional version number (informational).
    pub version: Option<Version>,
    /// Optional metadata.
    pub metadata: Option<serde_json::Value>,
    /// Node definitions (may contain nested children).
    #[serde(default)]
    pub nodes: Vec<InterchangeNode>,
    /// Edge definitions (canonical path references).
    #[serde(default)]
    pub edges: Vec<InterchangeEdge>,
    /// Constraint definitions.
    #[serde(default)]
    pub constraints: Vec<InterchangeConstraint>,
}

/// A node in the interchange format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeNode {
    /// Canonical path (required).
    pub canonical_path: String,
    /// Node kind (required).
    pub kind: NodeKind,
    /// Human-readable name. Inferred from last path segment if omitted.
    pub name: Option<String>,
    /// Language-specific type. Defaults to generic for the kind if omitted.
    pub sub_kind: Option<String>,
    /// Language-specific qualified name.
    pub qualified_name: Option<String>,
    /// Source language.
    pub language: Option<String>,
    /// Provenance. Inferred from document kind if omitted.
    pub provenance: Option<Provenance>,
    /// File path or URL reference.
    pub source_ref: Option<String>,
    /// Extensible metadata.
    pub metadata: Option<serde_json::Value>,
    /// Nested children (shorthand for containment).
    #[serde(default)]
    pub children: Option<Vec<InterchangeNode>>,
}

/// An edge in the interchange format. References canonical paths, not UUIDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeEdge {
    /// Source node canonical path.
    pub source: String,
    /// Target node canonical path.
    pub target: String,
    /// Edge kind.
    pub kind: EdgeKind,
    /// Extensible metadata.
    pub metadata: Option<serde_json::Value>,
}

/// A constraint in the interchange format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeConstraint {
    /// Human-readable name.
    pub name: String,
    /// Constraint kind (e.g., "must_not_depend", "boundary").
    pub kind: String,
    /// Scope pattern (canonical path glob).
    pub scope: String,
    /// Target pattern (for dependency constraints).
    pub target: Option<String>,
    /// Additional parameters.
    pub params: Option<serde_json::Value>,
    /// Description shown on violation.
    pub message: String,
    /// Severity. Defaults to Error if omitted.
    pub severity: Option<Severity>,
}

/// Parse a YAML string into an interchange document.
///
/// Checks the format version and flattens nested children into
/// explicit nodes and `Contains` edges.
pub fn parse_yaml(input: &str) -> Result<InterchangeDocument, InterchangeError> {
    let mut doc: InterchangeDocument =
        serde_yaml::from_str(input).map_err(|e| InterchangeError::Parse(e.to_string()))?;

    if doc.format != "svt/v1" {
        return Err(InterchangeError::UnsupportedFormat(doc.format));
    }

    // Flatten nested children (generates Contains edges)
    let (flat_nodes, contains_edges) = flatten_nodes(&doc.nodes);
    doc.nodes = flat_nodes;
    doc.edges.extend(contains_edges);

    Ok(doc)
}

/// Recursively flatten nested children into a flat node list and Contains edges.
fn flatten_nodes(nodes: &[InterchangeNode]) -> (Vec<InterchangeNode>, Vec<InterchangeEdge>) {
    let mut flat = Vec::new();
    let mut edges = Vec::new();

    fn recurse(
        node: &InterchangeNode,
        flat: &mut Vec<InterchangeNode>,
        edges: &mut Vec<InterchangeEdge>,
    ) {
        // Add this node without children
        let mut flat_node = node.clone();
        flat_node.children = None;
        flat.push(flat_node);

        if let Some(children) = &node.children {
            for child in children {
                edges.push(InterchangeEdge {
                    source: node.canonical_path.clone(),
                    target: child.canonical_path.clone(),
                    kind: EdgeKind::Contains,
                    metadata: None,
                });
                recurse(child, flat, edges);
            }
        }
    }

    for node in nodes {
        recurse(node, &mut flat, &mut edges);
    }

    (flat, edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interchange_node_deserialises_from_yaml() {
        let yaml = r#"
canonical_path: /svt/core
kind: service
sub_kind: crate
name: core
"#;
        let node: InterchangeNode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(node.canonical_path, "/svt/core");
        assert_eq!(node.kind, NodeKind::Service);
        assert_eq!(node.sub_kind, Some("crate".to_string()));
    }

    #[test]
    fn interchange_node_optional_fields_default_to_none() {
        let yaml = r#"
canonical_path: /svt/core
kind: service
"#;
        let node: InterchangeNode = serde_yaml::from_str(yaml).unwrap();
        assert!(node.name.is_none());
        assert!(node.sub_kind.is_none());
        assert!(node.children.is_none());
    }

    #[test]
    fn interchange_edge_deserialises_from_yaml() {
        let yaml = r#"
source: /svt/cli
target: /svt/core
kind: depends
"#;
        let edge: InterchangeEdge = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(edge.source, "/svt/cli");
        assert_eq!(edge.kind, EdgeKind::Depends);
    }

    #[test]
    fn parse_yaml_flat_document() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
  - canonical_path: /app/api
    kind: component
    sub_kind: module
    name: api
edges:
  - source: /app/api
    target: /app
    kind: contains
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(doc.format, "svt/v1");
        assert_eq!(doc.kind, SnapshotKind::Design);
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(doc.edges.len(), 1);
    }

    #[test]
    fn parse_yaml_rejects_unknown_format() {
        let yaml = r#"
format: svt/v99
kind: design
nodes: []
"#;
        let err = parse_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("svt/v99"));
    }

    #[test]
    fn parse_yaml_nested_generates_contains_edges() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
    children:
      - canonical_path: /app/api
        kind: component
        sub_kind: module
        name: api
      - canonical_path: /app/db
        kind: component
        sub_kind: module
        name: db
edges: []
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(doc.nodes.len(), 3, "should flatten to 3 nodes");
        // 2 contains edges generated from children
        let contains: Vec<_> = doc
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Contains)
            .collect();
        assert_eq!(contains.len(), 2);
        assert_eq!(contains[0].source, "/app");
        assert_eq!(contains[0].target, "/app/api");
    }

    #[test]
    fn parse_yaml_deeply_nested_children() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
        children:
          - canonical_path: /app/core/model
            kind: component
"#;
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(doc.nodes.len(), 3);
        assert_eq!(doc.edges.len(), 2, "should have 2 contains edges");
    }

    #[test]
    fn interchange_constraint_deserialises_severity() {
        let yaml = r#"
name: no-outward
kind: must_not_depend
scope: /svt/core/**
target: /svt/cli/**
message: "Core must not depend on CLI"
severity: error
"#;
        let c: InterchangeConstraint = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(c.severity, Some(Severity::Error));
    }
}
