use proptest::prelude::*;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

fn make_node(id: &str, path: &str, kind: NodeKind) -> Node {
    Node {
        id: id.to_string(),
        canonical_path: path.to_string(),
        qualified_name: None,
        kind,
        sub_kind: "module".to_string(),
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        language: None,
        provenance: Provenance::Design,
        source_ref: None,
        metadata: None,
    }
}

fn make_depends(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        kind: EdgeKind::Depends,
        provenance: Provenance::Design,
        metadata: None,
    }
}

/// Setup a chain: A -> B -> C
fn setup_chain() -> (CozoStore, Version) {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_node(v, &make_node("a", "/svc/a", NodeKind::Component))
        .unwrap();
    store
        .add_node(v, &make_node("b", "/svc/b", NodeKind::Component))
        .unwrap();
    store
        .add_node(v, &make_node("c", "/svc/c", NodeKind::Component))
        .unwrap();

    store.add_edge(v, &make_depends("e1", "a", "b")).unwrap();
    store.add_edge(v, &make_depends("e2", "b", "c")).unwrap();

    (store, v)
}

#[test]
fn direct_dependencies_returns_immediate_targets() {
    let (store, v) = setup_chain();
    let deps = store
        .query_dependencies(v, &"a".to_string(), false)
        .unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].id, "b");
}

#[test]
fn transitive_dependencies_returns_full_chain() {
    let (store, v) = setup_chain();
    let deps = store.query_dependencies(v, &"a".to_string(), true).unwrap();
    let ids: Vec<&str> = deps.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(deps.len(), 2);
    assert!(ids.contains(&"b"));
    assert!(ids.contains(&"c"));
}

#[test]
fn diamond_dependency_returns_each_node_once() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    // Diamond: A -> B, A -> C, B -> D, C -> D
    store
        .add_node(v, &make_node("a", "/svc/a", NodeKind::Component))
        .unwrap();
    store
        .add_node(v, &make_node("b", "/svc/b", NodeKind::Component))
        .unwrap();
    store
        .add_node(v, &make_node("c", "/svc/c", NodeKind::Component))
        .unwrap();
    store
        .add_node(v, &make_node("d", "/svc/d", NodeKind::Component))
        .unwrap();

    store.add_edge(v, &make_depends("e1", "a", "b")).unwrap();
    store.add_edge(v, &make_depends("e2", "a", "c")).unwrap();
    store.add_edge(v, &make_depends("e3", "b", "d")).unwrap();
    store.add_edge(v, &make_depends("e4", "c", "d")).unwrap();

    let deps = store.query_dependencies(v, &"a".to_string(), true).unwrap();
    let ids: Vec<&str> = deps.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(deps.len(), 3, "A transitively depends on B, C, D");
    assert!(ids.contains(&"b"));
    assert!(ids.contains(&"c"));
    assert!(ids.contains(&"d"));
}

#[test]
fn query_dependents_is_the_reverse() {
    let (store, v) = setup_chain();
    // C is depended on by B (directly) and A (transitively)
    let direct = store.query_dependents(v, &"c".to_string(), false).unwrap();
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].id, "b");

    let transitive = store.query_dependents(v, &"c".to_string(), true).unwrap();
    let ids: Vec<&str> = transitive.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(transitive.len(), 2);
    assert!(ids.contains(&"a"));
    assert!(ids.contains(&"b"));
}

#[test]
fn node_with_no_dependencies_returns_empty() {
    let (store, v) = setup_chain();
    let deps = store
        .query_dependencies(v, &"c".to_string(), false)
        .unwrap();
    assert!(deps.is_empty());

    let deps_transitive = store.query_dependencies(v, &"c".to_string(), true).unwrap();
    assert!(deps_transitive.is_empty());
}

#[test]
fn direct_dependencies_are_subset_of_transitive() {
    let (store, v) = setup_chain();
    let direct = store
        .query_dependencies(v, &"a".to_string(), false)
        .unwrap();
    let transitive = store.query_dependencies(v, &"a".to_string(), true).unwrap();
    let transitive_ids: Vec<&str> = transitive.iter().map(|n| n.id.as_str()).collect();
    for node in &direct {
        assert!(
            transitive_ids.contains(&node.id.as_str()),
            "direct dep {} not in transitive",
            node.id
        );
    }
}

proptest! {
    #[test]
    fn direct_deps_are_subset_of_transitive_deps(node_count in 3usize..7, edges in proptest::collection::vec((1usize..6, 1usize..6), 1..10)) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        // Create nodes
        for i in 0..node_count {
            store.add_node(v, &make_node(
                &format!("n{i}"),
                &format!("/svc/n{i}"),
                NodeKind::Component,
            )).unwrap();
        }

        // Add random Depends edges (DAG-ish: only add i -> j where i < j to avoid cycles in test setup)
        let mut edge_idx = 0;
        for (src, tgt) in &edges {
            let src = src % node_count;
            let tgt = tgt % node_count;
            if src < tgt {
                let _ = store.add_edge(v, &make_depends(&format!("e{edge_idx}"), &format!("n{src}"), &format!("n{tgt}")));
                edge_idx += 1;
            }
        }

        // For every node, verify direct deps are a subset of transitive deps
        for i in 0..node_count {
            let node_id = format!("n{i}");
            let direct = store.query_dependencies(v, &node_id, false).unwrap();
            let transitive = store.query_dependencies(v, &node_id, true).unwrap();
            let transitive_ids: std::collections::HashSet<&str> = transitive.iter().map(|n| n.id.as_str()).collect();
            for dep in &direct {
                prop_assert!(
                    transitive_ids.contains(dep.id.as_str()),
                    "direct dep {} of {} not found in transitive deps",
                    dep.id,
                    node_id
                );
            }
        }
    }
}
