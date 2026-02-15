use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

fn node(id: &str, path: &str, kind: NodeKind, sub_kind: &str) -> Node {
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

fn edge(id: &str, source: &str, target: &str, kind: EdgeKind) -> Edge {
    Edge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        kind,
        provenance: Provenance::Design,
        metadata: None,
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
            &node("svc", "/test-service", NodeKind::Service, "crate"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &node(
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
            &node(
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
            &node(
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
            &node(
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
            &node(
                "order",
                "/test-service/models/order",
                NodeKind::Unit,
                "struct",
            ),
        )
        .unwrap();

    // Containment hierarchy
    store
        .add_edge(version, &edge("c1", "svc", "handlers", EdgeKind::Contains))
        .unwrap();
    store
        .add_edge(version, &edge("c2", "svc", "models", EdgeKind::Contains))
        .unwrap();
    store
        .add_edge(
            version,
            &edge("c3", "handlers", "create", EdgeKind::Contains),
        )
        .unwrap();
    store
        .add_edge(
            version,
            &edge("c4", "handlers", "delete", EdgeKind::Contains),
        )
        .unwrap();
    store
        .add_edge(version, &edge("c5", "models", "order", EdgeKind::Contains))
        .unwrap();

    // Dependency: create handler depends on order model
    store
        .add_edge(version, &edge("d1", "create", "order", EdgeKind::Depends))
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
        .add_node(version, &node("app", "/app", NodeKind::System, "workspace"))
        .unwrap();
    store
        .add_node(
            version,
            &node("api", "/app/api", NodeKind::Component, "module"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &node("service", "/app/service", NodeKind::Component, "module"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &node("repo", "/app/repository", NodeKind::Component, "module"),
        )
        .unwrap();
    store
        .add_node(
            version,
            &node("db", "/app/database", NodeKind::Component, "module"),
        )
        .unwrap();

    // Containment
    store
        .add_edge(version, &edge("c1", "app", "api", EdgeKind::Contains))
        .unwrap();
    store
        .add_edge(version, &edge("c2", "app", "service", EdgeKind::Contains))
        .unwrap();
    store
        .add_edge(version, &edge("c3", "app", "repo", EdgeKind::Contains))
        .unwrap();
    store
        .add_edge(version, &edge("c4", "app", "db", EdgeKind::Contains))
        .unwrap();

    // Layer dependencies: api -> service -> repo -> db
    store
        .add_edge(version, &edge("d1", "api", "service", EdgeKind::Depends))
        .unwrap();
    store
        .add_edge(version, &edge("d2", "service", "repo", EdgeKind::Depends))
        .unwrap();
    store
        .add_edge(version, &edge("d3", "repo", "db", EdgeKind::Depends))
        .unwrap();
}
