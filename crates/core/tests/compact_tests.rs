mod helpers;

use proptest::prelude::*;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn compact_preserves_kept_version_and_removes_other() {
    let mut store = CozoStore::new_in_memory().unwrap();

    let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();
    store
        .add_edge(v1, &helpers::make_edge_default("e1", "n1", "n1"))
        .unwrap();
    store
        .add_constraint(v1, &helpers::make_constraint("c1"))
        .unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store
        .add_node(v2, &helpers::make_node_default("n2", "/svc/b"))
        .unwrap();
    store
        .add_edge(v2, &helpers::make_edge_default("e2", "n2", "n2"))
        .unwrap();
    store
        .add_constraint(v2, &helpers::make_constraint("c2"))
        .unwrap();

    // Keep only v2
    store.compact(&[v2]).unwrap();

    // v2 data should still exist
    assert!(store.get_node(v2, &"n2".to_string()).unwrap().is_some());
    assert_eq!(
        store
            .get_edges(v2, &"n2".to_string(), Direction::Outgoing, None)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(store.get_constraints(v2).unwrap().len(), 1);

    // v1 data should be gone
    assert!(store.get_node(v1, &"n1".to_string()).unwrap().is_none());
    assert!(store
        .get_edges(v1, &"n1".to_string(), Direction::Outgoing, None)
        .unwrap()
        .is_empty());
    assert!(store.get_constraints(v1).unwrap().is_empty());

    // v1 snapshot should be gone
    let snapshots = store.list_snapshots().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].version, v2);
}

#[test]
fn compact_with_empty_keep_removes_all() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_node(v, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();

    store.compact(&[]).unwrap();

    let snapshots = store.list_snapshots().unwrap();
    assert!(snapshots.is_empty());
    assert!(store.get_node(v, &"n1".to_string()).unwrap().is_none());
}

#[test]
fn compact_preserves_multiple_kept_versions() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store
        .add_node(v2, &helpers::make_node_default("n2", "/svc/b"))
        .unwrap();

    let v3 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_node(v3, &helpers::make_node_default("n3", "/svc/c"))
        .unwrap();

    // Keep v1 and v3, remove v2
    store.compact(&[v1, v3]).unwrap();

    assert!(store.get_node(v1, &"n1".to_string()).unwrap().is_some());
    assert!(store.get_node(v2, &"n2".to_string()).unwrap().is_none());
    assert!(store.get_node(v3, &"n3".to_string()).unwrap().is_some());

    let snapshots = store.list_snapshots().unwrap();
    assert_eq!(snapshots.len(), 2);
}

proptest! {
    #[test]
    fn compact_preserves_kept_versions_and_removes_the_rest(total in 2usize..5) {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Create `total` versions, each with a node
        let mut versions = Vec::new();
        for i in 0..total {
            let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
            store.add_node(v, &helpers::make_node_default(&format!("n{i}"), &format!("/svc/v{i}"))).unwrap();
            versions.push(v);
        }

        // Use a deterministic keep mask based on the version index (keep even-indexed versions)
        // This avoids nested strategies while still exercising different subsets.
        let keep: Vec<Version> = versions.iter().enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, &v)| v)
            .collect();
        let remove: Vec<(usize, Version)> = versions.iter().enumerate()
            .filter(|(i, _)| i % 2 != 0)
            .map(|(i, &v)| (i, v))
            .collect();

        store.compact(&keep).unwrap();

        // Kept versions should still have their data
        for (idx, &v) in keep.iter().enumerate() {
            let original_idx = idx * 2; // even indices
            let result = store.get_node(v, &format!("n{original_idx}").to_string()).unwrap();
            prop_assert!(result.is_some(), "kept version {} should still have node n{}", v, original_idx);
        }

        // Removed versions should have no data
        for (original_idx, v) in &remove {
            let result = store.get_node(*v, &format!("n{original_idx}").to_string()).unwrap();
            prop_assert!(result.is_none(), "removed version {} should not have node n{}", v, original_idx);
        }

        // Snapshot count should match kept versions
        let snapshots = store.list_snapshots().unwrap();
        prop_assert_eq!(snapshots.len(), keep.len());
    }
}
