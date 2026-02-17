//! Mapping from language-qualified names to canonical paths.

use std::collections::HashMap;

use svt_core::canonical::to_kebab_case;
use svt_core::model::*;
use uuid::Uuid;

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

/// UUID v5 namespace for deterministic ID generation.
const SVT_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Convert a Rust qualified name to a canonical path.
///
/// Splits on `::`, applies `to_kebab_case` to each segment,
/// joins with `/`, prepends `/`.
#[must_use]
pub fn qualified_name_to_canonical(qualified_name: &str) -> String {
    let segments: Vec<String> = qualified_name.split("::").map(to_kebab_case).collect();
    format!("/{}", segments.join("/"))
}

/// Generate a deterministic node ID from a canonical path.
fn node_id(canonical_path: &str) -> String {
    Uuid::new_v5(&SVT_NAMESPACE, canonical_path.as_bytes()).to_string()
}

/// Generate a deterministic edge ID from source path, target path, and kind.
fn edge_id(source_path: &str, target_path: &str, kind: EdgeKind) -> String {
    let input = format!("{}->{}:{:?}", source_path, target_path, kind);
    Uuid::new_v5(&SVT_NAMESPACE, input.as_bytes()).to_string()
}

/// Map analysis items and relations to graph nodes and edges.
///
/// Pure function: no I/O, no store access. Converts qualified names
/// to canonical paths, generates deterministic IDs, and builds
/// containment edges from parent relationships.
pub fn map_to_graph(
    items: &[AnalysisItem],
    relations: &[AnalysisRelation],
) -> (Vec<Node>, Vec<Edge>, Vec<AnalysisWarning>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut warnings = Vec::new();

    // Build qualified_name -> canonical_path lookup
    let mut qn_to_cp: HashMap<&str, String> = HashMap::new();

    for item in items {
        let cp = qualified_name_to_canonical(&item.qualified_name);
        qn_to_cp.insert(&item.qualified_name, cp.clone());

        let name = cp.rsplit('/').next().unwrap_or(&cp).to_string();

        nodes.push(Node {
            id: node_id(&cp),
            canonical_path: cp.clone(),
            qualified_name: Some(item.qualified_name.clone()),
            kind: item.kind,
            sub_kind: item.sub_kind.clone(),
            name,
            language: Some(item.language.clone()),
            provenance: Provenance::Analysis,
            source_ref: Some(item.source_ref.clone()),
            metadata: None,
        });

        // Generate Contains edge from parent
        if let Some(parent_qn) = &item.parent_qualified_name {
            let parent_cp = qualified_name_to_canonical(parent_qn);
            edges.push(Edge {
                id: edge_id(&parent_cp, &cp, EdgeKind::Contains),
                source: node_id(&parent_cp),
                target: node_id(&cp),
                kind: EdgeKind::Contains,
                provenance: Provenance::Analysis,
                metadata: None,
            });
        }
    }

    // Map relations to edges
    for rel in relations {
        let source_cp = match qn_to_cp.get(rel.source_qualified_name.as_str()) {
            Some(cp) => cp.clone(),
            None => {
                warnings.push(AnalysisWarning {
                    source_ref: String::new(),
                    message: format!(
                        "unresolvable relation source: {}",
                        rel.source_qualified_name
                    ),
                });
                continue;
            }
        };
        let target_cp = match qn_to_cp.get(rel.target_qualified_name.as_str()) {
            Some(cp) => cp.clone(),
            None => {
                warnings.push(AnalysisWarning {
                    source_ref: String::new(),
                    message: format!(
                        "unresolvable relation target: {}",
                        rel.target_qualified_name
                    ),
                });
                continue;
            }
        };

        edges.push(Edge {
            id: edge_id(&source_cp, &target_cp, rel.kind),
            source: node_id(&source_cp),
            target: node_id(&target_cp),
            kind: rel.kind,
            provenance: Provenance::Analysis,
            metadata: None,
        });
    }

    (nodes, edges, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use svt_core::model::{EdgeKind, NodeKind};

    fn make_item(
        qualified_name: &str,
        kind: NodeKind,
        sub_kind: &str,
        parent: Option<&str>,
    ) -> AnalysisItem {
        AnalysisItem {
            qualified_name: qualified_name.to_string(),
            kind,
            sub_kind: sub_kind.to_string(),
            parent_qualified_name: parent.map(|s| s.to_string()),
            source_ref: "test.rs:1".to_string(),
            language: "rust".to_string(),
        }
    }

    #[test]
    fn maps_crate_name_to_canonical_path() {
        let items = vec![make_item("svt_core", NodeKind::Service, "crate", None)];
        let (nodes, _, _) = map_to_graph(&items, &[]);
        assert_eq!(nodes[0].canonical_path, "/svt-core");
    }

    #[test]
    fn maps_nested_module_to_canonical_path() {
        let items = vec![
            make_item("svt_core", NodeKind::Service, "crate", None),
            make_item(
                "svt_core::model",
                NodeKind::Component,
                "module",
                Some("svt_core"),
            ),
        ];
        let (nodes, _, _) = map_to_graph(&items, &[]);
        let model = nodes.iter().find(|n| n.canonical_path == "/svt-core/model");
        assert!(
            model.is_some(),
            "should map svt_core::model to /svt-core/model"
        );
    }

    #[test]
    fn maps_pascal_case_struct_to_kebab() {
        let items = vec![
            make_item("svt_core", NodeKind::Service, "crate", None),
            make_item(
                "svt_core::CozoStore",
                NodeKind::Unit,
                "struct",
                Some("svt_core"),
            ),
        ];
        let (nodes, _, _) = map_to_graph(&items, &[]);
        let cs = nodes
            .iter()
            .find(|n| n.canonical_path == "/svt-core/cozo-store");
        assert!(cs.is_some(), "CozoStore should map to /svt-core/cozo-store");
    }

    #[test]
    fn generates_contains_edges_from_parent() {
        let items = vec![
            make_item("my_crate", NodeKind::Service, "crate", None),
            make_item("my_crate::Foo", NodeKind::Unit, "struct", Some("my_crate")),
        ];
        let (_, edges, _) = map_to_graph(&items, &[]);
        let contains: Vec<_> = edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Contains)
            .collect();
        assert_eq!(contains.len(), 1, "should have 1 Contains edge");
    }

    #[test]
    fn maps_depends_relation_to_edge() {
        let items = vec![
            make_item("a", NodeKind::Service, "crate", None),
            make_item("b", NodeKind::Service, "crate", None),
        ];
        let relations = vec![AnalysisRelation {
            source_qualified_name: "a".to_string(),
            target_qualified_name: "b".to_string(),
            kind: EdgeKind::Depends,
        }];
        let (_, edges, _) = map_to_graph(&items, &relations);
        let depends: Vec<_> = edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Depends)
            .collect();
        assert_eq!(depends.len(), 1);
    }

    #[test]
    fn unresolvable_relation_produces_warning() {
        let items = vec![make_item("a", NodeKind::Service, "crate", None)];
        let relations = vec![AnalysisRelation {
            source_qualified_name: "a".to_string(),
            target_qualified_name: "nonexistent".to_string(),
            kind: EdgeKind::Depends,
        }];
        let (_, _, warnings) = map_to_graph(&items, &relations);
        assert!(
            !warnings.is_empty(),
            "should warn about unresolvable target"
        );
    }

    #[test]
    fn ids_are_deterministic() {
        let items = vec![make_item("a", NodeKind::Service, "crate", None)];
        let (nodes1, _, _) = map_to_graph(&items, &[]);
        let (nodes2, _, _) = map_to_graph(&items, &[]);
        assert_eq!(
            nodes1[0].id, nodes2[0].id,
            "same input should produce same ID"
        );
    }

    #[test]
    fn qualified_name_to_canonical_basic_cases() {
        assert_eq!(qualified_name_to_canonical("svt_core"), "/svt-core");
        assert_eq!(
            qualified_name_to_canonical("svt_core::model"),
            "/svt-core/model"
        );
        assert_eq!(
            qualified_name_to_canonical("svt_core::model::Node"),
            "/svt-core/model/node"
        );
        assert_eq!(
            qualified_name_to_canonical("svt_core::store::CozoStore"),
            "/svt-core/store/cozo-store"
        );
    }
}
