mod helpers;

use proptest::prelude::*;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

/// Build a 5-level hierarchy:
/// system -> service -> comp1 -> comp2 -> unit
fn setup_hierarchy() -> (CozoStore, Version) {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();

    store
        .add_node(
            v,
            &helpers::make_node("sys", "/myapp", NodeKind::System, "workspace"),
        )
        .unwrap();
    store
        .add_node(
            v,
            &helpers::make_node("svc", "/myapp/api", NodeKind::Service, "crate"),
        )
        .unwrap();
    store
        .add_node(
            v,
            &helpers::make_node(
                "comp1",
                "/myapp/api/handlers",
                NodeKind::Component,
                "module",
            ),
        )
        .unwrap();
    store
        .add_node(
            v,
            &helpers::make_node(
                "comp2",
                "/myapp/api/handlers/auth",
                NodeKind::Component,
                "module",
            ),
        )
        .unwrap();
    store
        .add_node(
            v,
            &helpers::make_node(
                "unit1",
                "/myapp/api/handlers/auth/login",
                NodeKind::Unit,
                "function",
            ),
        )
        .unwrap();

    // Contains edges forming the hierarchy
    store
        .add_edge(v, &helpers::make_contains("c1", "sys", "svc"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c2", "svc", "comp1"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c3", "comp1", "comp2"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c4", "comp2", "unit1"))
        .unwrap();

    (store, v)
}

#[test]
fn get_children_returns_direct_children() {
    let (store, v) = setup_hierarchy();
    let children = store.get_children(v, &"sys".to_string()).unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].id, "svc");
}

#[test]
fn get_children_of_leaf_returns_empty() {
    let (store, v) = setup_hierarchy();
    let children = store.get_children(v, &"unit1".to_string()).unwrap();
    assert!(children.is_empty());
}

#[test]
fn get_parent_returns_direct_parent() {
    let (store, v) = setup_hierarchy();
    let parent = store.get_parent(v, &"svc".to_string()).unwrap();
    assert!(parent.is_some());
    assert_eq!(parent.unwrap().id, "sys");
}

#[test]
fn get_parent_of_root_returns_none() {
    let (store, v) = setup_hierarchy();
    let parent = store.get_parent(v, &"sys".to_string()).unwrap();
    assert!(parent.is_none());
}

#[test]
fn query_ancestors_returns_full_path_to_root() {
    let (store, v) = setup_hierarchy();
    let ancestors = store.query_ancestors(v, &"unit1".to_string()).unwrap();
    let ancestor_ids: Vec<&str> = ancestors.iter().map(|n| n.id.as_str()).collect();
    // Should contain comp2, comp1, svc, sys (in some order)
    assert_eq!(ancestors.len(), 4);
    assert!(ancestor_ids.contains(&"comp2"));
    assert!(ancestor_ids.contains(&"comp1"));
    assert!(ancestor_ids.contains(&"svc"));
    assert!(ancestor_ids.contains(&"sys"));
}

#[test]
fn query_ancestors_of_root_returns_empty() {
    let (store, v) = setup_hierarchy();
    let ancestors = store.query_ancestors(v, &"sys".to_string()).unwrap();
    assert!(ancestors.is_empty());
}

#[test]
fn query_descendants_returns_entire_subtree() {
    let (store, v) = setup_hierarchy();
    let descendants = store
        .query_descendants(v, &"sys".to_string(), None)
        .unwrap();
    assert_eq!(
        descendants.len(),
        4,
        "system root should have 4 descendants"
    );
    let ids: Vec<&str> = descendants.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"svc"));
    assert!(ids.contains(&"comp1"));
    assert!(ids.contains(&"comp2"));
    assert!(ids.contains(&"unit1"));
}

#[test]
fn query_descendants_with_kind_filter() {
    let (store, v) = setup_hierarchy();
    let filter = NodeFilter {
        kind: Some(NodeKind::Unit),
        ..Default::default()
    };
    let descendants = store
        .query_descendants(v, &"sys".to_string(), Some(&filter))
        .unwrap();
    assert_eq!(descendants.len(), 1);
    assert_eq!(descendants[0].id, "unit1");
}

#[test]
fn query_descendants_of_leaf_returns_empty() {
    let (store, v) = setup_hierarchy();
    let descendants = store
        .query_descendants(v, &"unit1".to_string(), None)
        .unwrap();
    assert!(descendants.is_empty());
}

#[test]
fn five_level_hierarchy_ancestors_at_each_level() {
    let (store, v) = setup_hierarchy();

    // unit1 has 4 ancestors
    assert_eq!(
        store
            .query_ancestors(v, &"unit1".to_string())
            .unwrap()
            .len(),
        4
    );
    // comp2 has 3 ancestors
    assert_eq!(
        store
            .query_ancestors(v, &"comp2".to_string())
            .unwrap()
            .len(),
        3
    );
    // comp1 has 2 ancestors
    assert_eq!(
        store
            .query_ancestors(v, &"comp1".to_string())
            .unwrap()
            .len(),
        2
    );
    // svc has 1 ancestor
    assert_eq!(
        store.query_ancestors(v, &"svc".to_string()).unwrap().len(),
        1
    );
    // sys has 0 ancestors
    assert_eq!(
        store.query_ancestors(v, &"sys".to_string()).unwrap().len(),
        0
    );
}

#[test]
fn five_level_hierarchy_descendants_at_each_level() {
    let (store, v) = setup_hierarchy();

    // sys has 4 descendants
    assert_eq!(
        store
            .query_descendants(v, &"sys".to_string(), None)
            .unwrap()
            .len(),
        4
    );
    // svc has 3 descendants
    assert_eq!(
        store
            .query_descendants(v, &"svc".to_string(), None)
            .unwrap()
            .len(),
        3
    );
    // comp1 has 2 descendants
    assert_eq!(
        store
            .query_descendants(v, &"comp1".to_string(), None)
            .unwrap()
            .len(),
        2
    );
    // comp2 has 1 descendant
    assert_eq!(
        store
            .query_descendants(v, &"comp2".to_string(), None)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn query_descendants_with_sub_kind_filter_only() {
    let (store, v) = setup_hierarchy();
    let filter = NodeFilter {
        sub_kind: Some("function".to_string()),
        ..Default::default()
    };
    let descendants = store
        .query_descendants(v, &"sys".to_string(), Some(&filter))
        .unwrap();
    assert_eq!(descendants.len(), 1);
    assert_eq!(descendants[0].id, "unit1");
    assert_eq!(descendants[0].sub_kind, "function");
}

#[test]
fn query_descendants_with_language_filter_only() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();

    // Root node
    store
        .add_node(
            v,
            &helpers::make_node("root", "/app", NodeKind::System, "workspace"),
        )
        .unwrap();

    // Rust child
    let mut rust_node = helpers::make_node("rs1", "/app/rs1", NodeKind::Unit, "function");
    rust_node.language = Some("rust".to_string());
    store.add_node(v, &rust_node).unwrap();

    // Python child
    let mut py_node = helpers::make_node("py1", "/app/py1", NodeKind::Unit, "function");
    py_node.language = Some("python".to_string());
    store.add_node(v, &py_node).unwrap();

    store
        .add_edge(v, &helpers::make_contains("c1", "root", "rs1"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c2", "root", "py1"))
        .unwrap();

    let filter = NodeFilter {
        language: Some("rust".to_string()),
        ..Default::default()
    };
    let descendants = store
        .query_descendants(v, &"root".to_string(), Some(&filter))
        .unwrap();
    assert_eq!(descendants.len(), 1);
    assert_eq!(descendants[0].id, "rs1");
}

#[test]
fn query_descendants_with_all_filters_combined() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let v = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();

    store
        .add_node(
            v,
            &helpers::make_node("root", "/app", NodeKind::System, "workspace"),
        )
        .unwrap();

    // Node matching all filters
    let mut matching = helpers::make_node("match", "/app/match", NodeKind::Unit, "function");
    matching.language = Some("rust".to_string());
    store.add_node(v, &matching).unwrap();

    // Node matching kind only
    let mut wrong_sub = helpers::make_node("wrong_sub", "/app/wrong_sub", NodeKind::Unit, "struct");
    wrong_sub.language = Some("rust".to_string());
    store.add_node(v, &wrong_sub).unwrap();

    // Node matching kind+sub_kind but wrong language
    let mut wrong_lang =
        helpers::make_node("wrong_lang", "/app/wrong_lang", NodeKind::Unit, "function");
    wrong_lang.language = Some("python".to_string());
    store.add_node(v, &wrong_lang).unwrap();

    store
        .add_edge(v, &helpers::make_contains("c1", "root", "match"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c2", "root", "wrong_sub"))
        .unwrap();
    store
        .add_edge(v, &helpers::make_contains("c3", "root", "wrong_lang"))
        .unwrap();

    let filter = NodeFilter {
        kind: Some(NodeKind::Unit),
        sub_kind: Some("function".to_string()),
        language: Some("rust".to_string()),
    };
    let descendants = store
        .query_descendants(v, &"root".to_string(), Some(&filter))
        .unwrap();
    assert_eq!(descendants.len(), 1);
    assert_eq!(descendants[0].id, "match");
}

#[test]
fn query_descendants_with_filter_matching_nothing_returns_empty() {
    let (store, v) = setup_hierarchy();
    let filter = NodeFilter {
        kind: Some(NodeKind::System),
        sub_kind: Some("nonexistent".to_string()),
        ..Default::default()
    };
    let descendants = store
        .query_descendants(v, &"sys".to_string(), Some(&filter))
        .unwrap();
    assert!(
        descendants.is_empty(),
        "expected empty result for non-matching filter"
    );
}

proptest! {
    #[test]
    fn ancestor_chain_has_no_duplicates(depth in 2usize..8) {
        let mut store = CozoStore::new_in_memory().unwrap();
        helpers::ensure_default_project(&mut store);
        let v = store.create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None).unwrap();

        // Build a chain of `depth` nodes connected by Contains edges.
        let ids: Vec<String> = (0..depth).map(|i| format!("n{i}")).collect();
        let paths: Vec<String> = (0..depth).map(|i| {
            // Build nested paths like /a, /a/b, /a/b/c, ...
            let segments: String = (0..=i).map(|j| format!("/{}", (b'a' + j as u8) as char)).collect();
            segments
        }).collect();

        for i in 0..depth {
            store.add_node(v, &helpers::make_node(&ids[i], &paths[i], NodeKind::Component, "module")).unwrap();
        }
        for i in 0..depth - 1 {
            store.add_edge(v, &helpers::make_contains(&format!("c{i}"), &ids[i], &ids[i + 1])).unwrap();
        }

        // Query ancestors of the deepest node
        let deepest = &ids[depth - 1];
        let ancestors = store.query_ancestors(v, &deepest.to_string()).unwrap();
        let ancestor_ids: Vec<&str> = ancestors.iter().map(|n| n.id.as_str()).collect();

        // No duplicates
        let mut seen = std::collections::HashSet::new();
        for id in &ancestor_ids {
            prop_assert!(seen.insert(*id), "duplicate ancestor: {}", id);
        }

        // Should have depth - 1 ancestors (all nodes except itself)
        prop_assert_eq!(ancestors.len(), depth - 1);
    }
}
