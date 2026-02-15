#![allow(dead_code)]

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

/// Create a node with all fields specified.
pub fn make_node(id: &str, path: &str, kind: NodeKind, sub_kind: &str) -> Node {
    Node {
        id: id.to_string(),
        canonical_path: path.to_string(),
        qualified_name: None,
        kind,
        sub_kind: sub_kind.to_string(),
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        language: None,
        provenance: Provenance::Design,
        source_ref: None,
        metadata: None,
    }
}

/// Create a node with a specified kind, defaulting sub_kind to "module".
pub fn make_node_with_kind(id: &str, path: &str, kind: NodeKind) -> Node {
    make_node(id, path, kind, "module")
}

/// Create a Component node with default sub_kind "module".
pub fn make_node_default(id: &str, path: &str) -> Node {
    make_node(id, path, NodeKind::Component, "module")
}

/// Create an edge with explicit kind.
pub fn make_edge(id: &str, source: &str, target: &str, kind: EdgeKind) -> Edge {
    Edge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        kind,
        provenance: Provenance::Design,
        metadata: None,
    }
}

/// Create a Contains edge.
pub fn make_contains(id: &str, parent: &str, child: &str) -> Edge {
    make_edge(id, parent, child, EdgeKind::Contains)
}

/// Create a Depends edge.
pub fn make_depends(id: &str, source: &str, target: &str) -> Edge {
    make_edge(id, source, target, EdgeKind::Depends)
}

/// Create a Depends edge (default kind) without specifying edge kind.
pub fn make_edge_default(id: &str, source: &str, target: &str) -> Edge {
    make_edge(id, source, target, EdgeKind::Depends)
}

/// Create a test constraint.
pub fn make_constraint(id: &str) -> Constraint {
    Constraint {
        id: id.to_string(),
        kind: "must_not_depend".to_string(),
        name: format!("constraint-{id}"),
        scope: "/a/**".to_string(),
        target: Some("/b/**".to_string()),
        params: None,
        message: "Violation".to_string(),
        severity: Severity::Error,
    }
}

/// Build a simple service graph:
///
/// ```text
/// /test-service (service)
///   /test-service/handlers (component)
///     /test-service/handlers/create (unit, function)
///     /test-service/handlers/delete (unit, function)
///   /test-service/models (component)
///     /test-service/models/order (unit, struct)
/// ```
///
/// Dependencies: handlers/create -> models/order
pub fn create_simple_service(store: &mut CozoStore, version: Version) {
    // Nodes
    store
        .add_node(
            version,
            &make_node("svc", "/test-service", NodeKind::Service, "crate"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node(
                "handlers",
                "/test-service/handlers",
                NodeKind::Component,
                "module",
            ),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node(
                "create",
                "/test-service/handlers/create",
                NodeKind::Unit,
                "function",
            ),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node(
                "delete",
                "/test-service/handlers/delete",
                NodeKind::Unit,
                "function",
            ),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node(
                "models",
                "/test-service/models",
                NodeKind::Component,
                "module",
            ),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node(
                "order",
                "/test-service/models/order",
                NodeKind::Unit,
                "struct",
            ),
        )
        .unwrap();

    // Containment hierarchy
    store
        .add_edge(version, &make_contains("c1", "svc", "handlers"))
        .unwrap();
    store
        .add_edge(version, &make_contains("c2", "svc", "models"))
        .unwrap();
    store
        .add_edge(version, &make_contains("c3", "handlers", "create"))
        .unwrap();
    store
        .add_edge(version, &make_contains("c4", "handlers", "delete"))
        .unwrap();
    store
        .add_edge(version, &make_contains("c5", "models", "order"))
        .unwrap();

    // Dependency: create handler depends on order model
    store
        .add_edge(version, &make_depends("d1", "create", "order"))
        .unwrap();
}

/// Build a layered architecture graph:
///
/// ```text
/// /app (system)
///   /app/api (component)
///   /app/service (component)
///   /app/repository (component)
///   /app/database (component)
/// ```
///
/// Dependencies follow strict layer ordering:
/// api -> service -> repository -> database
pub fn create_layered_architecture(store: &mut CozoStore, version: Version) {
    // Nodes
    store
        .add_node(
            version,
            &make_node("app", "/app", NodeKind::System, "workspace"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node("api", "/app/api", NodeKind::Component, "module"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node("service", "/app/service", NodeKind::Component, "module"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node("repo", "/app/repository", NodeKind::Component, "module"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &make_node("db", "/app/database", NodeKind::Component, "module"),
        )
        .unwrap();

    // Containment
    store
        .add_edge(version, &make_contains("c1", "app", "api"))
        .unwrap();
    store
        .add_edge(version, &make_contains("c2", "app", "service"))
        .unwrap();
    store
        .add_edge(version, &make_contains("c3", "app", "repo"))
        .unwrap();
    store
        .add_edge(version, &make_contains("c4", "app", "db"))
        .unwrap();

    // Layer dependencies: api -> service -> repo -> db
    store
        .add_edge(version, &make_depends("d1", "api", "service"))
        .unwrap();
    store
        .add_edge(version, &make_depends("d2", "service", "repo"))
        .unwrap();
    store
        .add_edge(version, &make_depends("d3", "repo", "db"))
        .unwrap();
}
