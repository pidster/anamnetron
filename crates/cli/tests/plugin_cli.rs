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
