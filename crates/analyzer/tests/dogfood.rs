//! Dog-food test: analyze this project and compare against the design model.

use std::path::PathBuf;

use svt_core::conformance::{self, ConstraintRegistry};
use svt_core::interchange::parse_yaml;
use svt_core::interchange_store::load_into_store;
use svt_core::model::DEFAULT_PROJECT_ID;
use svt_core::store::{CozoStore, GraphStore};

use svt_analyzer::analyze_project;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn dogfood_analyze_produces_meaningful_results() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root(), None).unwrap();

    // Should find all workspace crates
    assert!(
        summary.crates_analyzed >= 4,
        "should analyze at least 4 crates, got {}",
        summary.crates_analyzed
    );

    // Should produce substantial graph
    assert!(
        summary.nodes_created > 20,
        "should create many nodes, got {}",
        summary.nodes_created
    );

    // Should have edges
    assert!(
        summary.edges_created > 5,
        "should create edges, got {}",
        summary.edges_created
    );

    // Should find at least one TypeScript package (web/)
    assert!(
        summary.ts_packages_analyzed >= 1,
        "should analyze at least 1 TS package, got {}",
        summary.ts_packages_analyzed
    );

    println!(
        "Dog-food analysis: {} crates, {} TS packages, {} Go modules, {} Python packages, {} files, {} nodes, {} edges, {} warnings",
        summary.crates_analyzed,
        summary.ts_packages_analyzed,
        summary.go_packages_analyzed,
        summary.python_packages_analyzed,
        summary.files_analyzed,
        summary.nodes_created,
        summary.edges_created,
        summary.warnings.len()
    );
}

#[test]
fn dogfood_typescript_nodes_in_store() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root(), None).unwrap();

    let nodes = store.get_all_nodes(summary.version).unwrap();

    // Should find the web package node
    let package_nodes: Vec<_> = nodes.iter().filter(|n| n.sub_kind == "package").collect();
    assert!(
        !package_nodes.is_empty(),
        "should have at least one TS package node"
    );

    // Should find TypeScript nodes (language == "typescript" or "svelte")
    let ts_nodes: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n.language == Some("typescript".to_string()) || n.language == Some("svelte".to_string())
        })
        .collect();
    assert!(
        !ts_nodes.is_empty(),
        "should have TypeScript/Svelte nodes in store"
    );

    println!(
        "Dog-food TS: {} package nodes, {} TS/Svelte nodes",
        package_nodes.len(),
        ts_nodes.len()
    );
}

#[test]
fn dogfood_conformance_comparison() {
    let mut store = CozoStore::new_in_memory().unwrap();

    // Load design model
    let design_yaml =
        std::fs::read_to_string(project_root().join("design/architecture.yaml")).unwrap();
    let doc = parse_yaml(&design_yaml).unwrap();
    let design_version = load_into_store(&mut store, DEFAULT_PROJECT_ID, &doc).unwrap();

    // Run analysis
    let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root(), None).unwrap();

    // Compare
    let registry = ConstraintRegistry::with_defaults();
    let report = conformance::evaluate(&store, design_version, summary.version, &registry).unwrap();

    // All must_not_depend constraints should pass
    // (our code respects dependency direction: cli -> analyzer -> core)
    for result in &report.constraint_results {
        if result.constraint_kind == "must_not_depend" {
            assert_eq!(
                result.status,
                conformance::ConstraintStatus::Pass,
                "constraint '{}' should pass on real code, but got: {:?} with {} violations: {:?}",
                result.constraint_name,
                result.status,
                result.violations.len(),
                result.violations
            );
        }
    }

    // Print report for visibility
    println!("Conformance report:");
    println!(
        "  {} passed, {} failed, {} warned, {} not evaluable",
        report.summary.passed,
        report.summary.failed,
        report.summary.warned,
        report.summary.not_evaluable
    );
    println!(
        "  {} unimplemented, {} undocumented",
        report.summary.unimplemented, report.summary.undocumented
    );

    if !report.unimplemented.is_empty() {
        println!("  Unimplemented:");
        for node in &report.unimplemented {
            println!("    - {} ({:?})", node.canonical_path, node.kind);
        }
    }

    if !report.undocumented.is_empty() {
        println!("  Undocumented:");
        for node in &report.undocumented {
            println!("    - {} ({:?})", node.canonical_path, node.kind);
        }
    }
}
