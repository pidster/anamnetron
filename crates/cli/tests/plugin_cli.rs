//! Integration tests for the CLI plugin subcommand and `--plugin` flag.

use assert_cmd::Command;
use predicates::prelude::*;

fn svt_cmd() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("svt").unwrap()
}

#[test]
fn plugin_list_with_no_plugins_shows_empty() {
    svt_cmd()
        .args(["plugin", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No plugins loaded"));
}

#[test]
fn plugin_flag_with_nonexistent_file_warns() {
    svt_cmd()
        .args(["--plugin", "/nonexistent/lib.dylib", "plugin", "list"])
        .assert()
        .success()
        .stderr(predicate::str::contains("failed to load plugin"));
}

#[test]
fn plugin_install_without_source_shows_error() {
    svt_cmd()
        .args(["plugin", "install"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn plugin_install_nonexistent_source_shows_error() {
    svt_cmd()
        .args(["plugin", "install", "/nonexistent/path"])
        .assert()
        .failure();
}

#[test]
fn plugin_remove_nonexistent_shows_error() {
    let dir = tempfile::tempdir().unwrap();
    svt_cmd()
        .current_dir(dir.path())
        .args(["plugin", "remove", "no-such-plugin"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn plugin_info_nonexistent_shows_error() {
    svt_cmd()
        .args(["plugin", "info", "/nonexistent/path"])
        .assert()
        .failure();
}

#[test]
fn plugin_info_with_valid_manifest_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let content = r#"[plugin]
name = "test-cli-info"
version = "1.0.0"
description = "CLI integration test"
api_version = 1
"#;
    std::fs::write(dir.path().join("svt-plugin.toml"), content).unwrap();

    svt_cmd()
        .args(["plugin", "info", &dir.path().display().to_string()])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-cli-info"))
        .stdout(predicate::str::contains("v1.0.0"));
}
