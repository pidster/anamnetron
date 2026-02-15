mod helpers;

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

fn setup_two_nodes() -> (CozoStore, Version) {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_node(
            v,
            &helpers::make_node("a", "/svc/a", NodeKind::Component, "module"),
        )
        .unwrap();
    store
        .add_node(
            v,
            &helpers::make_node("b", "/svc/b", NodeKind::Component, "module"),
        )
        .unwrap();
    (store, v)
}

#[test]
fn add_edge_then_get_outgoing_returns_it() {
    let (mut store, v) = setup_two_nodes();
    let edge = helpers::make_edge("e1", "a", "b", EdgeKind::Depends);
    store.add_edge(v, &edge).unwrap();

    let edges = store
        .get_edges(v, &"a".to_string(), Direction::Outgoing, None)
        .unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].id, "e1");
    assert_eq!(edges[0].source, "a");
    assert_eq!(edges[0].target, "b");
    assert_eq!(edges[0].kind, EdgeKind::Depends);
}

#[test]
fn add_edge_then_get_incoming_returns_it() {
    let (mut store, v) = setup_two_nodes();
    let edge = helpers::make_edge("e1", "a", "b", EdgeKind::Depends);
    store.add_edge(v, &edge).unwrap();

    let edges = store
        .get_edges(v, &"b".to_string(), Direction::Incoming, None)
        .unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].id, "e1");
}

#[test]
fn direction_both_returns_edges_in_either_direction() {
    let (mut store, v) = setup_two_nodes();
    store
        .add_node(
            v,
            &helpers::make_node("c", "/svc/c", NodeKind::Component, "module"),
        )
        .unwrap();

    store
        .add_edge(v, &helpers::make_edge("e1", "a", "b", EdgeKind::Depends))
        .unwrap();
    store
        .add_edge(v, &helpers::make_edge("e2", "c", "b", EdgeKind::Depends))
        .unwrap();

    let edges = store
        .get_edges(v, &"b".to_string(), Direction::Both, None)
        .unwrap();
    assert_eq!(edges.len(), 2, "expected 2 edges for Direction::Both");
}

#[test]
fn filter_by_edge_kind_returns_only_matching() {
    let (mut store, v) = setup_two_nodes();
    store
        .add_edge(v, &helpers::make_edge("e1", "a", "b", EdgeKind::Depends))
        .unwrap();
    store
        .add_edge(v, &helpers::make_edge("e2", "a", "b", EdgeKind::Calls))
        .unwrap();

    let depends_edges = store
        .get_edges(
            v,
            &"a".to_string(),
            Direction::Outgoing,
            Some(EdgeKind::Depends),
        )
        .unwrap();
    assert_eq!(depends_edges.len(), 1);
    assert_eq!(depends_edges[0].kind, EdgeKind::Depends);

    let calls_edges = store
        .get_edges(
            v,
            &"a".to_string(),
            Direction::Outgoing,
            Some(EdgeKind::Calls),
        )
        .unwrap();
    assert_eq!(calls_edges.len(), 1);
    assert_eq!(calls_edges[0].kind, EdgeKind::Calls);
}

#[test]
fn add_edges_batch_then_all_retrievable() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    // Create a chain of nodes
    for i in 0..10 {
        store
            .add_node(
                v,
                &helpers::make_node(
                    &format!("n{i}"),
                    &format!("/svc/n{i}"),
                    NodeKind::Component,
                    "module",
                ),
            )
            .unwrap();
    }

    // Create edges between consecutive nodes
    let edges: Vec<Edge> = (0..9)
        .map(|i| {
            helpers::make_edge(
                &format!("e{i}"),
                &format!("n{i}"),
                &format!("n{}", i + 1),
                EdgeKind::Depends,
            )
        })
        .collect();
    store.add_edges_batch(v, &edges).unwrap();

    // Check each node has the right outgoing edge
    for i in 0..9 {
        let out = store
            .get_edges(v, &format!("n{i}"), Direction::Outgoing, None)
            .unwrap();
        assert_eq!(out.len(), 1, "node n{i} should have 1 outgoing edge");
        assert_eq!(out[0].target, format!("n{}", i + 1));
    }
}

#[test]
fn edge_metadata_survives_round_trip() {
    let (mut store, v) = setup_two_nodes();
    let edge = Edge {
        id: "e1".to_string(),
        source: "a".to_string(),
        target: "b".to_string(),
        kind: EdgeKind::Depends,
        provenance: Provenance::Analysis,
        metadata: Some(serde_json::json!({"weight": 42})),
    };
    store.add_edge(v, &edge).unwrap();

    let edges = store
        .get_edges(v, &"a".to_string(), Direction::Outgoing, None)
        .unwrap();
    assert_eq!(edges.len(), 1);
    assert!(edges[0].metadata.is_some());
    assert_eq!(edges[0].provenance, Provenance::Analysis);
}
