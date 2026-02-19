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

/// Extension point for constraint evaluation.
///
/// Implement this trait to add custom constraint kinds beyond the built-in set.
/// Each evaluator handles a single constraint kind string.
pub trait ConstraintEvaluator: Send + Sync {
    /// The constraint kind string this evaluator handles.
    fn kind(&self) -> &str;
    /// Evaluate a constraint against the store data.
    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult>;
}

/// Built-in evaluator for `must_not_depend` constraints.
#[derive(Debug)]
pub struct MustNotDependEvaluator;

impl ConstraintEvaluator for MustNotDependEvaluator {
    fn kind(&self) -> &str {
        "must_not_depend"
    }
    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_must_not_depend(store, constraint, eval_version)
    }
}

/// Built-in evaluator for `boundary` constraints.
#[derive(Debug)]
pub struct BoundaryEvaluator;

impl ConstraintEvaluator for BoundaryEvaluator {
    fn kind(&self) -> &str {
        "boundary"
    }
    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_boundary(store, constraint, eval_version)
    }
}

/// Built-in evaluator for `must_contain` constraints.
#[derive(Debug)]
pub struct MustContainEvaluator;

impl ConstraintEvaluator for MustContainEvaluator {
    fn kind(&self) -> &str {
        "must_contain"
    }
    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_must_contain(store, constraint, eval_version)
    }
}

/// Built-in evaluator for `max_fan_in` constraints.
#[derive(Debug)]
pub struct MaxFanInEvaluator;

impl ConstraintEvaluator for MaxFanInEvaluator {
    fn kind(&self) -> &str {
        "max_fan_in"
    }
    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_max_fan_in(store, constraint, eval_version)
    }
}

/// Registry of constraint evaluators, keyed by constraint kind string.
///
/// Use [`ConstraintRegistry::with_defaults`] to create a registry pre-populated
/// with all built-in evaluators, or [`ConstraintRegistry::new`] for an empty one.
pub struct ConstraintRegistry {
    evaluators: std::collections::HashMap<String, Box<dyn ConstraintEvaluator>>,
}

impl ConstraintRegistry {
    /// Create an empty registry with no evaluators.
    #[must_use]
    pub fn new() -> Self {
        Self {
            evaluators: std::collections::HashMap::new(),
        }
    }

    /// Create a registry pre-populated with all built-in evaluators.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(MustNotDependEvaluator));
        registry.register(Box::new(BoundaryEvaluator));
        registry.register(Box::new(MustContainEvaluator));
        registry.register(Box::new(MaxFanInEvaluator));
        registry
    }

    /// Register a constraint evaluator. Replaces any existing evaluator for the same kind.
    pub fn register(&mut self, evaluator: Box<dyn ConstraintEvaluator>) {
        self.evaluators
            .insert(evaluator.kind().to_string(), evaluator);
    }

    /// Look up an evaluator by constraint kind.
    #[must_use]
    pub fn get(&self, kind: &str) -> Option<&dyn ConstraintEvaluator> {
        self.evaluators.get(kind).map(|b| b.as_ref())
    }

    /// Return all registered constraint kind strings.
    #[must_use]
    pub fn kinds(&self) -> Vec<String> {
        self.evaluators.keys().cloned().collect()
    }
}

impl Default for ConstraintRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Evaluate a single `must_not_depend` constraint.
///
/// Finds all nodes matching `scope`, all nodes matching `target`,
/// and checks for `Depends` edges between them.
pub fn evaluate_constraint_must_not_depend(
    store: &dyn GraphStore,
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

/// Evaluate a `boundary` constraint (scope_only access mode).
///
/// Checks that no node outside the scope pattern has a `Depends` edge
/// targeting a node inside the scope. Internal-to-internal deps are allowed.
pub fn evaluate_constraint_boundary(
    store: &dyn GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let all_nodes = store.get_all_nodes(version)?;
    let depends_edges = store.get_all_edges(version, Some(EdgeKind::Depends))?;

    let mut scoped_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut id_to_path: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut id_to_source_ref: std::collections::HashMap<&str, Option<&str>> =
        std::collections::HashMap::new();

    for node in &all_nodes {
        id_to_path.insert(&node.id, &node.canonical_path);
        id_to_source_ref.insert(&node.id, node.source_ref.as_deref());
        if canonical_path_matches(&node.canonical_path, &constraint.scope) {
            scoped_ids.insert(&node.id);
        }
    }

    let mut violations = Vec::new();
    for edge in &depends_edges {
        let target_in_scope = scoped_ids.contains(edge.target.as_str());
        let source_in_scope = scoped_ids.contains(edge.source.as_str());

        // Violation: external source depends on internal target
        if target_in_scope && !source_in_scope {
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

/// Evaluate a `must_contain` constraint.
///
/// Checks that the scope node has at least one child matching
/// `params.child_pattern` (by name) and optionally `params.child_kind` (by NodeKind).
pub fn evaluate_constraint_must_contain(
    store: &dyn GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let scope_node = store.get_node_by_path(version, &constraint.scope)?;

    let scope_node = match scope_node {
        Some(n) => n,
        None => {
            return Ok(ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("Scope node '{}' not found", constraint.scope),
                violations: vec![],
            });
        }
    };

    let children = store.get_children(version, &scope_node.id)?;

    let child_pattern = constraint
        .params
        .as_ref()
        .and_then(|p| p.get("child_pattern"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let child_kind: Option<NodeKind> = constraint
        .params
        .as_ref()
        .and_then(|p| p.get("child_kind"))
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());

    let has_match = children.iter().any(|child| {
        let name_matches = child.name == child_pattern;
        let kind_matches = child_kind.map(|k| child.kind == k).unwrap_or(true);
        name_matches && kind_matches
    });

    if has_match {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Pass,
            severity: constraint.severity,
            message: constraint.message.clone(),
            violations: vec![],
        })
    } else {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Fail,
            severity: constraint.severity,
            message: constraint.message.clone(),
            violations: vec![Violation {
                source_path: constraint.scope.clone(),
                target_path: None,
                edge_id: None,
                edge_kind: None,
                source_ref: None,
            }],
        })
    }
}

/// Evaluate a `max_fan_in` constraint.
///
/// Counts incoming edges of the specified kind to the scope node.
/// If `params.level` is specified, only counts edges from nodes of that `NodeKind`.
/// Fails if the count exceeds `params.limit`.
pub fn evaluate_constraint_max_fan_in(
    store: &dyn GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let scope_node = store.get_node_by_path(version, &constraint.scope)?;

    let scope_node = match scope_node {
        Some(n) => n,
        None => {
            return Ok(ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("Scope node '{}' not found", constraint.scope),
                violations: vec![],
            });
        }
    };

    let params = constraint.params.as_ref();

    let edge_kind_str = params
        .and_then(|p| p.get("edge_kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("depends");

    let edge_kind_filter: Option<EdgeKind> =
        serde_json::from_str(&format!("\"{}\"", edge_kind_str)).ok();

    let limit = params
        .and_then(|p| p.get("limit"))
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::MAX) as usize;

    let level_filter: Option<NodeKind> = params
        .and_then(|p| p.get("level"))
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok());

    // Get incoming edges of the specified kind
    let incoming = store.get_edges(
        version,
        &scope_node.id,
        Direction::Incoming,
        edge_kind_filter,
    )?;

    // Apply level filter if specified
    let fan_in = if let Some(level) = level_filter {
        incoming
            .iter()
            .filter(|edge| {
                store
                    .get_node(version, &edge.source)
                    .ok()
                    .flatten()
                    .map(|n| n.kind == level)
                    .unwrap_or(false)
            })
            .count()
    } else {
        incoming.len()
    };

    if fan_in <= limit {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Pass,
            severity: constraint.severity,
            message: constraint.message.clone(),
            violations: vec![],
        })
    } else {
        Ok(ConstraintResult {
            constraint_name: constraint.name.clone(),
            constraint_kind: constraint.kind.clone(),
            status: ConstraintStatus::Fail,
            severity: constraint.severity,
            message: format!(
                "{} (fan-in: {}, limit: {})",
                constraint.message, fan_in, limit
            ),
            violations: vec![Violation {
                source_path: constraint.scope.clone(),
                target_path: None,
                edge_id: None,
                edge_kind: edge_kind_filter,
                source_ref: None,
            }],
        })
    }
}

/// Run structural checks (containment acyclicity and referential integrity) on a version.
fn structural_checks(store: &dyn GraphStore, version: Version) -> Result<Vec<ConstraintResult>> {
    let mut results = Vec::new();

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

    Ok(results)
}

/// Evaluate constraints from `constraint_version` against data in `eval_version`.
fn evaluate_constraints(
    store: &dyn GraphStore,
    constraint_version: Version,
    eval_version: Version,
) -> Result<Vec<ConstraintResult>> {
    let mut results = Vec::new();
    let constraints = store.get_constraints(constraint_version)?;
    for constraint in &constraints {
        let result = match constraint.kind.as_str() {
            "must_not_depend" => {
                evaluate_constraint_must_not_depend(store, constraint, eval_version)?
            }
            "boundary" => evaluate_constraint_boundary(store, constraint, eval_version)?,
            "must_contain" => evaluate_constraint_must_contain(store, constraint, eval_version)?,
            "max_fan_in" => evaluate_constraint_max_fan_in(store, constraint, eval_version)?,
            _ => ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("{} not evaluable", constraint.kind),
                violations: vec![],
            },
        };
        results.push(result);
    }
    Ok(results)
}

/// Compute summary counts from constraint results and unmatched node counts.
fn compute_summary(
    results: &[ConstraintResult],
    unimplemented: usize,
    undocumented: usize,
) -> ConformanceSummary {
    ConformanceSummary {
        passed: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Pass)
            .count(),
        failed: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Fail && r.severity == Severity::Error)
            .count(),
        warned: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Fail && r.severity == Severity::Warning)
            .count(),
        not_evaluable: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::NotEvaluable)
            .count(),
        unimplemented,
        undocumented,
    }
}

/// Evaluate a design version: structural checks + constraint evaluation.
///
/// Design-only mode: no analysis version. Non-evaluable constraints
/// (e.g., must_contain without analysis data) are marked `NotEvaluable`.
pub fn evaluate_design(store: &impl GraphStore, version: Version) -> Result<ConformanceReport> {
    let mut results = structural_checks(store, version)?;
    results.extend(evaluate_constraints(store, version, version)?);
    let summary = compute_summary(&results, 0, 0);

    Ok(ConformanceReport {
        design_version: version,
        analysis_version: None,
        constraint_results: results,
        unimplemented: vec![],
        undocumented: vec![],
        summary,
    })
}

/// Evaluate conformance between a design version and an analysis version.
///
/// Compares prescribed architecture against discovered architecture:
/// 1. Finds unimplemented nodes (in design but not in analysis)
/// 2. Finds undocumented nodes (in analysis but not in design, at matching depth)
/// 3. Evaluates all constraints against analysis edges
pub fn evaluate(
    store: &impl GraphStore,
    design_version: Version,
    analysis_version: Version,
) -> Result<ConformanceReport> {
    let design_nodes = store.get_all_nodes(design_version)?;
    let analysis_nodes = store.get_all_nodes(analysis_version)?;

    let design_paths: std::collections::HashSet<&str> = design_nodes
        .iter()
        .map(|n| n.canonical_path.as_str())
        .collect();
    let analysis_paths: std::collections::HashSet<&str> = analysis_nodes
        .iter()
        .map(|n| n.canonical_path.as_str())
        .collect();

    // Unimplemented: design nodes not found in analysis
    // Depth tolerance: a design node is "implemented" if any analysis node
    // has it as a prefix (is a descendant)
    let mut unimplemented = Vec::new();
    for node in &design_nodes {
        let path = &node.canonical_path;
        let has_match = analysis_paths.contains(path.as_str());
        let has_descendant = analysis_paths.iter().any(|ap| {
            ap.starts_with(path.as_str())
                && ap.len() > path.len()
                && ap.as_bytes()[path.len()] == b'/'
        });
        if !has_match && !has_descendant {
            unimplemented.push(UnmatchedNode {
                canonical_path: node.canonical_path.clone(),
                kind: node.kind,
                name: node.name.clone(),
            });
        }
    }

    // Undocumented: analysis nodes not in design, filtered to design depth
    let max_design_depth = design_nodes
        .iter()
        .map(|n| n.canonical_path.matches('/').count())
        .max()
        .unwrap_or(0);

    let mut undocumented = Vec::new();
    for node in &analysis_nodes {
        let depth = node.canonical_path.matches('/').count();
        if depth <= max_design_depth && !design_paths.contains(node.canonical_path.as_str()) {
            undocumented.push(UnmatchedNode {
                canonical_path: node.canonical_path.clone(),
                kind: node.kind,
                name: node.name.clone(),
            });
        }
    }

    // Structural checks + constraints
    let mut results = structural_checks(store, analysis_version)?;
    results.extend(evaluate_constraints(
        store,
        design_version,
        analysis_version,
    )?);
    let summary = compute_summary(&results, unimplemented.len(), undocumented.len());

    Ok(ConformanceReport {
        design_version,
        analysis_version: Some(analysis_version),
        constraint_results: results,
        unimplemented,
        undocumented,
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

        // must_contain should Fail (scope node exists but has no matching child)
        let mc = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "cli-has-main")
            .unwrap();
        assert_eq!(mc.status, ConstraintStatus::Fail);
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

    #[test]
    fn evaluate_finds_unimplemented_design_nodes() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Create design with 2 nodes
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d2", "/app/missing", NodeKind::Service, "crate"),
            )
            .unwrap();

        // Create analysis with only 1 matching node
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store
            .add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace"))
            .unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        assert!(
            !report.unimplemented.is_empty(),
            "should report /app/missing as unimplemented"
        );
        assert!(report
            .unimplemented
            .iter()
            .any(|n| n.canonical_path == "/app/missing"));
    }

    #[test]
    fn evaluate_finds_undocumented_analysis_nodes() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Design has /app and /app/core
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();

        // Analysis has /app, /app/core, and /app/extra (undocumented, same depth as design)
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store
            .add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                av,
                &make_node("a2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();
        store
            .add_node(
                av,
                &make_node("a3", "/app/extra", NodeKind::Service, "crate"),
            )
            .unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        assert!(report.analysis_version.is_some());
        // /app/extra should be flagged as undocumented (same depth as design, not child of design node)
        assert!(
            report
                .undocumented
                .iter()
                .any(|n| n.canonical_path == "/app/extra"),
            "should find /app/extra as undocumented, got: {:?}",
            report.undocumented
        );
    }

    #[test]
    fn evaluate_depth_tolerance_design_node_with_descendants() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Design has /app and /app/core
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();

        // Analysis has /app, /app/core, and /app/core/model (deeper)
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store
            .add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                av,
                &make_node("a2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();
        store
            .add_node(
                av,
                &make_node("a3", "/app/core/model", NodeKind::Component, "module"),
            )
            .unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        assert!(
            report.unimplemented.is_empty(),
            "all design nodes have matches, none should be unimplemented: {:?}",
            report.unimplemented
        );
    }

    #[test]
    fn evaluate_runs_constraints_against_analysis() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Load design
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();
        store
            .add_node(dv, &make_node("d3", "/app/cli", NodeKind::Service, "crate"))
            .unwrap();
        store
            .add_constraint(
                dv,
                &Constraint {
                    id: "c1".to_string(),
                    kind: "must_not_depend".to_string(),
                    name: "core-no-cli".to_string(),
                    scope: "/app/core/**".to_string(),
                    target: Some("/app/cli/**".to_string()),
                    params: None,
                    message: "Core must not depend on CLI".to_string(),
                    severity: Severity::Error,
                },
            )
            .unwrap();

        // Create analysis with a forbidden dependency
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store
            .add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                av,
                &make_node("a2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();
        store
            .add_node(av, &make_node("a3", "/app/cli", NodeKind::Service, "crate"))
            .unwrap();
        // Forbidden: core depends on cli
        store
            .add_edge(av, &make_edge("ae1", "a2", "a3", EdgeKind::Depends))
            .unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        let core_constraint = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "core-no-cli")
            .unwrap();
        assert_eq!(
            core_constraint.status,
            ConstraintStatus::Fail,
            "constraint should fail against analysis edges"
        );
    }

    #[test]
    fn summary_separates_failed_and_warned() {
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
      - canonical_path: /app/web
        kind: service
edges:
  - source: /app/core
    target: /app/cli
    kind: depends
  - source: /app/core
    target: /app/web
    kind: depends
constraints:
  - name: core-no-cli
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
  - name: core-no-web
    kind: must_not_depend
    scope: /app/core/**
    target: /app/web/**
    message: "Core should not depend on web"
    severity: warning
"#,
        );
        let report = evaluate_design(&store, version).unwrap();

        // Both must_not_depend constraints should fail
        let core_no_cli = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "core-no-cli")
            .unwrap();
        assert_eq!(core_no_cli.status, ConstraintStatus::Fail);

        let core_no_web = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "core-no-web")
            .unwrap();
        assert_eq!(core_no_web.status, ConstraintStatus::Fail);

        // failed should only count error-severity failures
        assert_eq!(
            report.summary.failed, 1,
            "failed should only count error-severity failures"
        );
        // warned should only count warning-severity failures
        assert_eq!(
            report.summary.warned, 1,
            "warned should only count warning-severity failures"
        );
    }

    #[test]
    fn evaluate_empty_analysis_reports_all_design_as_unimplemented() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Design has 2 nodes
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();

        // Analysis is empty
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        assert_eq!(
            report.unimplemented.len(),
            2,
            "all design nodes should be unimplemented when analysis is empty"
        );
    }

    #[test]
    fn evaluate_both_empty_produces_clean_report() {
        let mut store = CozoStore::new_in_memory().unwrap();

        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        assert!(report.unimplemented.is_empty());
        assert!(report.undocumented.is_empty());
        assert_eq!(
            report.summary.passed, 2,
            "structural checks should pass on empty graph"
        );
        assert_eq!(report.summary.failed, 0);
    }

    #[test]
    fn evaluate_summary_counts_are_correct() {
        let mut store = CozoStore::new_in_memory().unwrap();

        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d3", "/app/missing", NodeKind::Service, "crate"),
            )
            .unwrap();

        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store
            .add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                av,
                &make_node("a2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();
        store
            .add_node(
                av,
                &make_node("a3", "/app/extra", NodeKind::Service, "crate"),
            )
            .unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        assert_eq!(report.summary.unimplemented, 1, "/app/missing");
        assert_eq!(report.summary.undocumented, 1, "/app/extra");
        assert!(report.summary.passed >= 2, "structural checks should pass");
    }

    // --- boundary constraint tests ---

    #[test]
    fn boundary_passes_when_no_external_deps_on_internal() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let app = make_node("n1", "/app", NodeKind::System, "workspace");
        let store_mod = make_node("n2", "/app/store", NodeKind::Component, "module");
        let store_cozo = make_node("n3", "/app/store/cozo", NodeKind::Component, "module");
        let api = make_node("n4", "/app/api", NodeKind::Component, "module");
        store.add_node(v, &app).unwrap();
        store.add_node(v, &store_mod).unwrap();
        store.add_node(v, &store_cozo).unwrap();
        store.add_node(v, &api).unwrap();

        // api depends on store (allowed -- store is not inside boundary)
        store
            .add_edge(v, &make_edge("e1", "n4", "n2", EdgeKind::Depends))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "boundary".to_string(),
            name: "store-encapsulation".to_string(),
            scope: "/app/store/cozo/**".to_string(),
            target: None,
            params: Some(serde_json::json!({"access": "scope_only"})),
            message: "CozoDB internals must not leak".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_boundary(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Pass);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn boundary_fails_when_external_depends_on_internal() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let app = make_node("n1", "/app", NodeKind::System, "workspace");
        let store_mod = make_node("n2", "/app/store", NodeKind::Component, "module");
        let store_cozo = make_node("n3", "/app/store/cozo", NodeKind::Component, "module");
        let api = make_node("n4", "/app/api", NodeKind::Component, "module");
        store.add_node(v, &app).unwrap();
        store.add_node(v, &store_mod).unwrap();
        store.add_node(v, &store_cozo).unwrap();
        store.add_node(v, &api).unwrap();

        // api depends on store/cozo (FORBIDDEN -- inside boundary)
        store
            .add_edge(v, &make_edge("e1", "n4", "n3", EdgeKind::Depends))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "boundary".to_string(),
            name: "store-encapsulation".to_string(),
            scope: "/app/store/cozo/**".to_string(),
            target: None,
            params: Some(serde_json::json!({"access": "scope_only"})),
            message: "CozoDB internals must not leak".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_boundary(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Fail);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].source_path, "/app/api");
        assert_eq!(
            result.violations[0].target_path,
            Some("/app/store/cozo".to_string())
        );
    }

    #[test]
    fn boundary_allows_internal_to_internal_deps() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let app = make_node("n1", "/app", NodeKind::System, "workspace");
        let cozo = make_node("n2", "/app/store/cozo", NodeKind::Component, "module");
        let cozo_inner = make_node("n3", "/app/store/cozo/queries", NodeKind::Unit, "module");
        store.add_node(v, &app).unwrap();
        store.add_node(v, &cozo).unwrap();
        store.add_node(v, &cozo_inner).unwrap();

        // internal depends on internal (allowed)
        store
            .add_edge(v, &make_edge("e1", "n3", "n2", EdgeKind::Depends))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "boundary".to_string(),
            name: "store-encapsulation".to_string(),
            scope: "/app/store/cozo/**".to_string(),
            target: None,
            params: Some(serde_json::json!({"access": "scope_only"})),
            message: "CozoDB internals must not leak".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_boundary(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Pass);
    }

    // --- must_contain constraint tests ---

    #[test]
    fn must_contain_passes_when_child_exists() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let cli = make_node("n1", "/app/cli", NodeKind::Service, "crate");
        let cmds = make_node("n2", "/app/cli/commands", NodeKind::Component, "module");
        let check = make_node("n3", "/app/cli/commands/check", NodeKind::Unit, "function");
        store.add_node(v, &cli).unwrap();
        store.add_node(v, &cmds).unwrap();
        store.add_node(v, &check).unwrap();
        // Containment edges
        store
            .add_edge(v, &make_edge("e1", "n1", "n2", EdgeKind::Contains))
            .unwrap();
        store
            .add_edge(v, &make_edge("e2", "n2", "n3", EdgeKind::Contains))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "must_contain".to_string(),
            name: "cli-has-check".to_string(),
            scope: "/app/cli/commands".to_string(),
            target: None,
            params: Some(serde_json::json!({"child_pattern": "check", "child_kind": "unit"})),
            message: "CLI must have check command".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_must_contain(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Pass);
    }

    #[test]
    fn must_contain_fails_when_child_missing() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let cli = make_node("n1", "/app/cli", NodeKind::Service, "crate");
        let cmds = make_node("n2", "/app/cli/commands", NodeKind::Component, "module");
        store.add_node(v, &cli).unwrap();
        store.add_node(v, &cmds).unwrap();
        store
            .add_edge(v, &make_edge("e1", "n1", "n2", EdgeKind::Contains))
            .unwrap();
        // No children of /app/cli/commands

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "must_contain".to_string(),
            name: "cli-has-check".to_string(),
            scope: "/app/cli/commands".to_string(),
            target: None,
            params: Some(serde_json::json!({"child_pattern": "check", "child_kind": "unit"})),
            message: "CLI must have check command".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_must_contain(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Fail);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn must_contain_fails_when_name_matches_but_kind_does_not() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let cmds = make_node("n1", "/app/cli/commands", NodeKind::Component, "module");
        // Child named "check" but kind is Component, not Unit
        let check = make_node(
            "n2",
            "/app/cli/commands/check",
            NodeKind::Component,
            "module",
        );
        store.add_node(v, &cmds).unwrap();
        store.add_node(v, &check).unwrap();
        store
            .add_edge(v, &make_edge("e1", "n1", "n2", EdgeKind::Contains))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "must_contain".to_string(),
            name: "cli-has-check".to_string(),
            scope: "/app/cli/commands".to_string(),
            target: None,
            params: Some(serde_json::json!({"child_pattern": "check", "child_kind": "unit"})),
            message: "CLI must have check command".to_string(),
            severity: Severity::Error,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_must_contain(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Fail);
    }

    // --- max_fan_in constraint tests ---

    #[test]
    fn max_fan_in_passes_when_under_limit() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let model = make_node("n1", "/app/model", NodeKind::Component, "module");
        let api = make_node("n2", "/app/api", NodeKind::Component, "module");
        store.add_node(v, &model).unwrap();
        store.add_node(v, &api).unwrap();

        // 1 incoming depends edge (limit is 5)
        store
            .add_edge(v, &make_edge("e1", "n2", "n1", EdgeKind::Depends))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "max_fan_in".to_string(),
            name: "model-fan-in".to_string(),
            scope: "/app/model".to_string(),
            target: None,
            params: Some(serde_json::json!({"edge_kind": "depends", "limit": 5})),
            message: "Model fan-in should be reasonable".to_string(),
            severity: Severity::Warning,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_max_fan_in(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Pass);
    }

    #[test]
    fn max_fan_in_fails_when_over_limit() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let model = make_node("n1", "/app/model", NodeKind::Component, "module");
        let a = make_node("n2", "/app/a", NodeKind::Component, "module");
        let b = make_node("n3", "/app/b", NodeKind::Component, "module");
        let c = make_node("n4", "/app/c", NodeKind::Component, "module");
        store.add_node(v, &model).unwrap();
        store.add_node(v, &a).unwrap();
        store.add_node(v, &b).unwrap();
        store.add_node(v, &c).unwrap();

        // 3 incoming depends edges (limit is 2)
        store
            .add_edge(v, &make_edge("e1", "n2", "n1", EdgeKind::Depends))
            .unwrap();
        store
            .add_edge(v, &make_edge("e2", "n3", "n1", EdgeKind::Depends))
            .unwrap();
        store
            .add_edge(v, &make_edge("e3", "n4", "n1", EdgeKind::Depends))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "max_fan_in".to_string(),
            name: "model-fan-in".to_string(),
            scope: "/app/model".to_string(),
            target: None,
            params: Some(serde_json::json!({"edge_kind": "depends", "limit": 2})),
            message: "Model fan-in too high".to_string(),
            severity: Severity::Warning,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_max_fan_in(&store, &constraint, v).unwrap();
        assert_eq!(result.status, ConstraintStatus::Fail);
        assert_eq!(result.violations.len(), 1);
    }

    #[test]
    fn max_fan_in_with_level_filter_counts_only_matching_kind() {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

        let model = make_node("n1", "/app/model", NodeKind::Component, "module");
        let api = make_node("n2", "/app/api", NodeKind::Component, "module");
        let handler = make_node("n3", "/app/api/handler", NodeKind::Unit, "function");
        store.add_node(v, &model).unwrap();
        store.add_node(v, &api).unwrap();
        store.add_node(v, &handler).unwrap();

        // 2 incoming edges, but only 1 from component level
        store
            .add_edge(v, &make_edge("e1", "n2", "n1", EdgeKind::Depends))
            .unwrap();
        store
            .add_edge(v, &make_edge("e2", "n3", "n1", EdgeKind::Depends))
            .unwrap();

        let constraint = Constraint {
            id: "c1".to_string(),
            kind: "max_fan_in".to_string(),
            name: "model-fan-in".to_string(),
            scope: "/app/model".to_string(),
            target: None,
            params: Some(
                serde_json::json!({"edge_kind": "depends", "limit": 1, "level": "component"}),
            ),
            message: "Model fan-in at component level".to_string(),
            severity: Severity::Warning,
        };
        store.add_constraint(v, &constraint).unwrap();

        let result = evaluate_constraint_max_fan_in(&store, &constraint, v).unwrap();
        // Only 1 component-level edge, limit is 1, so it passes
        assert_eq!(result.status, ConstraintStatus::Pass);
    }

    #[test]
    fn constraint_evaluator_trait_returns_correct_kind() {
        let evaluator = MustNotDependEvaluator;
        assert_eq!(evaluator.kind(), "must_not_depend");
        let evaluator = BoundaryEvaluator;
        assert_eq!(evaluator.kind(), "boundary");
        let evaluator = MustContainEvaluator;
        assert_eq!(evaluator.kind(), "must_contain");
        let evaluator = MaxFanInEvaluator;
        assert_eq!(evaluator.kind(), "max_fan_in");
    }

    #[test]
    fn constraint_registry_with_defaults_has_all_built_ins() {
        let registry = ConstraintRegistry::with_defaults();
        assert!(registry.get("must_not_depend").is_some());
        assert!(registry.get("boundary").is_some());
        assert!(registry.get("must_contain").is_some());
        assert!(registry.get("max_fan_in").is_some());
        assert!(registry.get("unknown_kind").is_none());
        let mut kinds = registry.kinds();
        kinds.sort();
        assert_eq!(
            kinds,
            vec!["boundary", "max_fan_in", "must_contain", "must_not_depend"]
        );
    }

    #[test]
    fn constraint_registry_register_adds_evaluator() {
        let mut registry = ConstraintRegistry::new();
        assert!(registry.get("must_not_depend").is_none());
        registry.register(Box::new(MustNotDependEvaluator));
        assert!(registry.get("must_not_depend").is_some());
    }

    #[test]
    fn evaluate_design_node_with_only_descendant_is_implemented() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Design has /app/core but analysis only has /app/core/model (descendant)
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store
            .add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        store
            .add_node(
                dv,
                &make_node("d2", "/app/core", NodeKind::Service, "crate"),
            )
            .unwrap();

        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store
            .add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace"))
            .unwrap();
        // No exact /app/core — but has a descendant
        store
            .add_node(
                av,
                &make_node("a3", "/app/core/model", NodeKind::Component, "module"),
            )
            .unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        // /app/core should NOT be unimplemented due to depth tolerance
        assert!(
            !report
                .unimplemented
                .iter()
                .any(|n| n.canonical_path == "/app/core"),
            "/app/core should be considered implemented via descendant, unimplemented: {:?}",
            report.unimplemented
        );
    }
}
