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
