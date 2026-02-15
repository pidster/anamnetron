mod helpers;

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};
use svt_core::validation;

#[test]
fn simple_service_containment_navigation() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    helpers::create_simple_service(&mut store, v);

    // Service has two children: handlers and models
    let children = store.get_children(v, &"svc".to_string()).unwrap();
    assert_eq!(children.len(), 2);
    let child_ids: Vec<&str> = children.iter().map(|n| n.id.as_str()).collect();
    assert!(child_ids.contains(&"handlers"));
    assert!(child_ids.contains(&"models"));

    // handlers has two children: create and delete
    let handler_children = store.get_children(v, &"handlers".to_string()).unwrap();
    assert_eq!(handler_children.len(), 2);

    // create's parent is handlers
    let parent = store.get_parent(v, &"create".to_string()).unwrap();
    assert_eq!(parent.unwrap().id, "handlers");

    // create's ancestors: handlers, svc
    let ancestors = store.query_ancestors(v, &"create".to_string()).unwrap();
    assert_eq!(ancestors.len(), 2);
}

#[test]
fn simple_service_dependency_query() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    helpers::create_simple_service(&mut store, v);

    // create depends on order
    let deps = store
        .query_dependencies(v, &"create".to_string(), false)
        .unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].id, "order");

    // order has create as a dependent
    let dependents = store
        .query_dependents(v, &"order".to_string(), false)
        .unwrap();
    assert_eq!(dependents.len(), 1);
    assert_eq!(dependents[0].id, "create");
}

#[test]
fn simple_service_descendants_with_filter() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    helpers::create_simple_service(&mut store, v);

    // All descendants of svc
    let all = store
        .query_descendants(v, &"svc".to_string(), None)
        .unwrap();
    assert_eq!(all.len(), 5, "svc has 5 descendants total");

    // Only Unit descendants
    let filter = NodeFilter {
        kind: Some(NodeKind::Unit),
        ..Default::default()
    };
    let units = store
        .query_descendants(v, &"svc".to_string(), Some(&filter))
        .unwrap();
    assert_eq!(units.len(), 3, "svc has 3 Unit descendants");
    let unit_ids: Vec<&str> = units.iter().map(|n| n.id.as_str()).collect();
    assert!(unit_ids.contains(&"create"));
    assert!(unit_ids.contains(&"delete"));
    assert!(unit_ids.contains(&"order"));
}

#[test]
fn layered_architecture_transitive_dependencies_follow_layer_order() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    helpers::create_layered_architecture(&mut store, v);

    // api transitively depends on service, repo, db
    let deps = store
        .query_dependencies(v, &"api".to_string(), true)
        .unwrap();
    assert_eq!(deps.len(), 3, "api transitively depends on 3 layers");
    let dep_ids: Vec<&str> = deps.iter().map(|n| n.id.as_str()).collect();
    assert!(dep_ids.contains(&"service"));
    assert!(dep_ids.contains(&"repo"));
    assert!(dep_ids.contains(&"db"));

    // service has 2 transitive dependencies
    let service_deps = store
        .query_dependencies(v, &"service".to_string(), true)
        .unwrap();
    assert_eq!(service_deps.len(), 2);

    // db has no dependencies
    let db_deps = store
        .query_dependencies(v, &"db".to_string(), false)
        .unwrap();
    assert!(db_deps.is_empty());
}

#[test]
fn layered_architecture_containment() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    helpers::create_layered_architecture(&mut store, v);

    // app has 4 children
    let children = store.get_children(v, &"app".to_string()).unwrap();
    assert_eq!(children.len(), 4);

    // All children are Components
    for child in &children {
        assert_eq!(child.kind, NodeKind::Component);
    }
}

#[test]
fn layered_architecture_passes_validation() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    helpers::create_layered_architecture(&mut store, v);

    let cycles = validation::validate_contains_acyclic(&store, v).unwrap();
    assert!(cycles.is_empty(), "layered architecture has no cycles");

    let errors = validation::validate_referential_integrity(&store, v).unwrap();
    assert!(
        errors.is_empty(),
        "layered architecture has no integrity errors"
    );
}

#[test]
fn design_and_analysis_snapshots_coexist() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let design_v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let analysis_v = store
        .create_snapshot(SnapshotKind::Analysis, Some("abc123"))
        .unwrap();

    helpers::create_simple_service(&mut store, design_v);
    helpers::create_layered_architecture(&mut store, analysis_v);

    // latest_version returns correct values
    assert_eq!(
        store.latest_version(SnapshotKind::Design).unwrap(),
        Some(design_v)
    );
    assert_eq!(
        store.latest_version(SnapshotKind::Analysis).unwrap(),
        Some(analysis_v)
    );

    // Data is version-scoped: design has svc, analysis has app
    assert!(store
        .get_node(design_v, &"svc".to_string())
        .unwrap()
        .is_some());
    assert!(store
        .get_node(design_v, &"app".to_string())
        .unwrap()
        .is_none());
    assert!(store
        .get_node(analysis_v, &"app".to_string())
        .unwrap()
        .is_some());
    assert!(store
        .get_node(analysis_v, &"svc".to_string())
        .unwrap()
        .is_none());
}
