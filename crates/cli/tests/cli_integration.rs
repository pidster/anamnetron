//! CLI integration tests for `svt import` and `svt check`.

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
