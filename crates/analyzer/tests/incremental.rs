//! End-to-end integration tests for incremental analysis.

use std::path::PathBuf;

use svt_core::model::DEFAULT_PROJECT_ID;
use svt_core::store::{CozoStore, GraphStore};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn incremental_and_full_produce_same_node_set() {
    let root = workspace_root();
    let mut store = CozoStore::new_in_memory().unwrap();

    // First run: incremental with no previous (= full analysis, stores manifest)
    let first = svt_analyzer::analyze_project_incremental(
        &mut store,
        DEFAULT_PROJECT_ID,
        &root,
        None,
        None,
    )
    .unwrap();

    // Second run: incremental with previous (nothing changed, copies all data)
    let second = svt_analyzer::analyze_project_incremental(
        &mut store,
        DEFAULT_PROJECT_ID,
        &root,
        None,
        Some(first.version),
    )
    .unwrap();

    // Compare node canonical paths between versions
    let v1_nodes = store.get_all_nodes(first.version).unwrap();
    let v2_nodes = store.get_all_nodes(second.version).unwrap();

    let mut v1_paths: Vec<&str> = v1_nodes.iter().map(|n| n.canonical_path.as_str()).collect();
    let mut v2_paths: Vec<&str> = v2_nodes.iter().map(|n| n.canonical_path.as_str()).collect();
    v1_paths.sort();
    v2_paths.sort();

    assert_eq!(
        v1_paths, v2_paths,
        "incremental run should produce the same node set as full analysis"
    );

    // Edge counts should match too
    let v1_edges = store.get_all_edges(first.version, None).unwrap();
    let v2_edges = store.get_all_edges(second.version, None).unwrap();
    assert_eq!(
        v1_edges.len(),
        v2_edges.len(),
        "incremental run should produce the same edge count as full analysis"
    );
}

#[test]
fn incremental_without_previous_does_full_analysis() {
    let root = workspace_root();
    let mut store = CozoStore::new_in_memory().unwrap();

    let summary = svt_analyzer::analyze_project_incremental(
        &mut store,
        DEFAULT_PROJECT_ID,
        &root,
        None,
        None,
    )
    .unwrap();

    assert!(!summary.incremental, "first run should not be incremental");
    assert!(summary.nodes_created > 0);
    assert!(summary.edges_created > 0);
    assert_eq!(summary.nodes_copied, 0);
    assert_eq!(summary.edges_copied, 0);
    assert_eq!(summary.units_skipped, 0);
}

#[test]
fn file_manifest_stored_and_retrievable() {
    let root = workspace_root();
    let mut store = CozoStore::new_in_memory().unwrap();

    let summary = svt_analyzer::analyze_project_incremental(
        &mut store,
        DEFAULT_PROJECT_ID,
        &root,
        None,
        None,
    )
    .unwrap();

    let manifest = store.get_file_manifest(summary.version).unwrap();
    assert!(!manifest.is_empty(), "manifest should have entries");
    assert!(
        manifest.iter().any(|e| e.language == "rust"),
        "manifest should contain rust entries"
    );
    assert!(
        manifest.iter().all(|e| e.hash.len() == 64),
        "all hashes should be 64-char hex"
    );
    assert!(
        manifest.iter().all(|e| !e.unit_name.is_empty()),
        "all entries should have a unit name"
    );
}

#[test]
fn incremental_summary_has_correct_stats() {
    let root = workspace_root();
    let mut store = CozoStore::new_in_memory().unwrap();

    // First run: stores manifest
    let first = svt_analyzer::analyze_project_incremental(
        &mut store,
        DEFAULT_PROJECT_ID,
        &root,
        None,
        None,
    )
    .unwrap();

    // Second run: nothing changed
    let second = svt_analyzer::analyze_project_incremental(
        &mut store,
        DEFAULT_PROJECT_ID,
        &root,
        None,
        Some(first.version),
    )
    .unwrap();

    assert!(second.incremental, "should be incremental");
    assert!(
        second.units_skipped > 0,
        "should skip units when nothing changed"
    );
    assert_eq!(
        second.units_reanalyzed, 0,
        "should not re-analyze any units when nothing changed"
    );
    assert!(second.nodes_copied > 0, "should copy nodes from previous");
    assert!(second.edges_copied > 0, "should copy edges from previous");
}
