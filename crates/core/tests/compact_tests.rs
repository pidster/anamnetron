mod helpers;

use proptest::prelude::*;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn compact_preserves_kept_version_and_removes_other() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);

    let v1 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();
    store
        .add_edge(v1, &helpers::make_edge_default("e1", "n1", "n1"))
        .unwrap();
    store
        .add_constraint(v1, &helpers::make_constraint("c1"))
        .unwrap();

    let v2 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();
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
    store.compact(DEFAULT_PROJECT_ID, &[v2]).unwrap();

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
    let snapshots = store.list_snapshots(DEFAULT_PROJECT_ID).unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].version, v2);
}

#[test]
fn compact_with_empty_keep_removes_all() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .add_node(v, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();

    store.compact(DEFAULT_PROJECT_ID, &[]).unwrap();

    let snapshots = store.list_snapshots(DEFAULT_PROJECT_ID).unwrap();
    assert!(snapshots.is_empty());
    assert!(store.get_node(v, &"n1".to_string()).unwrap().is_none());
}

#[test]
fn compact_preserves_multiple_kept_versions() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v1 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();

    let v2 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();
    store
        .add_node(v2, &helpers::make_node_default("n2", "/svc/b"))
        .unwrap();

    let v3 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .add_node(v3, &helpers::make_node_default("n3", "/svc/c"))
        .unwrap();

    // Keep v1 and v3, remove v2
    store.compact(DEFAULT_PROJECT_ID, &[v1, v3]).unwrap();

    assert!(store.get_node(v1, &"n1".to_string()).unwrap().is_some());
    assert!(store.get_node(v2, &"n2".to_string()).unwrap().is_none());
    assert!(store.get_node(v3, &"n3".to_string()).unwrap().is_some());

    let snapshots = store.list_snapshots(DEFAULT_PROJECT_ID).unwrap();
    assert_eq!(snapshots.len(), 2);
}

#[test]
fn compact_removes_edges_constraints_and_file_manifests_for_deleted_versions() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);

    let v1 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n2", "/svc/b"))
        .unwrap();
    store
        .add_edge(v1, &helpers::make_edge_default("e1", "n1", "n2"))
        .unwrap();
    store
        .add_constraint(v1, &helpers::make_constraint("c1"))
        .unwrap();
    store
        .add_file_manifest(
            v1,
            &[svt_core::model::FileManifestEntry {
                path: "src/a.rs".to_string(),
                hash: "a".repeat(64),
                unit_name: "a".to_string(),
                language: "rust".to_string(),
            }],
        )
        .unwrap();

    let v2 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();
    store
        .add_node(v2, &helpers::make_node_default("n3", "/svc/c"))
        .unwrap();
    store
        .add_node(v2, &helpers::make_node_default("n4", "/svc/d"))
        .unwrap();
    store
        .add_edge(v2, &helpers::make_edge_default("e2", "n3", "n4"))
        .unwrap();
    store
        .add_constraint(v2, &helpers::make_constraint("c2"))
        .unwrap();
    store
        .add_file_manifest(
            v2,
            &[svt_core::model::FileManifestEntry {
                path: "src/c.rs".to_string(),
                hash: "c".repeat(64),
                unit_name: "c".to_string(),
                language: "rust".to_string(),
            }],
        )
        .unwrap();

    // Keep only v2, removing v1
    store.compact(DEFAULT_PROJECT_ID, &[v2]).unwrap();

    // v1 edges gone
    assert!(store.get_all_edges(v1, None).unwrap().is_empty());
    // v1 constraints gone
    assert!(store.get_constraints(v1).unwrap().is_empty());
    // v1 file manifest gone
    assert!(store.get_file_manifest(v1).unwrap().is_empty());

    // v2 data intact
    assert_eq!(store.get_all_edges(v2, None).unwrap().len(), 1);
    assert_eq!(store.get_constraints(v2).unwrap().len(), 1);
    assert_eq!(store.get_file_manifest(v2).unwrap().len(), 1);
}

#[test]
fn compact_with_mixed_design_and_analysis_versions() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);

    // Create multiple design versions
    let d1 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .add_node(d1, &helpers::make_node_default("d1n", "/design/v1"))
        .unwrap();

    let d2 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .add_node(d2, &helpers::make_node_default("d2n", "/design/v2"))
        .unwrap();

    // Create multiple analysis versions
    let a1 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, Some("abc"))
        .unwrap();
    store
        .add_node(a1, &helpers::make_node_default("a1n", "/analysis/v1"))
        .unwrap();

    let a2 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, Some("def"))
        .unwrap();
    store
        .add_node(a2, &helpers::make_node_default("a2n", "/analysis/v2"))
        .unwrap();

    // Keep latest of each kind: d2 and a2
    store.compact(DEFAULT_PROJECT_ID, &[d2, a2]).unwrap();

    // Old versions removed
    assert!(store.get_node(d1, &"d1n".to_string()).unwrap().is_none());
    assert!(store.get_node(a1, &"a1n".to_string()).unwrap().is_none());

    // Kept versions intact
    assert!(store.get_node(d2, &"d2n".to_string()).unwrap().is_some());
    assert!(store.get_node(a2, &"a2n".to_string()).unwrap().is_some());

    let snapshots = store.list_snapshots(DEFAULT_PROJECT_ID).unwrap();
    assert_eq!(snapshots.len(), 2);
    let kinds: Vec<SnapshotKind> = snapshots.iter().map(|s| s.kind).collect();
    assert!(kinds.contains(&SnapshotKind::Design));
    assert!(kinds.contains(&SnapshotKind::Analysis));
}

proptest! {
    #[test]
    fn compact_preserves_kept_versions_and_removes_the_rest(total in 2usize..5) {
        let mut store = CozoStore::new_in_memory().unwrap();
        helpers::ensure_default_project(&mut store);

        // Create `total` versions, each with a node
        let mut versions = Vec::new();
        for i in 0..total {
            let v = store.create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None).unwrap();
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

        store.compact(DEFAULT_PROJECT_ID, &keep).unwrap();

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
        let snapshots = store.list_snapshots(DEFAULT_PROJECT_ID).unwrap();
        prop_assert_eq!(snapshots.len(), keep.len());
    }
}
