//! Tests for `GraphStore::store_info()`.

mod helpers;

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn store_info_empty_store_returns_zero_snapshots() {
    let store = CozoStore::new_in_memory().unwrap();
    let info = store.store_info().unwrap();

    assert_eq!(info.schema_version, 1);
    assert_eq!(info.snapshot_count, 0);
    assert!(info.snapshots.is_empty());
}

#[test]
fn store_info_includes_node_and_edge_counts() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    store
        .add_node(v, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();
    store
        .add_node(v, &helpers::make_node_default("n2", "/svc/b"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_edge_default("e1", "n1", "n2"))
        .unwrap();

    let info = store.store_info().unwrap();
    assert_eq!(info.snapshot_count, 1);
    assert_eq!(info.snapshots.len(), 1);

    let snap = &info.snapshots[0];
    assert_eq!(snap.version, v);
    assert_eq!(snap.kind, SnapshotKind::Design);
    assert_eq!(snap.node_count, 2);
    assert_eq!(snap.edge_count, 1);
}

#[test]
fn store_info_with_multiple_versions_reports_each() {
    let mut store = CozoStore::new_in_memory().unwrap();

    let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();

    let v2 = store
        .create_snapshot(SnapshotKind::Analysis, Some("abc123"))
        .unwrap();
    store
        .add_node(v2, &helpers::make_node_default("n2", "/svc/b"))
        .unwrap();
    store
        .add_node(v2, &helpers::make_node_default("n3", "/svc/c"))
        .unwrap();
    store
        .add_edge(v2, &helpers::make_edge_default("e1", "n2", "n3"))
        .unwrap();

    let info = store.store_info().unwrap();
    assert_eq!(info.snapshot_count, 2);
    assert_eq!(info.snapshots.len(), 2);

    // v1: 1 node, 0 edges
    assert_eq!(info.snapshots[0].version, v1);
    assert_eq!(info.snapshots[0].node_count, 1);
    assert_eq!(info.snapshots[0].edge_count, 0);

    // v2: 2 nodes, 1 edge
    assert_eq!(info.snapshots[1].version, v2);
    assert_eq!(info.snapshots[1].kind, SnapshotKind::Analysis);
    assert_eq!(info.snapshots[1].commit_ref, Some("abc123".to_string()));
    assert_eq!(info.snapshots[1].node_count, 2);
    assert_eq!(info.snapshots[1].edge_count, 1);
}
