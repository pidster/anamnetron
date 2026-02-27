mod helpers;

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};
use svt_core::validation;

#[test]
fn clean_graph_passes_both_validations() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();

    store
        .add_node(
            v,
            &helpers::make_node_with_kind("a", "/svc/a", NodeKind::Service),
        )
        .unwrap();
    store
        .add_node(
            v,
            &helpers::make_node_with_kind("b", "/svc/a/b", NodeKind::Component),
        )
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c1", "a", "b"))
        .unwrap();

    let cycles = validation::validate_contains_acyclic(&store, v).unwrap();
    assert!(cycles.is_empty(), "no cycles in a clean tree");

    let integrity = validation::validate_referential_integrity(&store, v).unwrap();
    assert!(integrity.is_empty(), "no integrity errors in a clean graph");
}

#[test]
fn contains_cycle_is_detected() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();

    store
        .add_node(
            v,
            &helpers::make_node_with_kind("a", "/svc/a", NodeKind::Component),
        )
        .unwrap();
    store
        .add_node(
            v,
            &helpers::make_node_with_kind("b", "/svc/b", NodeKind::Component),
        )
        .unwrap();

    // A contains B, B contains A -> cycle
    store
        .add_edge(v, &helpers::make_contains("c1", "a", "b"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c2", "b", "a"))
        .unwrap();

    let cycles = validation::validate_contains_acyclic(&store, v).unwrap();
    assert!(!cycles.is_empty(), "should detect the A<->B contains cycle");
}

#[test]
fn self_referencing_contains_edge_is_detected() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();

    store
        .add_node(
            v,
            &helpers::make_node_with_kind("a", "/svc/a", NodeKind::Component),
        )
        .unwrap();

    // A contains A -> self-reference cycle
    store
        .add_edge(v, &helpers::make_contains("c1", "a", "a"))
        .unwrap();

    let cycles = validation::validate_contains_acyclic(&store, v).unwrap();
    assert!(
        !cycles.is_empty(),
        "should detect self-referencing contains edge"
    );
}

#[test]
fn edge_referencing_nonexistent_node_is_flagged() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();

    store
        .add_node(
            v,
            &helpers::make_node_with_kind("a", "/svc/a", NodeKind::Component),
        )
        .unwrap();

    // Edge references "missing" node which doesn't exist
    let edge = Edge {
        id: "e1".to_string(),
        source: "a".to_string(),
        target: "missing".to_string(),
        kind: EdgeKind::Depends,
        provenance: Provenance::Design,
        metadata: None,
    };
    store.add_edge(v, &edge).unwrap();

    let errors = validation::validate_referential_integrity(&store, v).unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].edge_id, "e1");
    assert_eq!(errors[0].missing_node_id, "missing");
}

#[test]
fn edge_with_missing_source_is_flagged() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();

    store
        .add_node(
            v,
            &helpers::make_node_with_kind("b", "/svc/b", NodeKind::Component),
        )
        .unwrap();

    // Edge source "missing" doesn't exist
    let edge = Edge {
        id: "e1".to_string(),
        source: "missing".to_string(),
        target: "b".to_string(),
        kind: EdgeKind::Depends,
        provenance: Provenance::Design,
        metadata: None,
    };
    store.add_edge(v, &edge).unwrap();

    let errors = validation::validate_referential_integrity(&store, v).unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].missing_node_id, "missing");
}
