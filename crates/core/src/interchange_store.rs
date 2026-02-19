//! Interchange store operations: loading documents into the graph store and exporting.
//!
//! Feature-gated behind `store`.

use std::collections::HashMap;

use crate::canonical::path_name;
use crate::interchange::*;
use crate::model::*;
use crate::store::{GraphStore, Result, StoreError};

/// Default sub_kind for a node kind when not specified.
fn default_sub_kind(kind: NodeKind) -> String {
    match kind {
        NodeKind::System => "system",
        NodeKind::Service => "service",
        NodeKind::Component => "component",
        NodeKind::Unit => "unit",
    }
    .to_string()
}

/// Infer provenance from the document's snapshot kind.
fn infer_provenance(kind: SnapshotKind) -> Provenance {
    match kind {
        SnapshotKind::Design => Provenance::Design,
        SnapshotKind::Analysis => Provenance::Analysis,
        SnapshotKind::Import => Provenance::Import,
    }
}

/// Load an interchange document into the store, creating a new snapshot.
///
/// Assigns UUIDs, resolves canonical path references to node IDs,
/// and infers missing fields (name, sub_kind, provenance).
pub fn load_into_store(store: &mut impl GraphStore, doc: &InterchangeDocument) -> Result<Version> {
    let version = store.create_snapshot(doc.kind, None)?;

    // Build nodes with UUIDs, collecting path->ID mapping
    let mut path_to_id: HashMap<String, String> = HashMap::new();
    let mut nodes = Vec::with_capacity(doc.nodes.len());

    for inode in &doc.nodes {
        let id = uuid::Uuid::new_v4().to_string();
        path_to_id.insert(inode.canonical_path.clone(), id.clone());

        nodes.push(Node {
            id,
            canonical_path: inode.canonical_path.clone(),
            qualified_name: inode.qualified_name.clone(),
            kind: inode.kind,
            sub_kind: inode
                .sub_kind
                .clone()
                .unwrap_or_else(|| default_sub_kind(inode.kind)),
            name: inode
                .name
                .clone()
                .unwrap_or_else(|| path_name(&inode.canonical_path).to_string()),
            language: inode.language.clone(),
            provenance: inode
                .provenance
                .unwrap_or_else(|| infer_provenance(doc.kind)),
            source_ref: inode.source_ref.clone(),
            metadata: inode.metadata.clone(),
        });
    }

    store.add_nodes_batch(version, &nodes)?;

    // Build edges, resolving canonical paths to node IDs
    let mut edges = Vec::with_capacity(doc.edges.len());
    for iedge in &doc.edges {
        let source_id = path_to_id.get(&iedge.source).ok_or_else(|| {
            StoreError::Internal(format!(
                "edge source path '{}' not found in nodes",
                iedge.source
            ))
        })?;
        let target_id = path_to_id.get(&iedge.target).ok_or_else(|| {
            StoreError::Internal(format!(
                "edge target path '{}' not found in nodes",
                iedge.target
            ))
        })?;

        edges.push(Edge {
            id: uuid::Uuid::new_v4().to_string(),
            source: source_id.clone(),
            target: target_id.clone(),
            kind: iedge.kind,
            provenance: infer_provenance(doc.kind),
            metadata: iedge.metadata.clone(),
        });
    }

    store.add_edges_batch(version, &edges)?;

    // Add constraints
    for ic in &doc.constraints {
        let constraint = Constraint {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ic.kind.clone(),
            name: ic.name.clone(),
            scope: ic.scope.clone(),
            target: ic.target.clone(),
            params: ic.params.clone(),
            message: ic.message.clone(),
            severity: ic.severity.unwrap_or(Severity::Error),
        };
        store.add_constraint(version, &constraint)?;
    }

    Ok(version)
}

/// Build an InterchangeDocument from store data for a given version.
fn build_export_document(store: &dyn GraphStore, version: Version) -> Result<InterchangeDocument> {
    // Find the snapshot for metadata
    let snapshots = store.list_snapshots()?;
    let snapshot = snapshots
        .iter()
        .find(|s| s.version == version)
        .ok_or(StoreError::VersionNotFound(version))?;

    let nodes = store.get_all_nodes(version)?;
    let edges = store.get_all_edges(version, None)?;
    let constraints = store.get_constraints(version)?;

    // Build ID->path mapping for edge resolution
    let id_to_path: HashMap<String, String> = nodes
        .iter()
        .map(|n| (n.id.clone(), n.canonical_path.clone()))
        .collect();

    let interchange_nodes: Vec<InterchangeNode> = nodes
        .iter()
        .map(|n| InterchangeNode {
            canonical_path: n.canonical_path.clone(),
            kind: n.kind,
            name: Some(n.name.clone()),
            sub_kind: Some(n.sub_kind.clone()),
            qualified_name: n.qualified_name.clone(),
            language: n.language.clone(),
            provenance: Some(n.provenance),
            source_ref: n.source_ref.clone(),
            metadata: n.metadata.clone(),
            children: None,
        })
        .collect();

    let interchange_edges: Vec<InterchangeEdge> = edges
        .iter()
        .filter_map(|e| {
            let source = id_to_path.get(&e.source)?;
            let target = id_to_path.get(&e.target)?;
            Some(InterchangeEdge {
                source: source.clone(),
                target: target.clone(),
                kind: e.kind,
                metadata: e.metadata.clone(),
            })
        })
        .collect();

    let interchange_constraints: Vec<InterchangeConstraint> = constraints
        .iter()
        .map(|c| InterchangeConstraint {
            name: c.name.clone(),
            kind: c.kind.clone(),
            scope: c.scope.clone(),
            target: c.target.clone(),
            params: c.params.clone(),
            message: c.message.clone(),
            severity: Some(c.severity),
        })
        .collect();

    Ok(InterchangeDocument {
        format: "svt/v1".to_string(),
        kind: snapshot.kind,
        version: Some(version),
        metadata: snapshot.metadata.clone(),
        nodes: interchange_nodes,
        edges: interchange_edges,
        constraints: interchange_constraints,
    })
}

/// Export a version from the store as YAML (flat format).
pub fn export_yaml(store: &dyn GraphStore, version: Version) -> Result<String> {
    let doc = build_export_document(store, version)?;
    serde_yaml::to_string(&doc).map_err(|e| StoreError::Internal(e.to_string()))
}

/// Export a version from the store as JSON (flat format).
pub fn export_json(store: &dyn GraphStore, version: Version) -> Result<String> {
    let doc = build_export_document(store, version)?;
    serde_json::to_string_pretty(&doc).map_err(|e| StoreError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interchange::parse_yaml;
    use crate::store::CozoStore;

    #[test]
    fn load_flat_document_creates_snapshot_and_nodes() {
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
  - source: /app
    target: /app/api
    kind: contains
constraints:
  - name: no-outward
    kind: must_not_depend
    scope: /app/api/**
    target: /app/**
    message: "API must not depend outward"
    severity: error
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let nodes = store.get_all_nodes(version).unwrap();
        assert_eq!(nodes.len(), 2);

        let edges = store.get_all_edges(version, None).unwrap();
        assert_eq!(edges.len(), 1);

        let constraints = store.get_constraints(version).unwrap();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].kind, "must_not_depend");
    }

    #[test]
    fn load_infers_name_from_path() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app/my-service
    kind: service
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let node = store
            .get_node_by_path(version, "/app/my-service")
            .unwrap()
            .unwrap();
        assert_eq!(node.name, "my-service");
    }

    #[test]
    fn load_infers_provenance_from_document_kind() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let node = store.get_node_by_path(version, "/app").unwrap().unwrap();
        assert_eq!(node.provenance, Provenance::Design);
    }

    #[test]
    fn load_nested_generates_contains_edges_with_resolved_ids() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let nodes = store.get_all_nodes(version).unwrap();
        assert_eq!(nodes.len(), 2);

        let edges = store
            .get_all_edges(version, Some(EdgeKind::Contains))
            .unwrap();
        assert_eq!(edges.len(), 1);

        // Edge should reference node UUIDs, not canonical paths
        let parent = store.get_node_by_path(version, "/app").unwrap().unwrap();
        let child = store
            .get_node_by_path(version, "/app/core")
            .unwrap()
            .unwrap();
        assert_eq!(edges[0].source, parent.id);
        assert_eq!(edges[0].target, child.id);
    }

    #[test]
    fn export_yaml_round_trips() {
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
  - source: /app
    target: /app/api
    kind: contains
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let exported = export_yaml(&store, version).unwrap();
        let re_parsed = parse_yaml(&exported).unwrap();
        assert_eq!(re_parsed.nodes.len(), 2);
        assert_eq!(re_parsed.edges.len(), 1);
    }

    #[test]
    fn export_json_produces_valid_json() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
edges: []
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let json_str = export_json(&store, version).unwrap();
        let re_parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(re_parsed["format"], "svt/v1");
    }
}
