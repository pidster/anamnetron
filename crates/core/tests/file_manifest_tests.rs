//! Tests for file manifest storage and node/edge copy methods.

mod helpers;

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn file_manifest_relation_exists_in_new_store() {
    // init_schema creates the file_manifest relation. Verify we can write to and read from it.
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    let entries = vec![FileManifestEntry {
        path: "src/lib.rs".to_string(),
        hash: "a".repeat(64),
        unit_name: "my-crate".to_string(),
        language: "rust".to_string(),
    }];
    store.add_file_manifest(v, &entries).unwrap();
    let loaded = store.get_file_manifest(v).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].path, "src/lib.rs");
}

#[test]
fn add_and_get_file_manifest_round_trips() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let entries = vec![
        FileManifestEntry {
            path: "src/main.rs".to_string(),
            hash: "a1".repeat(32),
            unit_name: "my-app".to_string(),
            language: "rust".to_string(),
        },
        FileManifestEntry {
            path: "src/lib.rs".to_string(),
            hash: "b2".repeat(32),
            unit_name: "my-lib".to_string(),
            language: "rust".to_string(),
        },
        FileManifestEntry {
            path: "web/src/App.tsx".to_string(),
            hash: "c3".repeat(32),
            unit_name: "web".to_string(),
            language: "typescript".to_string(),
        },
    ];

    store.add_file_manifest(v, &entries).unwrap();
    let loaded = store.get_file_manifest(v).unwrap();

    assert_eq!(loaded.len(), 3);

    // Sort for deterministic comparison
    let mut loaded_paths: Vec<&str> = loaded.iter().map(|e| e.path.as_str()).collect();
    loaded_paths.sort();
    assert_eq!(
        loaded_paths,
        vec!["src/lib.rs", "src/main.rs", "web/src/App.tsx"]
    );

    // Verify all fields
    let app_entry = loaded.iter().find(|e| e.path == "src/main.rs").unwrap();
    assert_eq!(app_entry.hash, "a1".repeat(32));
    assert_eq!(app_entry.unit_name, "my-app");
    assert_eq!(app_entry.language, "rust");

    let ts_entry = loaded.iter().find(|e| e.path == "web/src/App.tsx").unwrap();
    assert_eq!(ts_entry.language, "typescript");
    assert_eq!(ts_entry.unit_name, "web");
}

#[test]
fn get_file_manifest_empty_version_returns_empty() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    // Don't add any manifest entries
    let loaded = store.get_file_manifest(v).unwrap();
    assert!(loaded.is_empty());
}

#[test]
fn copy_nodes_duplicates_all_to_new_version() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    // Add several nodes to v1
    store
        .add_node(v1, &helpers::make_node_default("n1", "/svc/a"))
        .unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n2", "/svc/b"))
        .unwrap();
    store
        .add_node(v1, &helpers::make_node_default("n3", "/svc/c"))
        .unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let copied = store.copy_nodes(v1, v2).unwrap();
    assert_eq!(copied, 3);

    // All nodes should exist in v2
    assert!(store.get_node(v2, &"n1".to_string()).unwrap().is_some());
    assert!(store.get_node(v2, &"n2".to_string()).unwrap().is_some());
    assert!(store.get_node(v2, &"n3".to_string()).unwrap().is_some());

    // Original nodes should still exist in v1
    assert!(store.get_node(v1, &"n1".to_string()).unwrap().is_some());
}

#[test]
fn copy_edges_duplicates_all_to_new_version() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

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
        .add_edge(
            v1,
            &helpers::make_edge("e2", "n1", "n2", EdgeKind::Contains),
        )
        .unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let copied = store.copy_edges(v1, v2).unwrap();
    assert_eq!(copied, 2);

    // Edges should exist in v2
    let v2_edges = store.get_all_edges(v2, None).unwrap();
    assert_eq!(v2_edges.len(), 2);

    // Original edges still exist in v1
    let v1_edges = store.get_all_edges(v1, None).unwrap();
    assert_eq!(v1_edges.len(), 2);
}

#[test]
fn copy_preserves_all_node_fields() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let node = Node {
        id: "full-node".to_string(),
        canonical_path: "/my-service/module/MyStruct".to_string(),
        qualified_name: Some("my_service::module::MyStruct".to_string()),
        kind: NodeKind::Unit,
        sub_kind: "struct".to_string(),
        name: "MyStruct".to_string(),
        language: Some("rust".to_string()),
        provenance: Provenance::Analysis,
        source_ref: Some("src/module.rs:42".to_string()),
        metadata: Some(serde_json::json!({"visibility": "pub", "derives": ["Debug", "Clone"]})),
    };

    store.add_node(v1, &node).unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store.copy_nodes(v1, v2).unwrap();

    let copied_node = store
        .get_node(v2, &"full-node".to_string())
        .unwrap()
        .expect("copied node should exist");

    assert_eq!(copied_node.id, node.id);
    assert_eq!(copied_node.canonical_path, node.canonical_path);
    assert_eq!(copied_node.qualified_name, node.qualified_name);
    assert_eq!(copied_node.kind, node.kind);
    assert_eq!(copied_node.sub_kind, node.sub_kind);
    assert_eq!(copied_node.name, node.name);
    assert_eq!(copied_node.language, node.language);
    assert_eq!(copied_node.provenance, node.provenance);
    assert_eq!(copied_node.source_ref, node.source_ref);
    assert_eq!(copied_node.metadata, node.metadata);
}

#[test]
fn compact_deletes_file_manifest_entries() {
    let mut store = CozoStore::new_in_memory().unwrap();

    let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store
        .add_file_manifest(
            v1,
            &[FileManifestEntry {
                path: "src/lib.rs".to_string(),
                hash: "a".repeat(64),
                unit_name: "my-crate".to_string(),
                language: "rust".to_string(),
            }],
        )
        .unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store
        .add_file_manifest(
            v2,
            &[FileManifestEntry {
                path: "src/lib.rs".to_string(),
                hash: "b".repeat(64),
                unit_name: "my-crate".to_string(),
                language: "rust".to_string(),
            }],
        )
        .unwrap();

    // Compact keeping only v2
    store.compact(&[v2]).unwrap();

    // v1 manifest should be gone
    assert!(store.get_file_manifest(v1).unwrap().is_empty());

    // v2 manifest should remain
    let v2_manifest = store.get_file_manifest(v2).unwrap();
    assert_eq!(v2_manifest.len(), 1);
    assert_eq!(v2_manifest[0].hash, "b".repeat(64));
}

#[test]
fn copy_nodes_returns_zero_for_empty_version() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    // No nodes in v1
    let copied = store.copy_nodes(v1, v2).unwrap();
    assert_eq!(copied, 0);
}

#[test]
fn copy_edges_returns_zero_for_empty_version() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    // No edges in v1
    let copied = store.copy_edges(v1, v2).unwrap();
    assert_eq!(copied, 0);
}

#[test]
fn add_file_manifest_with_empty_entries_is_noop() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    // Empty entries should be a no-op
    store.add_file_manifest(v, &[]).unwrap();
    assert!(store.get_file_manifest(v).unwrap().is_empty());
}
