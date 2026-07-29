//! Headline proof for Phase F (P0): data-flow edges now appear for the
//! non-Rust analyzers.
//!
//! Before P0, the Go/TypeScript/Java/Python parsers never populated
//! `param_types`/`return_type` metadata, so the type-flow pass produced ZERO
//! `Transforms`/`DataFlow` edges for those languages. Each test here builds a
//! small on-disk fixture with a cross-function data flow (a producer returning
//! a project-local type `T` and a transformer consuming `T` and returning a
//! different project-local type `U`) and asserts that the analyzed graph
//! contains at least one data-flow edge for that language.
//!
//! This is a genuine end-to-end proof: it drives the real
//! [`svt_analyzer::analyze_project`] entry point, which discovers the fixture
//! via its manifest, runs the type-flow pass (pipeline Phase 6.6), and persists
//! the resulting edges into the store — exactly the path the CLI/server use.

use std::path::Path;

use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

use svt_analyzer::analyze_project;

/// Write a file, creating parent directories as needed.
fn write_file(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture dirs");
    }
    std::fs::write(&path, contents).expect("write fixture file");
}

/// Analyze a fixture directory and return the number of data-flow edges
/// (`Transforms` + `DataFlow`) in the resulting graph.
fn data_flow_edge_count(root: &Path) -> usize {
    let mut store = CozoStore::new_in_memory().expect("in-memory store");
    let summary =
        analyze_project(&mut store, DEFAULT_PROJECT_ID, root, None).expect("analysis succeeds");

    let edges = store
        .get_all_edges(summary.version, None)
        .expect("read edges");

    edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Transforms || e.kind == EdgeKind::DataFlow)
        .count()
}

#[test]
fn go_produces_data_flow_edges() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_file(
        dir.path(),
        "go.mod",
        "module example.com/votes\n\ngo 1.21\n",
    );
    write_file(
        dir.path(),
        "votes.go",
        r#"package votes

type Proposal struct {
    ID int
}

type Report struct {
    Total int
}

// Producer: returns a project-local type.
func Produce() Proposal {
    return Proposal{}
}

// Transformer: consumes Proposal, returns a different project-local type.
func Convert(p Proposal) Report {
    return Report{Total: p.ID}
}
"#,
    );

    let count = data_flow_edge_count(dir.path());
    assert!(
        count >= 1,
        "Go: expected at least one Transforms/DataFlow edge, got {count} \
         (P0 regression: non-Rust type-flow metadata is not being produced)"
    );
}

#[test]
fn typescript_produces_data_flow_edges() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_file(dir.path(), "package.json", r#"{"name": "votes"}"#);
    write_file(
        dir.path(),
        "index.ts",
        r#"export class Proposal {
    id = 0;
}

export class Report {
    total = 0;
}

// Producer: returns a project-local type.
export function produce(): Proposal {
    return new Proposal();
}

// Transformer: consumes Proposal, returns a different project-local type.
export function convert(p: Proposal): Report {
    const r = new Report();
    r.total = p.id;
    return r;
}
"#,
    );

    let count = data_flow_edge_count(dir.path());
    assert!(
        count >= 1,
        "TypeScript: expected at least one Transforms/DataFlow edge, got {count} \
         (P0 regression: non-Rust type-flow metadata is not being produced)"
    );
}

#[test]
fn java_produces_data_flow_edges() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_file(
        dir.path(),
        "pom.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <artifactId>votes</artifactId>
</project>
"#,
    );
    write_file(
        dir.path(),
        "Service.java",
        r#"class Proposal {
    int id;
}

class Report {
    int total;
}

public class Service {
    // Producer: returns a project-local type.
    Proposal produce() {
        return new Proposal();
    }

    // Transformer: consumes Proposal, returns a different project-local type.
    Report convert(Proposal p) {
        Report r = new Report();
        r.total = p.id;
        return r;
    }
}
"#,
    );

    let count = data_flow_edge_count(dir.path());
    assert!(
        count >= 1,
        "Java: expected at least one Transforms/DataFlow edge, got {count} \
         (P0 regression: non-Rust type-flow metadata is not being produced)"
    );
}

#[test]
fn python_produces_data_flow_edges() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_file(
        dir.path(),
        "pyproject.toml",
        "[project]\nname = \"votes\"\n",
    );
    write_file(
        dir.path(),
        "votes.py",
        r#"class Proposal:
    def __init__(self) -> None:
        self.id = 0


class Report:
    def __init__(self) -> None:
        self.total = 0


def produce() -> Proposal:
    return Proposal()


def convert(p: Proposal) -> Report:
    r = Report()
    r.total = p.id
    return r
"#,
    );

    let count = data_flow_edge_count(dir.path());
    assert!(
        count >= 1,
        "Python: expected at least one Transforms/DataFlow edge, got {count} \
         (P0 regression: non-Rust type-flow metadata is not being produced)"
    );
}
