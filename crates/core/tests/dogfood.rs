//! Dog-food test: load design/architecture.yaml and run conformance checks.

use svt_core::conformance::{self, ConstraintStatus};
use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn dogfood_architecture_yaml_loads_successfully() {
    let yaml = include_str!("../../../design/architecture.yaml");
    let doc = interchange::parse_yaml(yaml).unwrap();

    let warnings = interchange::validate_document(&doc).unwrap();
    assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    let nodes = store.get_all_nodes(version).unwrap();
    assert!(
        nodes.len() >= 25,
        "expected at least 25 nodes, got {}",
        nodes.len()
    );

    let edges = store.get_all_edges(version, None).unwrap();
    assert!(
        edges.len() >= 20,
        "expected at least 20 edges, got {}",
        edges.len()
    );

    let constraints = store.get_constraints(version).unwrap();
    assert!(
        constraints.len() >= 5,
        "expected at least 5 constraints, got {}",
        constraints.len()
    );
}

#[test]
fn dogfood_conformance_all_must_not_depend_pass() {
    let yaml = include_str!("../../../design/architecture.yaml");
    let doc = interchange::parse_yaml(yaml).unwrap();
    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    let report = conformance::evaluate_design(&store, version).unwrap();

    // All must_not_depend constraints should pass
    let must_not_depend_results: Vec<_> = report
        .constraint_results
        .iter()
        .filter(|r| r.constraint_kind == "must_not_depend")
        .collect();

    assert!(
        !must_not_depend_results.is_empty(),
        "should have must_not_depend constraints"
    );

    for result in &must_not_depend_results {
        assert_eq!(
            result.status,
            ConstraintStatus::Pass,
            "constraint '{}' should pass but got {:?} with violations: {:?}",
            result.constraint_name,
            result.status,
            result.violations
        );
    }

    // Non-evaluable constraints should be marked as such
    let not_evaluable: Vec<_> = report
        .constraint_results
        .iter()
        .filter(|r| r.status == ConstraintStatus::NotEvaluable)
        .collect();

    // boundary, must_contain, max_fan_in are not evaluable in design-only mode
    assert!(
        !not_evaluable.is_empty(),
        "should have some not-evaluable constraints"
    );

    // No failures
    assert_eq!(report.summary.failed, 0);
}

#[test]
fn dogfood_conformance_report_serialises_to_json() {
    let yaml = include_str!("../../../design/architecture.yaml");
    let doc = interchange::parse_yaml(yaml).unwrap();
    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    let report = conformance::evaluate_design(&store, version).unwrap();
    let json = serde_json::to_string_pretty(&report).unwrap();

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["design_version"].is_number());
    assert!(parsed["summary"]["passed"].is_number());
}
