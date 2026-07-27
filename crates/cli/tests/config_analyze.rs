//! Integration tests for project-config-driven analysis.
//!
//! Covers three behaviours that previously failed silently:
//! - every configured `sources` entry is analyzed (not just the first),
//! - `sources[].exclude` is honoured during analysis,
//! - an invalid `.svt/config.yaml` is rejected instead of ignored.
//!
//! Every test runs against its own temp directory via `--project-dir`, so no
//! test writes into the source tree.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn svt_in(dir: &Path) -> Command {
    #[allow(deprecated)]
    let mut cmd = Command::cargo_bin("svt").unwrap();
    cmd.arg("--project-dir").arg(dir);
    cmd
}

/// Export a snapshot version as Mermaid and return stdout.
///
/// Mermaid (not JSON) is used because the JSON exporter reads snapshots under
/// the hardcoded default project ID, so it cannot see snapshots created for a
/// configured project. That is a separate pre-existing defect.
fn export_graph(root: &Path, version: &str) -> String {
    let output = svt_in(root)
        .args(["export", "--format", "mermaid", "--version", version])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).unwrap()
}

/// Write `.svt/config.yaml` into a project directory.
fn write_config(dir: &Path, contents: &str) {
    let svt_dir = dir.join(".svt");
    fs::create_dir_all(&svt_dir).unwrap();
    fs::write(svt_dir.join("config.yaml"), contents).unwrap();
}

/// Create a minimal Rust crate at `dir/name` so the analyzer discovers a unit.
fn write_crate(root: &Path, relative: &str, crate_name: &str) {
    let dir = root.join(relative);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
    )
    .unwrap();
    fs::write(
        dir.join("src").join("lib.rs"),
        format!("pub fn {}_entry() {{}}\n", crate_name.replace('-', "_")),
    )
    .unwrap();
}

/// Create a Cargo workspace root so Rust discovery walks the whole tree.
fn write_workspace(root: &Path, members: &[&str]) {
    let members = members
        .iter()
        .map(|m| format!("\"{m}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join("Cargo.toml"),
        format!("[workspace]\nresolver = \"2\"\nmembers = [{members}]\n"),
    )
    .unwrap();
}

// --- Bug 1: multi-source analysis ---------------------------------------

#[test]
fn all_configured_sources_are_analyzed() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "alpha", "alpha-crate");
    write_crate(root, "beta", "beta-crate");
    write_config(
        root,
        "project: multi-source\nsources:\n  - path: alpha\n  - path: beta\n",
    );

    svt_in(root).arg("analyze").assert().success();

    // Both crates must appear in the single snapshot produced.
    let graph = export_graph(root, "1");
    assert!(
        graph.contains("alpha-crate"),
        "first source missing from snapshot"
    );
    assert!(
        graph.contains("beta-crate"),
        "second source was dropped -- multi-source regression"
    );
}

#[test]
fn multiple_sources_produce_a_single_snapshot() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "alpha", "alpha-crate");
    write_crate(root, "beta", "beta-crate");
    write_config(
        root,
        "project: multi-source\nsources:\n  - path: alpha\n  - path: beta\n",
    );

    svt_in(root)
        .arg("analyze")
        .assert()
        .success()
        // One snapshot, and it is the first version.
        .stdout(predicate::str::contains("Created analysis snapshot v1"))
        .stdout(predicate::str::contains("Analyzed 2 sources"));
}

#[test]
fn explicit_cli_path_overrides_configured_sources() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "alpha", "alpha-crate");
    write_crate(root, "beta", "beta-crate");
    write_config(
        root,
        "project: multi-source\nsources:\n  - path: alpha\n  - path: beta\n",
    );

    svt_in(root)
        .arg("analyze")
        .arg(root.join("alpha"))
        .assert()
        .success();

    let graph = export_graph(root, "1");
    assert!(graph.contains("alpha-crate"), "CLI path was not analyzed");
    assert!(
        !graph.contains("beta-crate"),
        "CLI path should override config sources"
    );
}

#[test]
fn multiple_sources_are_analyzed_incrementally() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "alpha", "alpha-crate");
    write_crate(root, "beta", "beta-crate");
    write_config(
        root,
        "project: multi-source\nsources:\n  - path: alpha\n  - path: beta\n",
    );

    svt_in(root)
        .args(["analyze", "--incremental"])
        .assert()
        .success();
    svt_in(root)
        .args(["analyze", "--incremental"])
        .assert()
        .success();

    // The second (incremental) run produced v2; both sources must survive it.
    let graph = export_graph(root, "2");
    assert!(graph.contains("alpha-crate"));
    assert!(
        graph.contains("beta-crate"),
        "incremental analysis dropped the second source"
    );
}

// --- Bug 2: sources[].exclude -------------------------------------------

#[test]
fn excluded_directories_are_omitted_from_analysis() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "kept", "kept-crate");
    write_crate(root, "vendor/skipped", "vendored-crate");
    write_workspace(root, &["kept", "vendor/skipped"]);
    write_config(
        root,
        "project: excludes\nsources:\n  - path: .\n    exclude:\n      - vendor\n",
    );

    svt_in(root).arg("analyze").assert().success();

    let graph = export_graph(root, "1");
    assert!(graph.contains("kept-crate"), "non-excluded crate missing");
    assert!(
        !graph.contains("vendored-crate"),
        "excluded directory was analyzed anyway"
    );
}

#[test]
fn exclude_with_trailing_slash_is_honored() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "kept", "kept-crate");
    write_crate(root, "third_party/dep", "third-party-crate");
    write_workspace(root, &["kept", "third_party/dep"]);
    write_config(
        root,
        "project: excludes\nsources:\n  - path: .\n    exclude:\n      - third_party/\n",
    );

    svt_in(root).arg("analyze").assert().success();

    let graph = export_graph(root, "1");
    assert!(graph.contains("kept-crate"));
    assert!(!graph.contains("third-party-crate"));
}

#[test]
fn empty_exclude_list_analyzes_everything() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "kept", "kept-crate");
    write_crate(root, "vendor/dep", "vendored-crate");
    write_workspace(root, &["kept", "vendor/dep"]);
    write_config(root, "project: no-excludes\nsources:\n  - path: .\n");

    svt_in(root).arg("analyze").assert().success();

    let graph = export_graph(root, "1");
    assert!(graph.contains("kept-crate"));
    assert!(
        graph.contains("vendored-crate"),
        "nothing should be excluded when exclude is empty"
    );
}

#[test]
fn excludes_are_honored_across_incremental_runs() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "kept", "kept-crate");
    write_crate(root, "vendor/skipped", "vendored-crate");
    write_workspace(root, &["kept", "vendor/skipped"]);
    write_config(
        root,
        "project: excludes\nsources:\n  - path: .\n    exclude:\n      - vendor\n",
    );

    svt_in(root)
        .args(["analyze", "--incremental"])
        .assert()
        .success();
    svt_in(root)
        .args(["analyze", "--incremental"])
        .assert()
        .success();

    // Excluded files must never enter the manifest, so they cannot reappear
    // (or churn) on a later incremental run.
    let graph = export_graph(root, "2");
    assert!(graph.contains("kept-crate"));
    assert!(
        !graph.contains("vendored-crate"),
        "excluded directory leaked into the incremental snapshot"
    );
}

// --- Bug 3: config validation is enforced -------------------------------

#[test]
fn invalid_project_id_in_config_is_rejected() {
    let tmp = TempDir::new().unwrap();
    write_config(tmp.path(), "project: INVALID ID\n");

    svt_in(tmp.path())
        .arg("analyze")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid config"))
        .stderr(predicate::str::contains("invalid project ID"));
}

#[test]
fn nonexistent_source_directory_in_config_is_rejected() {
    let tmp = TempDir::new().unwrap();
    write_config(
        tmp.path(),
        "project: good-id\nsources:\n  - path: nowhere\n",
    );

    svt_in(tmp.path())
        .arg("analyze")
        .assert()
        .failure()
        .stderr(predicate::str::contains("source path does not exist"));
}

#[test]
fn invalid_server_url_in_config_is_rejected() {
    let tmp = TempDir::new().unwrap();
    write_config(tmp.path(), "project: good-id\nserver:\n  url: notaurl\n");

    svt_in(tmp.path())
        .arg("analyze")
        .assert()
        .failure()
        .stderr(predicate::str::contains("http:// or https://"));
}

#[test]
fn duplicate_source_entries_in_config_are_rejected() {
    let tmp = TempDir::new().unwrap();
    write_config(
        tmp.path(),
        "project: good-id\nsources:\n  - path: .\n  - path: .\n",
    );

    svt_in(tmp.path())
        .arg("analyze")
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate source entry"));
}

#[test]
fn duplicate_design_entries_in_config_are_rejected() {
    let tmp = TempDir::new().unwrap();
    write_config(
        tmp.path(),
        "project: good-id\ndesign:\n  - a.yaml\n  - a.yaml\n",
    );

    svt_in(tmp.path())
        .arg("analyze")
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate design entry"));
}

#[test]
fn valid_config_passes_validation_and_analyzes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    write_crate(root, "app", "app-crate");
    write_config(
        root,
        "project: good-id\nname: Good\nsources:\n  - path: app\nserver:\n  url: https://example.com\n",
    );

    svt_in(root)
        .arg("analyze")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created analysis snapshot"));
}

#[test]
fn missing_config_file_does_not_fail_validation() {
    let tmp = TempDir::new().unwrap();
    write_crate(tmp.path(), ".", "root-crate");

    svt_in(tmp.path()).arg("analyze").assert().success();
}
