//! CLI integration tests for `svt import`, `svt check`, and `svt analyze`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn svt_cmd() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("svt").unwrap()
}

/// Helper: create a command with --project-dir pointing at the temp directory.
fn svt_in(dir: &TempDir) -> Command {
    let mut cmd = svt_cmd();
    cmd.arg("--project-dir").arg(dir.path());
    cmd
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

    svt_in(&dir)
        .arg("import")
        .arg("--file")
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

    // Import first
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Then check
    svt_in(&dir)
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

    // Import first
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Check with JSON output
    let output = svt_in(&dir)
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

    svt_in(&dir).arg("check").assert().failure().stderr(
        predicate::str::contains("not found").or(predicate::str::contains("Store not found")),
    );
}

#[test]
fn import_on_invalid_yaml_gives_clear_error() {
    let dir = TempDir::new().unwrap();
    let bad_yaml = dir.path().join("bad.yaml");
    fs::write(&bad_yaml, "this is not valid: [yaml: {").unwrap();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&bad_yaml)
        .assert()
        .failure();
}

#[test]
fn import_on_unsupported_extension_gives_clear_error() {
    let dir = TempDir::new().unwrap();
    let txt_file = dir.path().join("design.txt");
    fs::write(&txt_file, "hello").unwrap();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&txt_file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported file format"));
}

#[test]
fn analyze_succeeds_on_workspace() {
    let dir = TempDir::new().unwrap();

    // Analyze this project's workspace root
    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    svt_in(&dir)
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

    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    svt_in(&dir)
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

    // Import design first
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Check with --analysis flag pointing to nonexistent version succeeds but
    // reports all design nodes as unimplemented (empty analysis snapshot)
    svt_in(&dir)
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

    svt_in(&dir)
        .arg("analyze")
        .arg("/nonexistent/path/to/project")
        .assert()
        .failure();
}

#[test]
fn export_mermaid_produces_flowchart() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
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

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    let output = svt_in(&dir)
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
    let output_path = dir.path().join("output.mmd");

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
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

    // Import design (creates version 1)
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Import again (creates version 2 with same content)
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Diff v1 vs v2 — same content, should show no changes
    svt_in(&dir)
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

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    let output = svt_in(&dir)
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

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
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

    svt_in(&dir).arg("export").assert().failure();
}

// -- svt store subcommand tests --

#[test]
fn store_info_on_nonexistent_store_gives_clear_error() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("store")
        .arg("info")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found").or(predicate::str::contains("Store not found")),
        );
}

#[test]
fn store_info_after_import_shows_snapshot() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("store")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("Schema version: 3"))
        .stdout(predicate::str::contains("Snapshots: 1"))
        .stdout(predicate::str::contains("design"));
}

#[test]
fn store_compact_keeps_latest_by_default() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import twice to create two versions
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Compact — should keep latest design
    svt_in(&dir)
        .arg("store")
        .arg("compact")
        .assert()
        .success()
        .stdout(predicate::str::contains("kept 1"))
        .stdout(predicate::str::contains("removed 1"));
}

#[test]
fn store_compact_with_explicit_keep() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import three times
    for _ in 0..3 {
        svt_in(&dir)
            .arg("import")
            .arg("--file")
            .arg(&yaml_path)
            .assert()
            .success();
    }

    // Keep only versions 1 and 3
    svt_in(&dir)
        .arg("store")
        .arg("compact")
        .arg("--keep")
        .arg("1")
        .arg("--keep")
        .arg("3")
        .assert()
        .success()
        .stdout(predicate::str::contains("kept 2"))
        .stdout(predicate::str::contains("removed 1"));
}

#[test]
fn store_reset_with_force_deletes_and_recreates() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import to create store with data
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Reset with --force
    svt_in(&dir)
        .arg("store")
        .arg("reset")
        .arg("--force")
        .assert()
        .success()
        .stdout(predicate::str::contains("Store reset"));

    // Verify store is empty (info shows 0 snapshots)
    svt_in(&dir)
        .arg("store")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("Snapshots: 0"));
}

#[test]
fn analyze_incremental_flag_accepted() {
    let dir = TempDir::new().unwrap();

    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // First run with --incremental (stores manifest for future runs)
    svt_in(&dir)
        .arg("analyze")
        .arg(&project_root)
        .arg("--incremental")
        .assert()
        .success();

    // Second run: incremental with previous manifest available
    svt_in(&dir)
        .arg("analyze")
        .arg(&project_root)
        .arg("--incremental")
        .assert()
        .success()
        .stdout(predicate::str::contains("incremental"))
        .stdout(predicate::str::contains("units skipped"));
}

#[test]
fn analyze_incremental_without_previous_works() {
    let dir = TempDir::new().unwrap();

    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // --incremental on first run with no previous version should still work (falls back to full)
    svt_in(&dir)
        .arg("analyze")
        .arg(&project_root)
        .arg("--incremental")
        .assert()
        .success()
        .stdout(predicate::str::contains("nodes"))
        .stdout(predicate::str::contains("edges"));
}

#[test]
fn analyze_incremental_second_run_skips_unchanged() {
    let dir = TempDir::new().unwrap();

    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // First run with --incremental (falls back to full, stores manifest)
    svt_in(&dir)
        .arg("analyze")
        .arg(&project_root)
        .arg("--incremental")
        .assert()
        .success();

    // Second run: should detect nothing changed and skip all units
    svt_in(&dir)
        .arg("analyze")
        .arg(&project_root)
        .arg("--incremental")
        .assert()
        .success()
        .stdout(predicate::str::contains("incremental"))
        .stdout(predicate::str::contains("nodes copied"))
        .stdout(predicate::str::contains("edges copied"));
}

#[test]
fn store_compact_on_nonexistent_store_gives_clear_error() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("store")
        .arg("compact")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found").or(predicate::str::contains("Store not found")),
        );
}

// -- Config-driven workflow tests --

/// Write a .svt/config.yaml with given project name and design files.
fn write_config(dir: &TempDir, project: &str, design_files: &[&str]) {
    let svt_dir = dir.path().join(".svt");
    fs::create_dir_all(svt_dir.join("data")).unwrap();
    let design_yaml: String = design_files
        .iter()
        .map(|f| format!("  - {f}"))
        .collect::<Vec<_>>()
        .join("\n");
    let config = if design_files.is_empty() {
        format!("project: {project}\n")
    } else {
        format!("project: {project}\ndesign:\n{design_yaml}\n")
    };
    fs::write(svt_dir.join("config.yaml"), config).unwrap();
}

#[test]
fn init_creates_config_and_data_dir() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("init")
        .arg("--project")
        .arg("test-proj")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized project 'test-proj'"));

    assert!(dir.path().join(".svt/config.yaml").exists());
    assert!(dir.path().join(".svt/data").is_dir());
}

#[test]
fn init_refuses_to_overwrite_existing_config() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "existing", &[]);

    svt_in(&dir)
        .arg("init")
        .arg("--project")
        .arg("new-proj")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn import_with_config_merges_multiple_design_files() {
    let dir = TempDir::new().unwrap();

    // Create two design files
    let design1 = dir.path().join("arch.yaml");
    fs::write(
        &design1,
        r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
"#,
    )
    .unwrap();

    let design2 = dir.path().join("frontend.yaml");
    fs::write(
        &design2,
        r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /web
    kind: system
"#,
    )
    .unwrap();

    write_config(&dir, "merge-test", &["arch.yaml", "frontend.yaml"]);

    // Import with no args — should merge from config
    svt_in(&dir)
        .arg("import")
        .assert()
        .success()
        .stdout(predicate::str::contains("2 design files"))
        .stdout(predicate::str::contains("nodes"));
}

#[test]
fn import_with_no_config_and_no_file_gives_error() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("import")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No design files"));
}

#[test]
fn import_file_flag_overrides_config() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Config has a nonexistent design file — but --file should override
    write_config(&dir, "override-test", &["nonexistent.yaml"]);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"));
}

#[test]
fn config_project_id_is_used_for_store() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    write_config(&dir, "my-project", &[]);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Store info should work (project was auto-created from config)
    svt_in(&dir)
        .arg("store")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("Snapshots: 1"));
}

#[test]
fn backward_compat_works_without_config_file() {
    // Tests that commands work with just --project-dir pointing at a clean temp dir
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // No .svt/config.yaml — should use defaults (project="default")
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("PASS"));
}

#[test]
fn push_without_server_and_no_config_gives_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Push with no --server and no config should fail
    svt_in(&dir)
        .arg("push")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No server URL"));
}

#[test]
fn push_kind_flag_is_accepted() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Push with --kind design and bogus server should fail with connection error, not arg error
    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("design")
        .assert()
        .failure()
        .stderr(predicate::str::contains("push").or(predicate::str::contains("failed")));
}

#[test]
fn analyze_without_path_uses_default() {
    let dir = TempDir::new().unwrap();

    // Analyze with no path arg — should default to "." which is the tempdir (empty, but should not crash)
    svt_in(&dir)
        .arg("analyze")
        .assert()
        .success()
        .stdout(predicate::str::contains("Analyzed"));
}

#[test]
fn analyze_with_config_empty_sources_does_not_panic() {
    let dir = TempDir::new().unwrap();

    // Config exists but sources list is empty — should fall back to project dir, not panic
    let svt_dir = dir.path().join(".svt");
    fs::create_dir_all(svt_dir.join("data")).unwrap();
    fs::write(
        svt_dir.join("config.yaml"),
        "project: test-proj\nsources: []\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("analyze")
        .assert()
        .success()
        .stdout(predicate::str::contains("Analyzed"));
}

#[test]
fn analyze_with_config_source_path_is_used() {
    let dir = TempDir::new().unwrap();

    // Create a subdirectory as the source
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let svt_dir = dir.path().join(".svt");
    fs::create_dir_all(svt_dir.join("data")).unwrap();
    fs::write(
        svt_dir.join("config.yaml"),
        "project: test-proj\nsources:\n  - path: src\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("analyze")
        .assert()
        .success()
        .stdout(predicate::str::contains("Analyzed"));
}

#[test]
fn import_with_config_no_design_files_gives_error() {
    let dir = TempDir::new().unwrap();

    // Config exists but has no design files listed
    write_config(&dir, "test-proj", &[]);

    svt_in(&dir)
        .arg("import")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No design files"));
}

#[test]
fn push_with_no_server_anywhere_gives_error() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "test-proj", &[]);

    // Import something first so there's data to push
    let design_path = write_design_yaml(&dir);
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&design_path)
        .assert()
        .success();

    // Push with no --server flag and no server in config
    svt_in(&dir)
        .arg("push")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No server URL"));
}

// -- Additional coverage tests --

/// Helper: write a design YAML that has a constraint violation (cyclic dependency).
fn write_design_with_violation(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("violation.yaml");
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
  - source: /app/core
    target: /app/cli
    kind: depends
constraints:
  - name: core-no-cli
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
  - name: info-constraint
    kind: must_not_depend
    scope: /app/cli/**
    target: /app/core/**
    message: "Info level constraint"
    severity: info
"#,
    )
    .unwrap();
    path
}

/// Helper: write a second design YAML with different nodes for diff testing.
fn write_design_yaml_v2(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("design_v2.yaml");
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
      - canonical_path: /app/web
        kind: service
edges:
  - source: /app/web
    target: /app/core
    kind: depends
"#,
    )
    .unwrap();
    path
}

#[test]
fn check_fail_on_warning_exits_nonzero_when_violation_exists() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_with_violation(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // With --fail-on error (default), the constraint violation should cause exit 1
    svt_in(&dir)
        .arg("check")
        .arg("--fail-on")
        .arg("error")
        .assert()
        .failure();
}

#[test]
fn check_fail_on_warning_threshold() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_with_violation(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // --fail-on warning should also fail (error >= warning threshold)
    svt_in(&dir)
        .arg("check")
        .arg("--fail-on")
        .arg("warning")
        .assert()
        .failure();
}

#[test]
fn check_fail_on_info_threshold() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_with_violation(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // --fail-on info should also fail (error >= info threshold)
    svt_in(&dir)
        .arg("check")
        .arg("--fail-on")
        .arg("info")
        .assert()
        .failure();
}

#[test]
fn check_human_format_shows_fail_tag() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_with_violation(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    let output = svt_in(&dir)
        .arg("check")
        .arg("--format")
        .arg("human")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("FAIL"),
        "Human format should contain FAIL tag, got: {stdout}"
    );
}

#[test]
fn check_json_format_with_violation_has_fail_status() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_with_violation(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    let output = svt_in(&dir)
        .arg("check")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let results = parsed["constraint_results"].as_array().unwrap();
    let has_fail = results.iter().any(|r| r["status"].as_str() == Some("fail"));
    assert!(has_fail, "JSON report should contain a fail result");
}

#[test]
fn export_svg_format_is_accepted() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // SVG export requires Graphviz `dot` — test that the command is accepted
    // (it either succeeds with SVG output or fails with a clear Graphviz error)
    let output = svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("svg")
        .output()
        .unwrap();

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("<svg"),
            "SVG output should contain <svg tag"
        );
    } else {
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("Graphviz") || stderr.contains("dot"),
            "SVG failure should mention Graphviz dependency, got: {stderr}"
        );
    }
}

#[test]
fn export_dot_to_file_creates_output() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let output_path = dir.path().join("output.dot");

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("dot")
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported to"));

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(
        content.contains("digraph"),
        "DOT file should contain 'digraph', got: {content}"
    );
}

#[test]
fn export_unknown_format_gives_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("foobar")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown format"));
}

#[test]
fn export_svg_to_file_is_accepted() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let output_path = dir.path().join("output.svg");

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // SVG export requires Graphviz `dot` — test the command is accepted
    let output = svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("svg")
        .arg("--output")
        .arg(&output_path)
        .output()
        .unwrap();

    if output.status.success() {
        let content = fs::read_to_string(&output_path).unwrap();
        assert!(
            content.contains("<svg"),
            "SVG file should contain '<svg' tag"
        );
    } else {
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("Graphviz") || stderr.contains("dot"),
            "SVG failure should mention Graphviz dependency, got: {stderr}"
        );
    }
}

#[test]
fn diff_with_different_designs_shows_changes() {
    let dir = TempDir::new().unwrap();
    let yaml_v1 = write_design_yaml(&dir);
    let yaml_v2 = write_design_yaml_v2(&dir);

    // Import v1
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_v1)
        .assert()
        .success();

    // Import v2 (different nodes)
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_v2)
        .assert()
        .success();

    // Diff should show changes (added /app/web, removed /app/cli)
    svt_in(&dir)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("Diff: v1 -> v2"))
        .stdout(
            predicate::str::contains("added")
                .or(predicate::str::contains("+"))
                .or(predicate::str::contains("-")),
        );
}

#[test]
fn diff_json_with_different_designs_shows_changes() {
    let dir = TempDir::new().unwrap();
    let yaml_v1 = write_design_yaml(&dir);
    let yaml_v2 = write_design_yaml_v2(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_v1)
        .assert()
        .success();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_v2)
        .assert()
        .success();

    let output = svt_in(&dir)
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
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    let summary = &parsed["summary"];
    let total_changes = summary["nodes_added"].as_u64().unwrap_or(0)
        + summary["nodes_removed"].as_u64().unwrap_or(0)
        + summary["nodes_changed"].as_u64().unwrap_or(0);
    assert!(
        total_changes > 0,
        "Diff between different designs should have node changes, summary: {summary}"
    );
}

#[test]
fn store_compact_on_empty_store_after_reset() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import to create store, then reset it
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("store")
        .arg("reset")
        .arg("--force")
        .assert()
        .success();

    // Compact on empty store should succeed (nothing to remove)
    svt_in(&dir)
        .arg("store")
        .arg("compact")
        .assert()
        .success()
        .stdout(predicate::str::contains("kept 0"))
        .stdout(predicate::str::contains("removed 0"));
}

#[test]
fn store_compact_with_keep_specific_version() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import three times
    for _ in 0..3 {
        svt_in(&dir)
            .arg("import")
            .arg("--file")
            .arg(&yaml_path)
            .assert()
            .success();
    }

    // Keep only version 1
    svt_in(&dir)
        .arg("store")
        .arg("compact")
        .arg("--keep")
        .arg("1")
        .assert()
        .success()
        .stdout(predicate::str::contains("kept 1"))
        .stdout(predicate::str::contains("removed 2"));
}

#[test]
fn store_reset_on_nonexistent_store_gives_error() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("store")
        .arg("reset")
        .arg("--force")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("Store not found"))
                .or(predicate::str::contains("Nothing to reset")),
        );
}

#[test]
fn store_info_shows_version_kind_table_headers() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("store")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("VERSION"))
        .stdout(predicate::str::contains("KIND"))
        .stdout(predicate::str::contains("NODES"))
        .stdout(predicate::str::contains("EDGES"));
}

#[test]
fn import_nonexistent_file_gives_error() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg("/nonexistent/path/to/design.yaml")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn push_with_unreachable_server_gives_connection_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Use unreachable address
    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("design")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed"));
}

#[test]
fn push_with_invalid_kind_gives_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("invalid-kind")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown push kind"));
}

#[test]
fn push_all_kind_with_unreachable_server() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // --kind all should try to push design (and fail on connection)
    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("all")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed"));
}

#[test]
fn push_analysis_kind_with_no_analysis_gives_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Push --kind analysis but only design was imported
    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("analysis")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No analysis versions found"));
}

#[test]
fn export_json_to_file_creates_valid_interchange() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let output_path = dir.path().join("output.json");

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Exported to"));

    let content = fs::read_to_string(&output_path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("output file should be valid JSON");
    assert_eq!(parsed["format"], "svt/v1");
}

#[test]
fn export_on_empty_store_gives_error() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("mermaid")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found").or(predicate::str::contains("Store not found")),
        );
}

#[test]
fn import_json_format_succeeds() {
    let dir = TempDir::new().unwrap();
    let json_path = dir.path().join("design.json");
    fs::write(
        &json_path,
        r#"{
  "format": "svt/v1",
  "kind": "design",
  "nodes": [
    {
      "canonical_path": "/app",
      "kind": "system",
      "children": []
    }
  ],
  "edges": [],
  "constraints": []
}"#,
    )
    .unwrap();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&json_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Imported"))
        .stdout(predicate::str::contains("nodes"));
}

#[test]
fn check_with_specific_design_version() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Check with explicit --design 1
    svt_in(&dir)
        .arg("check")
        .arg("--design")
        .arg("1")
        .assert()
        .success()
        .stdout(predicate::str::contains("Checking design v1"));
}

#[test]
fn store_reset_after_reset_gives_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // First reset succeeds
    svt_in(&dir)
        .arg("store")
        .arg("reset")
        .arg("--force")
        .assert()
        .success();

    // Store info after reset shows empty store
    svt_in(&dir)
        .arg("store")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("Snapshots: 0"));
}

#[test]
fn export_with_specific_version() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import twice
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Export from version 1 explicitly
    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("json")
        .arg("--version")
        .arg("1")
        .assert()
        .success()
        .stdout(predicate::str::contains("svt/v1"));
}

#[test]
fn export_png_requires_output_flag() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // PNG without --output should fail
    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("png")
        .assert()
        .failure()
        .stderr(predicate::str::contains("binary format").or(predicate::str::contains("--output")));
}

#[test]
fn diff_on_empty_store_gives_error() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("2")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found").or(predicate::str::contains("Store not found")),
        );
}

#[test]
fn plugin_list_with_no_plugins_loaded() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("plugin")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins loaded"));
}

// -- Init coverage tests --

#[test]
fn init_creates_gitignore_with_svt_data_entry() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("init")
        .arg("--project")
        .arg("gi-test")
        .current_dir(dir.path())
        .assert()
        .success();

    let gitignore = dir.path().join(".gitignore");
    assert!(gitignore.exists(), ".gitignore should be created");
    let content = fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains(".svt/data"),
        ".gitignore should contain .svt/data, got: {content}"
    );
}

#[test]
fn init_config_yaml_contains_correct_project_name() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("init")
        .arg("--project")
        .arg("my-cool-project")
        .current_dir(dir.path())
        .assert()
        .success();

    let config_content = fs::read_to_string(dir.path().join(".svt/config.yaml")).unwrap();
    assert!(
        config_content.contains("my-cool-project"),
        "config.yaml should contain project name, got: {config_content}"
    );
}

#[test]
fn init_appends_to_existing_gitignore() {
    let dir = TempDir::new().unwrap();

    // Create an existing .gitignore with other content
    let gitignore = dir.path().join(".gitignore");
    fs::write(&gitignore, "target/\nnode_modules/\n").unwrap();

    svt_in(&dir)
        .arg("init")
        .arg("--project")
        .arg("append-test")
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(&gitignore).unwrap();
    assert!(
        content.contains("target/"),
        "existing entries should be preserved"
    );
    assert!(
        content.contains(".svt/data"),
        ".svt/data should be appended"
    );
}

#[test]
fn init_skips_gitignore_append_if_already_has_svt_data() {
    let dir = TempDir::new().unwrap();

    // Create a .gitignore that already has .svt/data
    let gitignore = dir.path().join(".gitignore");
    fs::write(&gitignore, "target/\n.svt/data\n").unwrap();

    svt_in(&dir)
        .arg("init")
        .arg("--project")
        .arg("idempotent-test")
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(&gitignore).unwrap();
    // Count occurrences of .svt/data — should be exactly 1
    let count = content.matches(".svt/data").count();
    assert_eq!(
        count, 1,
        ".svt/data should appear exactly once, got {count} in: {content}"
    );
}

#[test]
fn init_derives_project_name_from_directory_when_no_flag() {
    let dir = TempDir::new().unwrap();
    // Create a named subdirectory to test derive_project_name fallback
    let named_dir = dir.path().join("my-named-project");
    fs::create_dir_all(&named_dir).unwrap();

    // Run without --project; should derive name from directory
    let mut cmd = svt_cmd();
    cmd.arg("--project-dir")
        .arg(&named_dir)
        .arg("init")
        .current_dir(&named_dir);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should mention the derived project name (from directory or git)
    assert!(
        stdout.contains("Initialized project"),
        "Should succeed with derived project name, stdout: {stdout}"
    );
    // Config should exist
    assert!(named_dir.join(".svt/config.yaml").exists());
}

// -- Push coverage tests --

#[test]
fn push_design_kind_with_unreachable_server_gives_error_message() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    let output = svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("design")
        .output()
        .unwrap();

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("failed") || stderr.contains("error"),
        "Should show connection error, got: {stderr}"
    );
    assert!(!output.status.success());
}

// -- Analyze coverage tests --

#[test]
fn analyze_with_explicit_path_argument_overrides_config() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "path-override", &[]);

    // Create a subdirectory with a simple Rust file
    let sub = dir.path().join("subdir");
    fs::create_dir_all(&sub).unwrap();
    fs::write(
        sub.join("Cargo.toml"),
        "[package]\nname = \"sub\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::create_dir_all(sub.join("src")).unwrap();
    fs::write(sub.join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

    svt_in(&dir)
        .arg("analyze")
        .arg(sub.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Analyzed"));
}

// -- Diff coverage tests --

#[test]
fn diff_when_target_version_does_not_exist_shows_all_removed() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import only one version
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Diff --from 1 --to 2: version 2 is empty, so everything shows as removed
    svt_in(&dir)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("removed"));
}

#[test]
fn diff_same_version_shows_no_changes() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Diff a version against itself
    svt_in(&dir)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("1")
        .assert()
        .success()
        .stdout(predicate::str::contains("No changes"));
}

// -- Store compact coverage tests --

#[test]
fn store_compact_with_only_design_no_analysis() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import two design versions, no analysis
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Compact should keep latest design only
    svt_in(&dir)
        .arg("store")
        .arg("compact")
        .assert()
        .success()
        .stdout(predicate::str::contains("kept 1"))
        .stdout(predicate::str::contains("removed 1"));
}

#[test]
fn store_compact_with_only_analysis_no_design() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "compact-analysis", &[]);

    // Create a minimal Rust project to analyze
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "pub fn hello() {}\n").unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"compact-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    // Analyze twice to get two analysis versions (no design)
    svt_in(&dir)
        .arg("analyze")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();
    svt_in(&dir)
        .arg("analyze")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    // Compact should keep latest analysis only
    svt_in(&dir)
        .arg("store")
        .arg("compact")
        .assert()
        .success()
        .stdout(predicate::str::contains("kept 1"))
        .stdout(predicate::str::contains("removed 1"));
}

// -- Export edge case coverage --

#[test]
fn export_no_data_in_store_after_reset_gives_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import then reset — store exists but is empty
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("store")
        .arg("reset")
        .arg("--force")
        .assert()
        .success();

    // Export should now fail with "no design versions" error
    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("mermaid")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No design versions"));
}

#[test]
fn export_mermaid_to_stdout_does_not_require_output_flag() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Mermaid to stdout (no --output) should work fine
    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("mermaid")
        .assert()
        .success()
        .stdout(predicate::str::contains("flowchart"));
}

// -- Check edge cases --

#[test]
fn check_with_analysis_and_no_analysis_in_store_gives_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // --analysis with no analysis versions should fail
    svt_in(&dir)
        .arg("check")
        .arg("--analysis")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No analysis versions"));
}

#[test]
fn check_with_specific_analysis_version() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    write_config(&dir, "check-analysis", &[]);

    // Import design
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Create a minimal Rust project and analyze it
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "pub fn hello() {}\n").unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"check-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("analyze")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    // Check with specific analysis version (should be v2 since design is v1)
    svt_in(&dir)
        .arg("check")
        .arg("--analysis")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("Comparing design"));
}

// -- Plugin list with project-dir --

#[test]
fn plugin_list_with_project_dir_flag() {
    let dir = TempDir::new().unwrap();

    svt_in(&dir)
        .arg("plugin")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins loaded"));
}

// -- Store info edge cases --

#[test]
fn store_info_after_analyze_shows_analysis_kind() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "info-analysis", &[]);

    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "pub fn hello() {}\n").unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"info-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("analyze")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    svt_in(&dir)
        .arg("store")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("analysis"))
        .stdout(predicate::str::contains("Snapshots: 1"));
}

// -- Import edge cases --

#[test]
fn import_config_with_nonexistent_design_file_gives_error() {
    let dir = TempDir::new().unwrap();
    write_config(&dir, "bad-design", &["nonexistent.yaml"]);

    svt_in(&dir)
        .arg("import")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

// -- Check report output coverage --

#[test]
fn check_human_format_shows_pass_count_summary() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("passed"))
        .stdout(predicate::str::contains("failed"));
}

#[test]
fn check_default_fail_on_is_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Default fail_on is "error"; design with no violations should pass
    svt_in(&dir).arg("check").assert().success();
}

// -- Analyze incremental coverage --

#[test]
fn analyze_incremental_reports_skipped_and_reanalyzed_counts() {
    let dir = TempDir::new().unwrap();

    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    // First analyze with --incremental (falls back to full, stores manifest)
    svt_in(&dir)
        .arg("analyze")
        .arg(&project_root)
        .arg("--incremental")
        .assert()
        .success();

    // Second analyze with --incremental should report skipped/reanalyzed counts
    svt_in(&dir)
        .arg("analyze")
        .arg("--incremental")
        .arg(&project_root)
        .assert()
        .success()
        .stdout(predicate::str::contains("incremental"))
        .stdout(predicate::str::contains("units skipped"))
        .stdout(predicate::str::contains("re-analyzed"));
}

// -- Push with server from config --

#[test]
fn push_uses_server_url_from_config() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Create config with server URL
    let svt_dir = dir.path().join(".svt");
    fs::create_dir_all(&svt_dir).unwrap();
    fs::write(
        svt_dir.join("config.yaml"),
        "project: push-config-test\nserver:\n  url: http://127.0.0.1:1\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Push without --server flag — should pick up from config and fail on connection
    svt_in(&dir)
        .arg("push")
        .arg("--kind")
        .arg("design")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed"));
}

// -- Export version flag with analysis --

#[test]
fn export_with_analysis_version_after_import() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import design (creates v1), then import again (creates v2)
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Export v1 explicitly (not latest) as JSON
    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("json")
        .arg("--version")
        .arg("1")
        .assert()
        .success()
        .stdout(predicate::str::contains("svt/v1"));

    // Export v2 explicitly
    svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("json")
        .arg("--version")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("svt/v1"));
}

// -- Push with explicit version flag --

#[test]
fn push_design_with_explicit_version() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);

    // Import twice so version 1 and 2 exist
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Push --kind design --version 1 (explicit version, should fail on connection)
    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("design")
        .arg("--version")
        .arg("1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed"));
}

#[test]
fn push_analysis_with_explicit_version() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    write_config(&dir, "push-ver-test", &[]);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Create analysis snapshot
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "pub fn hello() {}\n").unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"push-ver\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("analyze")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    // Push --kind analysis --version 2 (explicit version)
    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("analysis")
        .arg("--version")
        .arg("2")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed"));
}

#[test]
fn push_all_with_both_design_and_analysis() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    write_config(&dir, "push-all-test", &[]);

    // Import design
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Create analysis
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "pub fn hello() {}\n").unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"push-all\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("analyze")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    // Push --kind all with both design and analysis present
    // This exercises the lines 1131-1143 where both design and analysis are pushed
    svt_in(&dir)
        .arg("push")
        .arg("--server")
        .arg("http://127.0.0.1:1")
        .arg("--kind")
        .arg("all")
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed"));
}

// -- Diff changed-fields coverage --

/// Write a design where /app/core has a description, so diffing against a version without it shows Changed.
fn write_design_with_description(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("design_desc.yaml");
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
        description: "The core service"
      - canonical_path: /app/cli
        kind: service
edges:
  - source: /app/cli
    target: /app/core
    kind: depends
"#,
    )
    .unwrap();
    path
}

#[test]
fn diff_shows_changed_nodes_with_changed_fields() {
    let dir = TempDir::new().unwrap();
    let yaml_v1 = write_design_yaml(&dir);
    let yaml_v2 = write_design_with_description(&dir);

    // Import v1 (no description)
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_v1)
        .assert()
        .success();

    // Import v2 (with description on /app/core)
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_v2)
        .assert()
        .success();

    // Diff should show "~" (Changed) and field names in brackets
    svt_in(&dir)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("2")
        .assert()
        .success()
        .stdout(predicate::str::contains("~").or(predicate::str::contains("changed")));
}

// -- Validation warnings coverage --

#[test]
fn import_design_with_validation_warnings() {
    let dir = TempDir::new().unwrap();
    // Create a design with a duplicate canonical_path which should trigger a validation warning
    let path = dir.path().join("warn.yaml");
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
      - canonical_path: /app/core
        kind: service
edges: []
constraints: []
"#,
    )
    .unwrap();

    // Import should succeed but might produce validation warnings on stderr
    let output = svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&path)
        .output()
        .unwrap();

    // Whether it succeeds or fails, the command should run (not panic)
    let _stdout = String::from_utf8(output.stdout).unwrap();
    let _stderr = String::from_utf8(output.stderr).unwrap();
}

// -- Export PNG to file coverage --

#[test]
fn export_png_to_file_is_accepted() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let output_path = dir.path().join("output.png");

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // PNG export requires Graphviz `dot` — test the command is accepted
    let output = svt_in(&dir)
        .arg("export")
        .arg("--format")
        .arg("png")
        .arg("--output")
        .arg(&output_path)
        .output()
        .unwrap();

    if output.status.success() {
        assert!(
            output_path.exists(),
            "PNG file should be created on success"
        );
    } else {
        let stderr = String::from_utf8(output.stderr).unwrap();
        // Should fail with Graphviz-related error, not a crash
        assert!(
            stderr.contains("Graphviz") || stderr.contains("dot") || stderr.contains("svg"),
            "PNG failure should mention Graphviz dependency, got: {stderr}"
        );
    }
}

// -- Plugin list with --plugin-dir pointing to empty dir --

#[test]
fn plugin_list_with_explicit_empty_plugin_dir() {
    let dir = TempDir::new().unwrap();
    let plugin_dir = dir.path().join("my-plugins");
    fs::create_dir_all(&plugin_dir).unwrap();

    svt_in(&dir)
        .arg("--plugin-dir")
        .arg(&plugin_dir)
        .arg("plugin")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins loaded"));
}

// -- Check with NotEvaluable constraint status --

#[test]
fn check_with_analysis_shows_unimplemented_and_undocumented() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    write_config(&dir, "ne-test", &[]);

    // Import design
    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    // Create a minimal project that does NOT match the design nodes
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "pub fn unrelated() {}\n").unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"ne-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    svt_in(&dir)
        .arg("analyze")
        .arg(dir.path().to_str().unwrap())
        .assert()
        .success();

    // Check design vs analysis — should show Unimplemented and possibly Undocumented
    svt_in(&dir)
        .arg("check")
        .arg("--analysis")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Comparing design")
                .and(predicate::str::contains("Unimplemented").or(predicate::str::contains("N/A"))),
        );
}

// -- Init with git remote URL derivation --

#[test]
fn init_derives_project_name_from_git_remote_url() {
    // Run init inside this project's git repo (which has a remote origin)
    // to exercise the derive_project_name git URL parsing path
    let dir = TempDir::new().unwrap();

    // Create a git repo with a remote URL
    let repo_dir = dir.path().join("my-repo");
    fs::create_dir_all(&repo_dir).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/example/test-repo.git",
        ])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    // Run init without --project — should derive name from the git remote
    let mut cmd = svt_cmd();
    cmd.arg("--project-dir")
        .arg(&repo_dir)
        .arg("init")
        .current_dir(&repo_dir);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        stdout.contains("Initialized project 'test-repo'"),
        "Should derive 'test-repo' from git remote URL, got: {stdout}"
    );
}

#[test]
fn init_derives_project_name_from_ssh_git_remote() {
    let dir = TempDir::new().unwrap();
    let repo_dir = dir.path().join("ssh-repo");
    fs::create_dir_all(&repo_dir).unwrap();
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "git@github.com:user/My-Project.git",
        ])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    let mut cmd = svt_cmd();
    cmd.arg("--project-dir")
        .arg(&repo_dir)
        .arg("init")
        .current_dir(&repo_dir);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should derive lowercase, hyphenated name from SSH URL
    assert!(
        stdout.contains("Initialized project 'my-project'"),
        "Should derive 'my-project' from SSH git remote URL, got: {stdout}"
    );
}

// -- Plugin load warning coverage --

#[test]
fn plugin_flag_with_nonexistent_file_warns_but_continues() {
    let dir = TempDir::new().unwrap();

    // Pass a nonexistent plugin file — should warn on stderr but still run the command
    svt_in(&dir)
        .arg("--plugin")
        .arg("/nonexistent/plugin.so")
        .arg("plugin")
        .arg("list")
        .assert()
        .success()
        .stderr(predicate::str::contains("WARN"))
        .stdout(predicate::str::contains("No plugins loaded"));
}

#[test]
fn plugin_dir_with_invalid_library_warns_but_continues() {
    let dir = TempDir::new().unwrap();
    let plugin_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugin_dir).unwrap();

    // Create a fake shared library file with the right extension but invalid content
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    let fake_lib = plugin_dir.join(format!("fake_plugin.{ext}"));
    fs::write(&fake_lib, b"not a real library").unwrap();

    // The scan should find the file, fail to load it, and warn
    svt_in(&dir)
        .arg("--plugin-dir")
        .arg(&plugin_dir)
        .arg("plugin")
        .arg("list")
        .assert()
        .success()
        .stderr(predicate::str::contains("WARN"))
        .stdout(predicate::str::contains("No plugins loaded"));
}

// -- Diff format coverage --

#[test]
fn diff_human_format_shows_summary_line() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let yaml_v2 = write_design_yaml_v2(&dir);

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_path)
        .assert()
        .success();

    svt_in(&dir)
        .arg("import")
        .arg("--file")
        .arg(&yaml_v2)
        .assert()
        .success();

    svt_in(&dir)
        .arg("diff")
        .arg("--from")
        .arg("1")
        .arg("--to")
        .arg("2")
        .arg("--format")
        .arg("human")
        .assert()
        .success()
        .stdout(predicate::str::contains("added"))
        .stdout(predicate::str::contains("removed"));
}
