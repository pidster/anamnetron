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
}
