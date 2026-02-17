//! Graph endpoint returning Cytoscape.js-compatible element format.

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use svt_core::model::{EdgeKind, Version};
use svt_core::store::GraphStore;

use crate::error::ApiError;
use crate::state::SharedState;

/// A Cytoscape.js node element.
#[derive(Debug, Serialize, Deserialize)]
pub struct CyNode {
    /// Node data.
    pub data: CyNodeData,
}

/// Data fields for a Cytoscape.js node.
#[derive(Debug, Serialize, Deserialize)]
pub struct CyNodeData {
    /// Node ID.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Node kind.
    pub kind: String,
    /// Sub-kind (e.g., "crate", "module").
    pub sub_kind: String,
    /// Canonical path.
    pub canonical_path: String,
    /// Parent node ID (for compound nodes). None for root nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Source language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Source reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

/// A Cytoscape.js edge element.
#[derive(Debug, Serialize, Deserialize)]
pub struct CyEdge {
    /// Edge data.
    pub data: CyEdgeData,
}

/// Data fields for a Cytoscape.js edge.
#[derive(Debug, Serialize, Deserialize)]
pub struct CyEdgeData {
    /// Edge ID.
    pub id: String,
    /// Source node ID.
    pub source: String,
    /// Target node ID.
    pub target: String,
    /// Edge kind.
    pub kind: String,
}

/// Full Cytoscape.js graph payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct CytoscapeGraph {
    /// Graph elements.
    pub elements: CytoscapeElements,
}

/// Nodes and edges in Cytoscape.js format.
#[derive(Debug, Serialize, Deserialize)]
pub struct CytoscapeElements {
    /// Node elements (with parent for compound nodes).
    pub nodes: Vec<CyNode>,
    /// Edge elements (non-containment only).
    pub edges: Vec<CyEdge>,
}

/// GET /api/snapshots/{version}/graph
pub async fn get_graph(
    State(state): State<SharedState>,
    Path(version): Path<Version>,
) -> Result<Json<CytoscapeGraph>, ApiError> {
    let all_nodes = state.store.get_all_nodes(version)?;
    let all_edges = state.store.get_all_edges(version, None)?;

    // Build parent map from Contains edges: child_id -> parent_id
    let mut parent_map: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for edge in &all_edges {
        if edge.kind == EdgeKind::Contains {
            parent_map.insert(&edge.target, &edge.source);
        }
    }

    let cy_nodes: Vec<CyNode> = all_nodes
        .iter()
        .map(|node| CyNode {
            data: CyNodeData {
                id: node.id.clone(),
                label: node.name.clone(),
                kind: serde_json::to_value(node.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default(),
                sub_kind: node.sub_kind.clone(),
                canonical_path: node.canonical_path.clone(),
                parent: parent_map.get(node.id.as_str()).map(|s| s.to_string()),
                language: node.language.clone(),
                source_ref: node.source_ref.clone(),
            },
        })
        .collect();

    // Only non-containment edges as Cytoscape edges
    let cy_edges: Vec<CyEdge> = all_edges
        .iter()
        .filter(|e| e.kind != EdgeKind::Contains)
        .map(|edge| CyEdge {
            data: CyEdgeData {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                kind: serde_json::to_value(edge.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default(),
            },
        })
        .collect();

    Ok(Json(CytoscapeGraph {
        elements: CytoscapeElements {
            nodes: cy_nodes,
            edges: cy_edges,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::{routing::get, Router};
    use http_body_util::BodyExt;
    use svt_core::model::{Edge, Node, NodeKind, Provenance, SnapshotKind};
    use svt_core::store::{CozoStore, GraphStore};
    use tower::ServiceExt;

    use crate::state::AppState;

    fn make_node(id: &str, path: &str, kind: NodeKind) -> Node {
        Node {
            id: id.to_string(),
            canonical_path: path.to_string(),
            qualified_name: None,
            kind,
            sub_kind: "test".to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            language: None,
            provenance: Provenance::Design,
            source_ref: None,
            metadata: None,
        }
    }

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/api/snapshots/{version}/graph", get(get_graph))
            .with_state(state)
    }

    #[tokio::test]
    async fn graph_contains_edges_become_parent_field() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(v, &make_node("n1", "/app", NodeKind::System))
            .unwrap();
        store
            .add_node(v, &make_node("n2", "/app/core", NodeKind::Service))
            .unwrap();

        // Contains edge: n1 contains n2
        store
            .add_edge(
                v,
                &Edge {
                    id: "e-c".to_string(),
                    source: "n1".to_string(),
                    target: "n2".to_string(),
                    kind: EdgeKind::Contains,
                    provenance: Provenance::Design,
                    metadata: None,
                },
            )
            .unwrap();
        // Depends edge: stays as edge
        store
            .add_edge(
                v,
                &Edge {
                    id: "e-d".to_string(),
                    source: "n2".to_string(),
                    target: "n1".to_string(),
                    kind: EdgeKind::Depends,
                    provenance: Provenance::Design,
                    metadata: None,
                },
            )
            .unwrap();

        let state = Arc::new(AppState {
            store,
            design_version: Some(v),
            analysis_version: None,
        });
        let app = test_app(state);
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/snapshots/1/graph")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let graph: CytoscapeGraph = serde_json::from_slice(&body).unwrap();

        // n2 should have parent = n1
        let n2 = graph
            .elements
            .nodes
            .iter()
            .find(|n| n.data.id == "n2")
            .unwrap();
        assert_eq!(n2.data.parent, Some("n1".to_string()));

        // n1 should have no parent
        let n1 = graph
            .elements
            .nodes
            .iter()
            .find(|n| n.data.id == "n1")
            .unwrap();
        assert!(n1.data.parent.is_none());

        // Only non-containment edges in edges list
        assert_eq!(graph.elements.edges.len(), 1);
        assert_eq!(graph.elements.edges[0].data.kind, "depends");
    }
}
