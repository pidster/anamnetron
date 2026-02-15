mod helpers;

use proptest::prelude::*;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn add_node_then_get_by_id_round_trips() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let node = helpers::make_node("n1", "/test-service", NodeKind::Service, "crate");
    store.add_node(v, &node).unwrap();

    let retrieved = store
        .get_node(v, &"n1".to_string())
        .unwrap()
        .expect("node should exist");
    assert_eq!(retrieved.id, "n1");
    assert_eq!(retrieved.canonical_path, "/test-service");
    assert_eq!(retrieved.kind, NodeKind::Service);
}

#[test]
fn add_node_then_get_by_path_round_trips() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let node = helpers::make_node("n1", "/test-service", NodeKind::Service, "crate");
    store.add_node(v, &node).unwrap();

    let retrieved = store
        .get_node_by_path(v, "/test-service")
        .unwrap()
        .expect("node should exist");
    assert_eq!(retrieved.id, "n1");
}

#[test]
fn get_nonexistent_node_returns_none() {
    let store = CozoStore::new_in_memory().unwrap();
    let result = store.get_node(1, &"missing".to_string()).unwrap();
    assert!(result.is_none());
}

#[test]
fn add_nodes_batch_then_retrieve_all() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let nodes: Vec<Node> = (0..100)
        .map(|i| {
            helpers::make_node(
                &format!("n{i}"),
                &format!("/svc/comp{i}"),
                NodeKind::Component,
                "module",
            )
        })
        .collect();
    store.add_nodes_batch(v, &nodes).unwrap();

    for i in 0..100 {
        let n = store.get_node_by_path(v, &format!("/svc/comp{i}")).unwrap();
        assert!(n.is_some(), "node /svc/comp{i} not found");
    }
}

#[test]
fn node_optional_fields_survive_round_trip() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let node = Node {
        id: "n1".to_string(),
        canonical_path: "/svc".to_string(),
        qualified_name: Some("my_svc".to_string()),
        kind: NodeKind::Service,
        sub_kind: "crate".to_string(),
        name: "svc".to_string(),
        language: Some("rust".to_string()),
        provenance: Provenance::Analysis,
        source_ref: Some("src/lib.rs:1".to_string()),
        metadata: Some(serde_json::json!({"wasm": true})),
    };
    store.add_node(v, &node).unwrap();

    let back = store.get_node(v, &"n1".to_string()).unwrap().unwrap();
    assert_eq!(back.qualified_name.as_deref(), Some("my_svc"));
    assert_eq!(back.language.as_deref(), Some("rust"));
    assert_eq!(back.source_ref.as_deref(), Some("src/lib.rs:1"));
    assert!(back.metadata.is_some());
}

#[test]
fn get_all_nodes_returns_all_nodes_for_version() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let n1 = helpers::make_node("n1", "/app", NodeKind::System, "workspace");
    let n2 = helpers::make_node("n2", "/app/api", NodeKind::Component, "module");
    store.add_node(v, &n1).unwrap();
    store.add_node(v, &n2).unwrap();

    let all = store.get_all_nodes(v).unwrap();
    assert_eq!(all.len(), 2);

    let paths: Vec<&str> = all.iter().map(|n| n.canonical_path.as_str()).collect();
    assert!(paths.contains(&"/app"));
    assert!(paths.contains(&"/app/api"));
}

proptest! {
    #[test]
    fn n_nodes_added_then_queried_returns_exactly_n(n in 1usize..50) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        for i in 0..n {
            let node = helpers::make_node(
                &format!("node{i}"),
                &format!("/svc/comp{i}"),
                NodeKind::Component,
                "module",
            );
            store.add_node(v, &node).unwrap();
        }

        // Verify each node can be found by path
        for i in 0..n {
            let path = format!("/svc/comp{i}");
            let result = store.get_node_by_path(v, &path).unwrap();
            prop_assert!(result.is_some(), "node at path {} not found", path);
            let node = result.unwrap();
            prop_assert_eq!(&node.id, &format!("node{i}"));
        }
    }
}
