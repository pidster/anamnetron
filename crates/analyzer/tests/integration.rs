//! End-to-end integration tests for the analyzer pipeline.
//!
//! These tests exercise the full analysis pipeline on this workspace:
//! discover crates via cargo metadata, parse with tree-sitter, map to
//! canonical paths, and insert into an in-memory graph store.

use std::path::PathBuf;

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

use svt_analyzer::analyze_project;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn full_pipeline_produces_nodes_and_edges() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    assert!(
        summary.nodes_created > 10,
        "should create many nodes, got {}",
        summary.nodes_created
    );
    assert!(
        summary.edges_created > 5,
        "should create edges, got {}",
        summary.edges_created
    );

    // Verify nodes are actually in the store.
    // The store deduplicates nodes by (id, version) so the stored count may be
    // less than `nodes_created` when multiple items share the same canonical path.
    let nodes = store.get_all_nodes(summary.version).unwrap();
    assert!(
        nodes.len() <= summary.nodes_created,
        "store should have at most as many nodes as were created"
    );
    assert!(
        nodes.len() > 10,
        "store should have many nodes, got {}",
        nodes.len()
    );

    // All nodes should have Analysis provenance and a known language
    for node in &nodes {
        assert_eq!(node.provenance, Provenance::Analysis);
        assert!(
            node.language == Some("rust".to_string())
                || node.language == Some("typescript".to_string())
                || node.language == Some("svelte".to_string()),
            "unexpected language: {:?} for node {}",
            node.language,
            node.canonical_path
        );
    }
}

#[test]
fn analysis_snapshot_is_queryable() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    // Should be able to find svt-core by canonical path (workspace-aware: /svt/core)
    let core_node = store
        .get_node_by_path(summary.version, "/svt/core")
        .unwrap();
    assert!(
        core_node.is_some(),
        "should find svt-core at canonical path /svt/core"
    );

    let core = core_node.unwrap();
    assert_eq!(core.kind, NodeKind::Service);
    assert_eq!(core.sub_kind, "crate");
}

#[test]
fn analysis_edges_have_correct_provenance() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    let edges = store.get_all_edges(summary.version, None).unwrap();
    for edge in &edges {
        assert_eq!(edge.provenance, Provenance::Analysis);
    }
}

#[test]
fn contains_edges_form_hierarchy() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    let edges = store.get_all_edges(summary.version, None).unwrap();
    let contains: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Contains)
        .collect();
    assert!(
        !contains.is_empty(),
        "should have Contains edges for module hierarchy"
    );
}

#[test]
fn warnings_collected_not_dropped() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    // Warnings list should be accessible (may or may not be empty depending on code)
    println!("Analysis produced {} warnings", summary.warnings.len());
    // Just verify the field is populated and accessible
    for w in &summary.warnings {
        assert!(
            !w.message.is_empty(),
            "warning messages should not be empty"
        );
    }
}

#[test]
fn multiple_crates_all_represented() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    let nodes = store.get_all_nodes(summary.version).unwrap();
    let crate_nodes: Vec<_> = nodes.iter().filter(|n| n.sub_kind == "crate").collect();

    // Should find at least svt-core, svt-analyzer, svt-cli, svt-server
    assert!(
        crate_nodes.len() >= 4,
        "should have at least 4 crate nodes, got {}",
        crate_nodes.len()
    );

    let paths: Vec<&str> = crate_nodes
        .iter()
        .map(|n| n.canonical_path.as_str())
        .collect();
    assert!(paths.contains(&"/svt/core"), "should have /svt/core");
    assert!(
        paths.contains(&"/svt/analyzer"),
        "should have /svt/analyzer"
    );
}
