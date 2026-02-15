use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

fn make_node(id: &str, path: &str) -> Node {
    Node {
        id: id.to_string(),
        canonical_path: path.to_string(),
        qualified_name: None,
        kind: NodeKind::Component,
        sub_kind: "module".to_string(),
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        language: None,
        provenance: Provenance::Design,
        source_ref: None,
        metadata: None,
    }
}

fn make_edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        kind: EdgeKind::Depends,
        provenance: Provenance::Design,
        metadata: None,
    }
}

fn make_constraint(id: &str) -> Constraint {
    Constraint {
        id: id.to_string(),
        kind: "must_not_depend".to_string(),
        name: format!("constraint-{id}"),
        scope: "/a/**".to_string(),
        target: Some("/b/**".to_string()),
        params: None,
        message: "Violation".to_string(),
        severity: Severity::Error,
    }
}

#[test]
fn compact_preserves_kept_version_and_removes_other() {
    let mut store = CozoStore::new_in_memory().unwrap();

    let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store.add_node(v1, &make_node("n1", "/svc/a")).unwrap();
    store.add_edge(v1, &make_edge("e1", "n1", "n1")).unwrap();
    store.add_constraint(v1, &make_constraint("c1")).unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store.add_node(v2, &make_node("n2", "/svc/b")).unwrap();
    store.add_edge(v2, &make_edge("e2", "n2", "n2")).unwrap();
    store.add_constraint(v2, &make_constraint("c2")).unwrap();

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
    store.add_node(v, &make_node("n1", "/svc/a")).unwrap();

    store.compact(&[]).unwrap();

    let snapshots = store.list_snapshots().unwrap();
    assert!(snapshots.is_empty());
    assert!(store.get_node(v, &"n1".to_string()).unwrap().is_none());
}

#[test]
fn compact_preserves_multiple_kept_versions() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store.add_node(v1, &make_node("n1", "/svc/a")).unwrap();

    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store.add_node(v2, &make_node("n2", "/svc/b")).unwrap();

    let v3 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store.add_node(v3, &make_node("n3", "/svc/c")).unwrap();

    // Keep v1 and v3, remove v2
    store.compact(&[v1, v3]).unwrap();

    assert!(store.get_node(v1, &"n1".to_string()).unwrap().is_some());
    assert!(store.get_node(v2, &"n2".to_string()).unwrap().is_none());
    assert!(store.get_node(v3, &"n3".to_string()).unwrap().is_some());

    let snapshots = store.list_snapshots().unwrap();
    assert_eq!(snapshots.len(), 2);
}
