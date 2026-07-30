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

/// Analyze a fixture and return its `Transforms` edges as `(source, target)`
/// qualified-name pairs, so tests can assert on the resolved endpoints, plus the
/// qualified names of every node in the graph so tests can assert preconditions
/// (e.g. that a type name is genuinely declared in two packages). Graph edges
/// reference node IDs, so endpoints are mapped back to their nodes' qualified
/// names (edges whose endpoints lack a qualified name are skipped).
fn analyze_transforms_and_nodes(root: &Path) -> (Vec<(String, String)>, Vec<String>) {
    use std::collections::HashMap;

    let mut store = CozoStore::new_in_memory().expect("in-memory store");
    let summary =
        analyze_project(&mut store, DEFAULT_PROJECT_ID, root, None).expect("analysis succeeds");

    let nodes = store.get_all_nodes(summary.version).expect("read nodes");
    let node_qns: Vec<String> = nodes
        .iter()
        .filter_map(|n| n.qualified_name.clone())
        .collect();
    let qn_by_id: HashMap<String, String> = nodes
        .into_iter()
        .filter_map(|n| n.qualified_name.map(|qn| (n.id, qn)))
        .collect();

    let transforms = store
        .get_all_edges(summary.version, None)
        .expect("read edges")
        .iter()
        .filter(|e| e.kind == EdgeKind::Transforms)
        .filter_map(|e| {
            let src = qn_by_id.get(&e.source)?;
            let tgt = qn_by_id.get(&e.target)?;
            Some((src.clone(), tgt.clone()))
        })
        .collect();

    (transforms, node_qns)
}

#[test]
fn go_locally_unambiguous_type_recovers_globally_ambiguous_edge() {
    // Recall recovery, end to end: the short name `Config` is declared in TWO Go
    // packages (`app` at the root and `other` in a subdirectory), so it is
    // GLOBALLY ambiguous. The transformer `Convert(Config) Report` lives in the
    // `app` package, where `Config` unambiguously denotes `app`'s own `Config`.
    //
    // Under the P0 global-unique-only rule, `Config` would be left bare and the
    // Transforms edge dropped at graph mapping (a bare name matches no node).
    // Same-module-first resolves it to the `app`-local declaration, so the edge
    // survives mapping. Its mere presence — with a fully-qualified `::Config`
    // source that resolved to a real node — proves the recovery.
    let dir = tempfile::tempdir().expect("tempdir");
    write_file(dir.path(), "go.mod", "module example.com/app\n\ngo 1.21\n");
    write_file(
        dir.path(),
        "app.go",
        r#"package app

type Config struct {
    Name string
}

type Report struct {
    Total int
}

// Transformer: consumes the same-module Config, returns a different local type.
func Convert(c Config) Report {
    return Report{}
}
"#,
    );
    // A second package reusing the short name `Config`, making it globally
    // ambiguous. This is what caused P0 to drop the edge above.
    write_file(
        dir.path(),
        "other/other.go",
        r#"package other

type Config struct {
    Other bool
}
"#,
    );

    let (edges, node_qns) = analyze_transforms_and_nodes(dir.path());

    // Precondition: `Config` must be GENUINELY ambiguous (declared in two
    // discovered packages), or this test would pass vacuously via the
    // global-unique fallback and prove nothing about same-module recovery.
    let config_decls: Vec<&String> = node_qns
        .iter()
        .filter(|qn| qn.ends_with("::Config"))
        .collect();
    assert!(
        config_decls.len() >= 2,
        "fixture must declare Config in two packages to exercise recovery (P0 would \
         otherwise resolve it via the global-unique fallback); got: {config_decls:?}"
    );

    // Find the recovered Config -> Report edge and assert both endpoints resolved
    // within the SAME (`app`) module — never crossing into `other`.
    let recovered = edges
        .iter()
        .find(|(src, tgt)| src.ends_with("::Config") && tgt.ends_with("::Report"));
    let (src, tgt) = recovered.unwrap_or_else(|| {
        panic!("expected a recovered Config->Report Transforms edge, got: {edges:?}")
    });

    // Fully qualified (contains `::`) — proof it resolved to a node rather than
    // being dropped as a bare name, and both endpoints share the same module.
    let src_module = src.rsplit_once("::").expect("qualified source").0;
    let tgt_module = tgt.rsplit_once("::").expect("qualified target").0;
    assert_eq!(
        src_module, tgt_module,
        "recovered edge must resolve within one module, got {src} -> {tgt}"
    );
    assert!(
        !src_module.contains("other"),
        "same-module resolution must not reach into the `other` package, got {src}"
    );
}
