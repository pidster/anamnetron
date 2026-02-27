mod helpers;

use svt_core::model::{SnapshotKind, DEFAULT_PROJECT_ID};
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
