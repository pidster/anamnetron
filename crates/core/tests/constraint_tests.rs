use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

fn make_constraint(id: &str, kind: &str, scope: &str, target: Option<&str>) -> Constraint {
    Constraint {
        id: id.to_string(),
        kind: kind.to_string(),
        name: format!("{kind}-{id}"),
        scope: scope.to_string(),
        target: target.map(|t| t.to_string()),
        params: None,
        message: format!("Constraint {id} violated"),
        severity: Severity::Error,
    }
}

#[test]
fn add_constraint_then_get_constraints_round_trips() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let c = make_constraint("c1", "must_not_depend", "/payments/**", Some("/user/**"));
    store.add_constraint(v, &c).unwrap();

    let constraints = store.get_constraints(v).unwrap();
    assert_eq!(constraints.len(), 1);
    assert_eq!(constraints[0].id, "c1");
    assert_eq!(constraints[0].kind, "must_not_depend");
    assert_eq!(constraints[0].scope, "/payments/**");
    assert_eq!(constraints[0].target.as_deref(), Some("/user/**"));
    assert_eq!(constraints[0].severity, Severity::Error);
}

#[test]
fn multiple_constraints_per_version_all_returned() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store
        .add_constraint(
            v,
            &make_constraint("c1", "must_not_depend", "/a/**", Some("/b/**")),
        )
        .unwrap();
    store
        .add_constraint(v, &make_constraint("c2", "boundary", "/c/**", None))
        .unwrap();
    store
        .add_constraint(
            v,
            &make_constraint("c3", "must_not_depend", "/d/**", Some("/e/**")),
        )
        .unwrap();

    let constraints = store.get_constraints(v).unwrap();
    assert_eq!(constraints.len(), 3);
}

#[test]
fn get_constraints_for_empty_version_returns_empty() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let constraints = store.get_constraints(v).unwrap();
    assert!(constraints.is_empty());
}

#[test]
fn optional_fields_survive_as_none() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let c = Constraint {
        id: "c1".to_string(),
        kind: "boundary".to_string(),
        name: "no-cross-boundary".to_string(),
        scope: "/internal/**".to_string(),
        target: None,
        params: None,
        message: "Boundary violation".to_string(),
        severity: Severity::Warning,
    };
    store.add_constraint(v, &c).unwrap();

    let constraints = store.get_constraints(v).unwrap();
    assert_eq!(constraints.len(), 1);
    assert_eq!(constraints[0].target, None);
    assert_eq!(constraints[0].params, None);
    assert_eq!(constraints[0].severity, Severity::Warning);
}

#[test]
fn constraints_are_version_scoped() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v1 = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let v2 = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    store
        .add_constraint(
            v1,
            &make_constraint("c1", "must_not_depend", "/a/**", Some("/b/**")),
        )
        .unwrap();
    store
        .add_constraint(v2, &make_constraint("c2", "boundary", "/c/**", None))
        .unwrap();

    let v1_constraints = store.get_constraints(v1).unwrap();
    assert_eq!(v1_constraints.len(), 1);
    assert_eq!(v1_constraints[0].id, "c1");

    let v2_constraints = store.get_constraints(v2).unwrap();
    assert_eq!(v2_constraints.len(), 1);
    assert_eq!(v2_constraints[0].id, "c2");
}
