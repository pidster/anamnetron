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
        .stdout(predicate::str::contains("Schema version: 2"))
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
