//! Integration tests for M21: Analysis Depth improvements.
//!
//! These tests verify crate-level dependency edges, Self:: resolution,
//! and local variable type inference against the real codebase.

use std::collections::HashMap;
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

/// Build a map from node ID to canonical path for edge resolution.
fn node_path_map(store: &CozoStore, version: Version) -> HashMap<NodeId, String> {
    store
        .get_all_nodes(version)
        .unwrap()
        .into_iter()
        .map(|n| (n.id, n.canonical_path))
        .collect()
}

#[test]
fn crate_level_dependencies_exist() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root(), None).unwrap();

    let paths = node_path_map(&store, summary.version);
    let edges = store.get_all_edges(summary.version, None).unwrap();
    let depends: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Depends)
        .collect();

    // svt-analyzer depends on svt-core — should have a Depends edge
    // from /svt/analyzer to /svt/core at the crate level.
    let analyzer_to_core = depends.iter().any(|e| {
        let source = paths.get(&e.source).map(|s| s.as_str()).unwrap_or("");
        let target = paths.get(&e.target).map(|s| s.as_str()).unwrap_or("");
        source == "/svt/analyzer" && target == "/svt/core"
    });
    assert!(
        analyzer_to_core,
        "should have Depends edge from /svt/analyzer to /svt/core, \
         crate-level depends edges: {:?}",
        depends
            .iter()
            .filter_map(|e| {
                let s = paths.get(&e.source)?;
                let t = paths.get(&e.target)?;
                if s.starts_with("/svt/") && !s.contains("::") {
                    Some(format!("{s} -> {t}"))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn method_call_resolution_improved() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root(), None).unwrap();

    // With heuristic resolution, some method calls should be resolved.
    assert!(
        summary.method_calls_resolved > 0,
        "should resolve some method calls, got resolved={}, unresolved={}",
        summary.method_calls_resolved,
        summary.method_calls_unresolved,
    );

    // There should still be unresolved calls (chained, trait objects, etc.)
    assert!(
        summary.method_calls_unresolved > 0,
        "should still have some unresolved method calls"
    );

    // Dogfood ratchet: resolution rate must not regress below the honest
    // baseline. After the R0/R1 fixes (self-resolved method calls are now
    // counted, and method-call edges are attributed to the calling function),
    // the measured rate is ~11.72% (962 of 8210 on this repo). The floor is set
    // just below that; do not lower it without a corresponding honest change.
    let total = summary.method_calls_resolved + summary.method_calls_unresolved;
    let resolution_pct = (summary.method_calls_resolved as f64 / total as f64) * 100.0;
    assert!(
        resolution_pct > 11.5,
        "resolution rate regressed below the honest baseline (>11.5%), \
         got {resolution_pct:.2}% ({} of {total})",
        summary.method_calls_resolved,
    );
}

#[test]
fn summary_carries_per_shape_method_call_breakdown() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root(), None).unwrap();

    let s = &summary.method_call_stats;

    // The per-shape breakdown surfaced on the summary (and printed by the CLI)
    // must be internally consistent with the headline resolved/unresolved fields.
    assert_eq!(
        s.resolved(),
        summary.method_calls_resolved,
        "per-shape resolved buckets must sum to the headline resolved count"
    );
    assert_eq!(
        s.unresolved(),
        summary.method_calls_unresolved,
        "per-shape unresolved buckets must sum to the headline unresolved count"
    );
    assert_eq!(
        s.total(),
        summary.method_calls_resolved + summary.method_calls_unresolved,
        "partition invariant must hold on the summary breakdown"
    );

    // On the real codebase every bucket that R0 exists to measure is populated —
    // this is the distribution that drives R2/R3 prioritization.
    assert!(
        s.self_resolved > 0,
        "self-resolved calls should be counted (got {s:?})"
    );
    assert!(
        s.local_var_resolved > 0,
        "local-var-resolved calls should be counted (got {s:?})"
    );
    assert!(
        s.unresolved_chained > 0,
        "chained calls (the R3 target bucket) should be counted (got {s:?})"
    );
}

#[test]
fn self_type_calls_resolved_in_real_code() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root(), None).unwrap();

    let paths = node_path_map(&store, summary.version);
    let edges = store.get_all_edges(summary.version, None).unwrap();
    let calls: Vec<_> = edges.iter().filter(|e| e.kind == EdgeKind::Calls).collect();

    // The codebase uses Self::new() and Type::new() patterns.
    // After M21, these should produce Calls edges.
    assert!(
        !calls.is_empty(),
        "should have Calls edges from resolved function/method calls"
    );

    // Verify at least some calls target qualified names with :: separators
    // (indicating resolved scoped calls, not just bare function names).
    let scoped_calls = calls
        .iter()
        .filter(|c| {
            paths
                .get(&c.target)
                .map(|p| p.matches('/').count() >= 3)
                .unwrap_or(false)
        })
        .count();
    assert!(
        scoped_calls > 0,
        "should have Calls edges to deeply scoped targets (resolved Type::method patterns)"
    );
}
