//! Conformance evaluation: constraint checking and report generation.
//!
//! Feature-gated behind `store`.

use serde::{Deserialize, Serialize};

use crate::canonical::canonical_path_matches;
use crate::model::*;
use crate::store::{GraphStore, Result};

/// Status of a single constraint evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintStatus {
    /// Constraint satisfied.
    Pass,
    /// Constraint violated.
    Fail,
    /// Cannot be evaluated (e.g., needs analysis data).
    NotEvaluable,
}

/// A single violation found during constraint evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// Canonical path of the violating source node.
    pub source_path: String,
    /// Canonical path of the forbidden target (if applicable).
    pub target_path: Option<String>,
    /// ID of the violating edge.
    pub edge_id: Option<String>,
    /// Kind of the violating edge.
    pub edge_kind: Option<EdgeKind>,
    /// Source reference (file path, line number, or URL) from the source node.
    pub source_ref: Option<String>,
}

/// Result of evaluating a single constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintResult {
    /// Name of the constraint.
    pub constraint_name: String,
    /// Kind of the constraint (e.g., "must_not_depend").
    pub constraint_kind: String,
    /// Pass, Fail, or NotEvaluable.
    pub status: ConstraintStatus,
    /// Severity from the constraint definition.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
    /// Specific violations found (empty if pass).
    pub violations: Vec<Violation>,
}

/// A node that is unmatched between design and analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmatchedNode {
    /// Canonical path.
    pub canonical_path: String,
    /// Node kind.
    pub kind: NodeKind,
    /// Human-readable name.
    pub name: String,
}

/// Summary counts for a conformance report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConformanceSummary {
    /// Constraints that passed.
    pub passed: usize,
    /// Constraints that failed.
    pub failed: usize,
    /// Constraints that produced warnings.
    pub warned: usize,
    /// Constraints that could not be evaluated.
    pub not_evaluable: usize,
    /// Design nodes not found in analysis.
    pub unimplemented: usize,
    /// Analysis nodes not found in design.
    pub undocumented: usize,
}

/// Full conformance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// Design version evaluated.
    pub design_version: Version,
    /// Analysis version (None in design-only mode).
    pub analysis_version: Option<Version>,
    /// Results for each constraint.
    pub constraint_results: Vec<ConstraintResult>,
    /// Design nodes not found in analysis.
    pub unimplemented: Vec<UnmatchedNode>,
    /// Analysis nodes not found in design.
    pub undocumented: Vec<UnmatchedNode>,
    /// Summary counts.
    pub summary: ConformanceSummary,
}

/// Evaluate a single `must_not_depend` constraint.
///
/// Finds all nodes matching `scope`, all nodes matching `target`,
/// and checks for `Depends` edges between them.
pub fn evaluate_constraint_must_not_depend(
    store: &impl GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let all_nodes = store.get_all_nodes(version)?;
    let depends_edges = store.get_all_edges(version, Some(EdgeKind::Depends))?;

    let target_pattern = constraint.target.as_deref().unwrap_or("");

    // Build sets of node IDs matching scope and target patterns
    let mut scope_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut target_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut id_to_path: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();

    for node in &all_nodes {
        id_to_path.insert(&node.id, &node.canonical_path);
        if canonical_path_matches(&node.canonical_path, &constraint.scope) {
            scope_ids.insert(&node.id);
        }
        if canonical_path_matches(&node.canonical_path, target_pattern) {
            target_ids.insert(&node.id);
        }
    }

    // Build ID→source_ref mapping for violation reporting
    let mut id_to_source_ref: std::collections::HashMap<&str, Option<&str>> =
        std::collections::HashMap::new();
    for node in &all_nodes {
        id_to_source_ref.insert(&node.id, node.source_ref.as_deref());
    }

    // Find forbidden edges: scope node depends on target node
    let mut violations = Vec::new();
    for edge in &depends_edges {
        if scope_ids.contains(edge.source.as_str()) && target_ids.contains(edge.target.as_str()) {
            violations.push(Violation {
                source_path: id_to_path
                    .get(edge.source.as_str())
                    .unwrap_or(&"")
                    .to_string(),
                target_path: Some(
                    id_to_path
                        .get(edge.target.as_str())
                        .unwrap_or(&"")
                        .to_string(),
                ),
                edge_id: Some(edge.id.clone()),
                edge_kind: Some(edge.kind),
                source_ref: id_to_source_ref
                    .get(edge.source.as_str())
                    .copied()
                    .flatten()
                    .map(|s| s.to_string()),
            });
        }
    }

    let status = if violations.is_empty() {
        ConstraintStatus::Pass
    } else {
        ConstraintStatus::Fail
    };

    Ok(ConstraintResult {
        constraint_name: constraint.name.clone(),
        constraint_kind: constraint.kind.clone(),
        status,
        severity: constraint.severity,
        message: constraint.message.clone(),
        violations,
    })
}

/// Evaluate a design version: structural checks + constraint evaluation.
///
/// Design-only mode: no analysis version. Non-evaluable constraints
/// (e.g., must_contain without analysis data) are marked `NotEvaluable`.
pub fn evaluate_design(store: &impl GraphStore, version: Version) -> Result<ConformanceReport> {
    let mut results = Vec::new();

    // Structural check: containment acyclicity
    let cycles = crate::validation::validate_contains_acyclic(store, version)?;
    results.push(ConstraintResult {
        constraint_name: "containment-acyclic".to_string(),
        constraint_kind: "structural".to_string(),
        status: if cycles.is_empty() {
            ConstraintStatus::Pass
        } else {
            ConstraintStatus::Fail
        },
        severity: Severity::Error,
        message: if cycles.is_empty() {
            "Containment hierarchy is acyclic".to_string()
        } else {
            format!("Found {} cycle(s) in containment hierarchy", cycles.len())
        },
        violations: cycles
            .iter()
            .map(|c| Violation {
                source_path: c.node_ids.first().cloned().unwrap_or_default(),
                target_path: c.node_ids.last().cloned(),
                edge_id: None,
                edge_kind: Some(EdgeKind::Contains),
                source_ref: None,
            })
            .collect(),
    });

    // Structural check: referential integrity
    let integrity_errors = crate::validation::validate_referential_integrity(store, version)?;
    results.push(ConstraintResult {
        constraint_name: "referential-integrity".to_string(),
        constraint_kind: "structural".to_string(),
        status: if integrity_errors.is_empty() {
            ConstraintStatus::Pass
        } else {
            ConstraintStatus::Fail
        },
        severity: Severity::Error,
        message: if integrity_errors.is_empty() {
            "All edge references are valid".to_string()
        } else {
            format!(
                "Found {} referential integrity error(s)",
                integrity_errors.len()
            )
        },
        violations: integrity_errors
            .iter()
            .map(|e| Violation {
                source_path: e.missing_node_id.clone(),
                target_path: None,
                edge_id: Some(e.edge_id.clone()),
                edge_kind: None,
                source_ref: None,
            })
            .collect(),
    });

    // Evaluate each constraint
    let constraints = store.get_constraints(version)?;
    for constraint in &constraints {
        let result = match constraint.kind.as_str() {
            "must_not_depend" => evaluate_constraint_must_not_depend(store, constraint, version)?,
            _ => ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("{} not evaluable (design-only mode)", constraint.kind),
                violations: vec![],
            },
        };
        results.push(result);
    }

    // Compute summary
    let summary = ConformanceSummary {
        passed: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Pass)
            .count(),
        failed: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Fail)
            .count(),
        warned: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Fail && r.severity == Severity::Warning)
            .count(),
        not_evaluable: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::NotEvaluable)
            .count(),
        unimplemented: 0,
        undocumented: 0,
    };

    Ok(ConformanceReport {
        design_version: version,
        analysis_version: None,
        constraint_results: results,
        unimplemented: vec![],
        undocumented: vec![],
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::CozoStore;

    fn make_node(id: &str, path: &str, kind: NodeKind, sub_kind: &str) -> Node {
        Node {
            id: id.to_string(),
            canonical_path: path.to_string(),
            qualified_name: None,
            kind,
            sub_kind: sub_kind.to_string(),
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            language: None,
            provenance: Provenance::Design,
            source_ref: None,
            metadata: None,
        }
    }

    fn make_edge(id: &str, source: &str, target: &str, kind: EdgeKind) -> Edge {
        Edge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            kind,
            provenance: Provenance::Design,
            metadata: None,
        }
    }

    #[test]
    fn must_not_depend_passes_when_no_violations() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let app = make_node("n1", "/app", NodeKind::System, "workspace");
        let core = make_node("n2", "/app/core", NodeKind::Service, "crate");
        let cli = make_node("n3", "/app/cli", NodeKind::Service, "crate");
        store.add_node(v, &app).unwrap();
        store.add_node(v, &core).unwrap();
        store.add_node(v, &cli).unwrap();

        // cli depends on core (allowed)
        let edge = make_edge("e1", "n3", "n2", EdgeKind::Depends);
        store.add_edge(v, &edge).unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "must_not_depend".to_string(),
            name: "core-no-cli-deps".to_string(),
            scope: "/app/core/**".to_string(),
            target: Some("/app/cli/**".to_string()),
            params: None,
            message: "Core must not depend on CLI".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_must_not_depend(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Pass);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn must_not_depend_fails_with_violation() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let app = make_node("n1", "/app", NodeKind::System, "workspace");
        let core = make_node("n2", "/app/core", NodeKind::Service, "crate");
        let cli = make_node("n3", "/app/cli", NodeKind::Service, "crate");
        store.add_node(v, &app).unwrap();
        store.add_node(v, &core).unwrap();
        store.add_node(v, &cli).unwrap();

        // core depends on cli (FORBIDDEN)
        let edge = make_edge("e1", "n2", "n3", EdgeKind::Depends);
        store.add_edge(v, &edge).unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "must_not_depend".to_string(),
            name: "core-no-cli-deps".to_string(),
            scope: "/app/core/**".to_string(),
            target: Some("/app/cli/**".to_string()),
            params: None,
            message: "Core must not depend on CLI".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_must_not_depend(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Fail);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].source_path, "/app/core");
        assert_eq!(
            result.violations[0].target_path,
            Some("/app/cli".to_string())
        );
    }

    fn load_test_doc(yaml: &str) -> (CozoStore, Version) {
        use crate::interchange::parse_yaml;
        use crate::interchange_store::load_into_store;

        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();
        (store, version)
    }

    #[test]
    fn evaluate_design_reports_all_constraints() {
        let (store, version) = load_test_doc(
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
  - name: core-no-cli-deps
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
  - name: cli-has-main
    kind: must_contain
    scope: /app/cli
    message: "CLI must contain main"
    severity: warning
"#,
        );
        let report = evaluate_design(&store, version).unwrap();
        // 2 structural + 2 constraints
        assert_eq!(report.constraint_results.len(), 4);
        assert_eq!(report.design_version, version);
        assert!(report.analysis_version.is_none());

        // must_not_depend should pass
        let mnd = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "core-no-cli-deps")
            .unwrap();
        assert_eq!(mnd.status, ConstraintStatus::Pass);

        // must_contain should be NotEvaluable
        let mc = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "cli-has-main")
            .unwrap();
        assert_eq!(mc.status, ConstraintStatus::NotEvaluable);
    }

    #[test]
    fn evaluate_design_summary_counts() {
        let (store, version) = load_test_doc(
            r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
edges: []
constraints:
  - name: core-no-outward
    kind: must_not_depend
    scope: /app/core/**
    target: /app/**
    message: "Core stays clean"
    severity: error
"#,
        );
        let report = evaluate_design(&store, version).unwrap();
        // 2 structural (both pass) + 1 must_not_depend (pass)
        assert_eq!(report.summary.passed, 3);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.summary.not_evaluable, 0);
    }
}
