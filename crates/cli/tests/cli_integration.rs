//! CLI integration tests for `svt import`, `svt check`, and `svt analyze`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn svt_cmd() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("svt").unwrap()
}

fn write_design_yaml(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("design.yaml");
    fs::write(
        &path,
        r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
      - canonical_path: /app/cli
        kind: service
edges:
  - source: /app/cli
    target: /app/core
    kind: depends
constraints:
  - name: core-no-cli
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
"#,
    )
    .unwrap();
    path
}

#[test]
fn import_succeeds_on_valid_yaml() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"))
        .stdout(predicate::str::contains("nodes"));
}

#[test]
fn check_succeeds_after_import() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    // Import first
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Then check
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"))
        .stdout(predicate::str::contains("passed"));
}

#[test]
fn check_json_format_produces_valid_json() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    // Import first
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Check with JSON output
    let output = svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("check")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("output should be valid JSON");
    assert!(parsed.get("constraint_results").is_some());
    assert!(parsed.get("summary").is_some());
}

#[test]
fn check_on_empty_store_gives_clear_error() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("check")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found").or(predicate::str::contains("Store not found")),
        );
}

#[test]
fn import_on_invalid_yaml_gives_clear_error() {
    let dir = TempDir::new().unwrap();
    let bad_yaml = dir.path().join("bad.yaml");
    fs::write(&bad_yaml, "this is not valid: [yaml: {").unwrap();
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&bad_yaml)
        .assert()
        .failure();
}

#[test]
fn import_on_unsupported_extension_gives_clear_error() {
    let dir = TempDir::new().unwrap();
    let txt_file = dir.path().join("design.txt");
    fs::write(&txt_file, "hello").unwrap();
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&txt_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported file format"));
}

#[test]
fn analyze_succeeds_on_workspace() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join(".svt/store");

    // Analyze this project's workspace root
    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("analyze")
        .arg(&project_root)
        .assert()
        .success()
        .stdout(predicate::str::contains("nodes"))
        .stdout(predicate::str::contains("edges"));
}

#[test]
fn analyze_with_commit_ref() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join(".svt/store");

    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("analyze")
        .arg(&project_root)
        .arg("--commit-ref")
        .arg("abc123")
        .assert()
        .success()
        .stdout(predicate::str::contains("snapshot"));
}

#[test]
fn check_with_analysis_flag_accepted() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    // Import design first
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Check with --analysis flag pointing to nonexistent version succeeds but
    // reports all design nodes as unimplemented (empty analysis snapshot)
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("check")
        .arg("--analysis")
        .arg("999")
        .assert()
        .success()
        .stdout(predicate::str::contains("Comparing design"))
        .stdout(predicate::str::contains("Unimplemented"));
}

#[test]
fn analyze_on_nonexistent_path_gives_error() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("analyze")
        .arg("/nonexistent/path/to/project")
        .assert()
        .failure();
}

#[test]
fn export_mermaid_produces_flowchart() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .arg("--format")
        .arg("mermaid")
        .assert()
        .success()
        .stdout(predicate::str::contains("flowchart TD"))
        .stdout(predicate::str::contains("subgraph"));
}

#[test]
fn export_json_produces_valid_interchange() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    let output = svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("output should be valid JSON");
    assert_eq!(parsed["format"], "svt/v1");
}

#[test]
fn export_to_file_creates_output() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");
    let output_path = dir.path().join("output.mmd");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .arg("--format")
        .arg("mermaid")
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success();

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.starts_with("flowchart TD"));
}

#[test]
fn diff_shows_changes_between_versions() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    // Import design (creates version 1)
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Import again (creates version 2 with same content)
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    // Diff v1 vs v2 — same content, should show no changes
    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("Diff: v1 -> v2"))
        .stdout(predicate::str::contains("No changes"));
}

#[test]
fn diff_json_format_produces_valid_json() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    let output = svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("2")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("output should be valid JSON");
    assert!(parsed.get("from_version").is_some());
    assert!(parsed.get("to_version").is_some());
    assert!(parsed.get("summary").is_some());
}

#[test]
fn export_dot_produces_digraph() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("import")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .arg("--format")
        .arg("dot")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph"))
        .stdout(predicate::str::contains("subgraph cluster_"));
}

#[test]
fn export_without_format_gives_error() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join(".svt/store");

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("export")
        .assert()
        .failure();
}
