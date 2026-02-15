//! Core data model types for the software visualizer graph.

use serde::{Deserialize, Serialize};

/// Snapshot version number. Monotonically increasing.
pub type Version = u64;

/// Unique identifier for a node (UUID v4).
pub type NodeId = String;

/// Unique identifier for an edge (UUID v4).
pub type EdgeId = String;

/// Abstraction level of a node in the architecture hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// Top-level boundary (workspace, monorepo, solution).
    System,
    /// Deployable or distributable unit (crate, package, assembly).
    Service,
    /// Logical grouping within a service (module, namespace, package).
    Component,
    /// Individual code element (class, struct, function, trait).
    Unit,
}

/// Relationship type between nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Hierarchical nesting (parent contains child).
    Contains,
    /// Import/use dependency.
    Depends,
    /// Runtime invocation.
    Calls,
    /// Fulfills a contract (trait, interface, protocol).
    Implements,
    /// Inheritance relationship.
    Extends,
    /// Data movement between elements.
    DataFlow,
    /// Public visibility boundary.
    Exports,
}

/// Origin of a piece of knowledge in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Human-authored, prescriptive.
    Design,
    /// Machine-derived from code analysis.
    Analysis,
    /// Ingested from an external knowledge source.
    Import,
    /// Derived from heuristics or patterns.
    Inferred,
}

/// Type of snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    /// Design model snapshot.
    Design,
    /// Code analysis snapshot.
    Analysis,
    /// External import snapshot.
    Import,
}

/// Severity level for constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Conformance check fails.
    Error,
    /// Reported but does not fail.
    Warning,
    /// Informational only.
    Info,
}

/// Direction for edge queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Edges where the node is the source.
    Outgoing,
    /// Edges where the node is the target.
    Incoming,
    /// Edges in either direction.
    Both,
}

/// A node in the architecture graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Internal unique identifier (UUID v4).
    pub id: NodeId,
    /// Language-neutral path derived from containment hierarchy.
    pub canonical_path: String,
    /// Language-specific qualified name (null for design nodes).
    pub qualified_name: Option<String>,
    /// Abstraction level.
    pub kind: NodeKind,
    /// Language-specific or domain-specific type (e.g., "crate", "class", "trait").
    pub sub_kind: String,
    /// Human-readable name (last segment of canonical path).
    pub name: String,
    /// Source language, if derived from code analysis.
    pub language: Option<String>,
    /// Origin of this knowledge.
    pub provenance: Provenance,
    /// File path, line number, or external URL.
    pub source_ref: Option<String>,
    /// Extensible key-value properties.
    pub metadata: Option<serde_json::Value>,
}

/// An edge (relationship) in the architecture graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier.
    pub id: EdgeId,
    /// Source node ID.
    pub source: NodeId,
    /// Target node ID.
    pub target: NodeId,
    /// Relationship type.
    pub kind: EdgeKind,
    /// Origin of this knowledge.
    pub provenance: Provenance,
    /// Extensible key-value properties.
    pub metadata: Option<serde_json::Value>,
}

/// An architectural constraint (design-mode assertion).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    /// Unique identifier.
    pub id: String,
    /// Constraint type (e.g., "must_not_depend", "boundary"). String for extensibility.
    pub kind: String,
    /// Human-readable name.
    pub name: String,
    /// Canonical path pattern this constraint applies to (supports glob).
    pub scope: String,
    /// Target path pattern (for dependency constraints).
    pub target: Option<String>,
    /// Additional parameters.
    pub params: Option<serde_json::Value>,
    /// Description shown on violation.
    pub message: String,
    /// Severity level.
    pub severity: Severity,
}

/// A versioned snapshot of the graph state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Monotonically increasing version number.
    pub version: Version,
    /// Type of snapshot.
    pub kind: SnapshotKind,
    /// Git commit hash, if applicable.
    pub commit_ref: Option<String>,
    /// Timestamp (informational, not used for ordering).
    pub created_at: String,
    /// Additional context.
    pub metadata: Option<serde_json::Value>,
}

/// Filter criteria for node queries.
#[derive(Debug, Clone, Default)]
pub struct NodeFilter {
    /// Filter by abstraction level.
    pub kind: Option<NodeKind>,
    /// Filter by language-specific type.
    pub sub_kind: Option<String>,
    /// Filter by source language.
    pub language: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_serialises_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&NodeKind::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&NodeKind::Service).unwrap(),
            "\"service\""
        );
        assert_eq!(
            serde_json::to_string(&NodeKind::Component).unwrap(),
            "\"component\""
        );
        assert_eq!(serde_json::to_string(&NodeKind::Unit).unwrap(), "\"unit\"");
    }

    #[test]
    fn edge_kind_round_trips_through_json() {
        for kind in [
            EdgeKind::Contains,
            EdgeKind::Depends,
            EdgeKind::Calls,
            EdgeKind::Implements,
            EdgeKind::Extends,
            EdgeKind::DataFlow,
            EdgeKind::Exports,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: EdgeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn snapshot_kind_round_trips_through_json() {
        for kind in [
            SnapshotKind::Design,
            SnapshotKind::Analysis,
            SnapshotKind::Import,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: SnapshotKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn node_round_trips_through_json() {
        let node = Node {
            id: "test-id".to_string(),
            canonical_path: "/test-service/handlers/create".to_string(),
            qualified_name: Some("test_service::handlers::create".to_string()),
            kind: NodeKind::Unit,
            sub_kind: "function".to_string(),
            name: "create".to_string(),
            language: Some("rust".to_string()),
            provenance: Provenance::Analysis,
            source_ref: Some("src/handlers.rs:42".to_string()),
            metadata: Some(serde_json::json!({"is_async": true})),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, node.id);
        assert_eq!(back.canonical_path, node.canonical_path);
        assert_eq!(back.kind, node.kind);
        assert_eq!(back.qualified_name, node.qualified_name);
    }

    #[test]
    fn node_with_none_fields_round_trips() {
        let node = Node {
            id: "test-id".to_string(),
            canonical_path: "/design-service".to_string(),
            qualified_name: None,
            kind: NodeKind::Service,
            sub_kind: "crate".to_string(),
            name: "design-service".to_string(),
            language: None,
            provenance: Provenance::Design,
            source_ref: None,
            metadata: None,
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.qualified_name, None);
        assert_eq!(back.language, None);
        assert_eq!(back.metadata, None);
    }

    #[test]
    fn edge_round_trips_through_json() {
        let edge = Edge {
            id: "edge-1".to_string(),
            source: "node-a".to_string(),
            target: "node-b".to_string(),
            kind: EdgeKind::Depends,
            provenance: Provenance::Analysis,
            metadata: None,
        };
        let json = serde_json::to_string(&edge).unwrap();
        let back: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, edge.source);
        assert_eq!(back.kind, edge.kind);
    }

    #[test]
    fn constraint_round_trips_through_json() {
        let constraint = Constraint {
            id: "c-1".to_string(),
            kind: "must_not_depend".to_string(),
            name: "no-internal-access".to_string(),
            scope: "/payments/**".to_string(),
            target: Some("/user-service/internal/**".to_string()),
            params: None,
            message: "Payment must not access user internals".to_string(),
            severity: Severity::Error,
        };
        let json = serde_json::to_string(&constraint).unwrap();
        let back: Constraint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, "must_not_depend");
        assert_eq!(back.severity, Severity::Error);
    }
}
