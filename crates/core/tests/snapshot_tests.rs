mod helpers;

use svt_core::model::{Project, SnapshotKind, DEFAULT_PROJECT_ID};
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn create_snapshot_returns_version_one() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let version = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();
    assert_eq!(version, 1);
}

#[test]
fn create_second_snapshot_returns_version_two() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();
    let v2 = store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();
    assert_eq!(v2, 2);
}

#[test]
fn latest_version_returns_none_for_empty_store() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    let latest = store
        .latest_version(DEFAULT_PROJECT_ID, SnapshotKind::Analysis)
        .unwrap();
    assert_eq!(latest, None);
}

#[test]
fn latest_version_filters_by_kind() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();
    store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();
    store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, None)
        .unwrap();

    assert_eq!(
        store
            .latest_version(DEFAULT_PROJECT_ID, SnapshotKind::Design)
            .unwrap(),
        Some(3)
    );
    assert_eq!(
        store
            .latest_version(DEFAULT_PROJECT_ID, SnapshotKind::Analysis)
            .unwrap(),
        Some(2)
    );
    assert_eq!(
        store
            .latest_version(DEFAULT_PROJECT_ID, SnapshotKind::Import)
            .unwrap(),
        None
    );
}

#[test]
fn list_snapshots_returns_all_in_version_order() {
    let mut store = CozoStore::new_in_memory().unwrap();
    helpers::ensure_default_project(&mut store);
    store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Design, Some("abc123"))
        .unwrap();
    store
        .create_snapshot(DEFAULT_PROJECT_ID, SnapshotKind::Analysis, None)
        .unwrap();

    let snapshots = store.list_snapshots(DEFAULT_PROJECT_ID).unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].version, 1);
    assert_eq!(snapshots[0].kind, SnapshotKind::Design);
    assert_eq!(snapshots[0].commit_ref.as_deref(), Some("abc123"));
    assert_eq!(snapshots[1].version, 2);
    assert_eq!(snapshots[1].kind, SnapshotKind::Analysis);
    assert_eq!(snapshots[1].commit_ref, None);
}

/// Helper to create a named project in the store.
fn create_project(store: &mut CozoStore, id: &str, name: &str) {
    store
        .create_project(&Project {
            id: id.to_string(),
            name: name.to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            description: None,
            metadata: None,
        })
        .unwrap();
}

#[test]
fn multi_project_snapshots_get_independent_version_numbers() {
    let mut store = CozoStore::new_in_memory().unwrap();
    create_project(&mut store, "project-a", "Project A");
    create_project(&mut store, "project-b", "Project B");

    let v_a = store
        .create_snapshot("project-a", SnapshotKind::Analysis, None)
        .unwrap();
    let v_b = store
        .create_snapshot("project-b", SnapshotKind::Analysis, None)
        .unwrap();

    assert_eq!(v_a, 1, "first snapshot in project-a should be version 1");
    assert_eq!(
        v_b, 1,
        "first snapshot in project-b should be version 1, not influenced by project-a"
    );
}

#[test]
fn sequential_snapshots_in_same_project_increment_correctly() {
    let mut store = CozoStore::new_in_memory().unwrap();
    create_project(&mut store, "project-seq", "Sequential Project");

    let v1 = store
        .create_snapshot("project-seq", SnapshotKind::Analysis, None)
        .unwrap();
    let v2 = store
        .create_snapshot("project-seq", SnapshotKind::Design, None)
        .unwrap();
    let v3 = store
        .create_snapshot("project-seq", SnapshotKind::Analysis, None)
        .unwrap();

    assert_eq!(v1, 1, "first snapshot should be version 1");
    assert_eq!(v2, 2, "second snapshot should be version 2");
    assert_eq!(v3, 3, "third snapshot should be version 3");
}

/// Interleaved snapshots across projects should each maintain their own version
/// sequence independently. The `snapshot_projects` table uses a composite key
/// `{ version: Int, project_id: String }` so each project has independent numbering.
#[test]
fn interleaved_multi_project_snapshots_maintain_correct_versions() {
    let mut store = CozoStore::new_in_memory().unwrap();
    create_project(&mut store, "project-x", "Project X");
    create_project(&mut store, "project-y", "Project Y");

    // Interleave: X v1, Y v1, X v2, Y v2
    let x1 = store
        .create_snapshot("project-x", SnapshotKind::Analysis, None)
        .unwrap();
    let y1 = store
        .create_snapshot("project-y", SnapshotKind::Analysis, None)
        .unwrap();
    let x2 = store
        .create_snapshot("project-x", SnapshotKind::Design, None)
        .unwrap();
    let y2 = store
        .create_snapshot("project-y", SnapshotKind::Design, None)
        .unwrap();

    assert_eq!(x1, 1, "project-x first snapshot should be version 1");
    assert_eq!(y1, 1, "project-y first snapshot should be version 1");
    assert_eq!(x2, 2, "project-x second snapshot should be version 2");
    assert_eq!(y2, 2, "project-y second snapshot should be version 2");

    // Verify list_snapshots returns correct per-project views
    let x_snapshots = store.list_snapshots("project-x").unwrap();
    let y_snapshots = store.list_snapshots("project-y").unwrap();

    assert_eq!(
        x_snapshots.len(),
        2,
        "project-x should have exactly 2 snapshots"
    );
    assert_eq!(
        y_snapshots.len(),
        2,
        "project-y should have exactly 2 snapshots"
    );

    assert_eq!(x_snapshots[0].version, 1);
    assert_eq!(x_snapshots[1].version, 2);
    assert_eq!(y_snapshots[0].version, 1);
    assert_eq!(y_snapshots[1].version, 2);
}
