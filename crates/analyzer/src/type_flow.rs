//! Type-flow analysis pass.
//!
//! Examines function signature metadata and trait implementations to infer
//! `transforms` and `data_flow` edges between types and modules. This pass
//! is language-agnostic — it only reads metadata that any language parser
//! can produce.
//!
//! ## Phase B: From/Into Transform Detection
//!
//! Scans items for `"trait": "From"` / `"trait": "TryFrom"` metadata and
//! functions where input type differs from output type (both project-local).
//! Emits [`EdgeKind::Transforms`] edges.
//!
//! ## Phase C: Cross-Module Data Flow
//!
//! Walks [`EdgeKind::Calls`] edges across module boundaries, comparing
//! parameter and return types to detect data movement. Emits
//! [`EdgeKind::DataFlow`] edges between parent modules.

use std::collections::{HashMap, HashSet};

use svt_core::analysis::{AnalysisItem, AnalysisRelation, ParseResult};
use svt_core::model::EdgeKind;

/// A function's resolved type signature, extracted from item metadata.
#[derive(Debug, Clone)]
struct FunctionSignature {
    /// Qualified name of the function.
    qualified_name: String,
    /// Parameter types: `(param_name, type_qualified_name)`.
    param_types: Vec<(String, String)>,
    /// Return type qualified name, if present.
    return_type: Option<String>,
    /// Qualified name of the parent module.
    parent_module: String,
}

/// A deduplication key for data flow edges.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DataFlowKey {
    source_module: String,
    target_module: String,
    source_type: String,
    target_type: String,
}

/// Type-flow analysis pass.
///
/// Examines function signature metadata and trait implementations to infer
/// `transforms` and `data_flow` edges between types and modules.
pub struct TypeFlowAnalysis {
    /// Function signatures indexed by qualified name.
    signatures: HashMap<String, FunctionSignature>,
    /// Items with From/TryFrom trait metadata: `(parent_qn, impl_for, trait_name)`.
    from_impls: Vec<(String, String, String)>,
    /// Calls edges from parse results.
    calls: Vec<(String, String)>,
}

impl TypeFlowAnalysis {
    /// Build the analysis from all parsed results.
    ///
    /// Indexes function signatures, From/TryFrom trait implementations, and
    /// call edges from the combined parse results.
    #[must_use]
    pub fn from_parse_results(results: &[ParseResult]) -> Self {
        let mut signatures = HashMap::new();
        let mut from_impls = Vec::new();
        let mut calls = Vec::new();

        for result in results {
            // Index function signatures from item metadata.
            for item in &result.items {
                if !is_function_like(&item.sub_kind) {
                    // Check for From/TryFrom trait metadata on any item.
                    if let Some(ref meta) = item.metadata {
                        if let Some(trait_name) = meta.get("trait").and_then(|v| v.as_str()) {
                            if trait_name == "From" || trait_name == "TryFrom" {
                                if let Some(impl_for) =
                                    meta.get("impl_for").and_then(|v| v.as_str())
                                {
                                    if let Some(ref parent) = item.parent_qualified_name {
                                        from_impls.push((
                                            parent.clone(),
                                            impl_for.to_string(),
                                            trait_name.to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }

                // Also check function items for From/TryFrom trait metadata.
                if let Some(ref meta) = item.metadata {
                    if let Some(trait_name) = meta.get("trait").and_then(|v| v.as_str()) {
                        if trait_name == "From" || trait_name == "TryFrom" {
                            if let Some(impl_for) = meta.get("impl_for").and_then(|v| v.as_str()) {
                                if let Some(ref parent) = item.parent_qualified_name {
                                    from_impls.push((
                                        parent.clone(),
                                        impl_for.to_string(),
                                        trait_name.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }

                if let Some(sig) = extract_signature(item) {
                    signatures.insert(sig.qualified_name.clone(), sig);
                }
            }

            // Collect calls edges.
            for rel in &result.relations {
                if rel.kind == EdgeKind::Calls {
                    calls.push((
                        rel.source_qualified_name.clone(),
                        rel.target_qualified_name.clone(),
                    ));
                }
            }
        }

        Self {
            signatures,
            from_impls,
            calls,
        }
    }

    /// Return the number of function signatures indexed.
    #[must_use]
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    /// Check if a qualified name has a signature in the index.
    #[must_use]
    pub fn has_signature(&self, qualified_name: &str) -> bool {
        self.signatures.contains_key(qualified_name)
    }

    /// Run the analysis, returning new relations to add to the graph.
    ///
    /// Produces [`EdgeKind::Transforms`] edges (Phase B) and
    /// [`EdgeKind::DataFlow`] edges (Phase C).
    #[must_use]
    pub fn analyze(&self) -> Vec<AnalysisRelation> {
        let mut relations = Vec::new();

        // Phase B: From/Into transform detection.
        self.detect_from_transforms(&mut relations);
        self.detect_signature_transforms(&mut relations);

        // Phase C: Cross-module data flow from call chains.
        self.detect_cross_module_data_flow(&mut relations);

        relations
    }

    /// Phase B, part 1: Detect transforms from From/TryFrom implementations.
    ///
    /// When an item has metadata `"trait": "From"` and `"impl_for": "B"`,
    /// it means `impl From<A> for B` where A is the parent type. Emit a
    /// `Transforms` edge from A to B.
    fn detect_from_transforms(&self, relations: &mut Vec<AnalysisRelation>) {
        let mut seen = HashSet::new();
        for (source_type, target_type, _trait_name) in &self.from_impls {
            let key = (source_type.clone(), target_type.clone());
            if seen.insert(key) {
                relations.push(AnalysisRelation {
                    source_qualified_name: source_type.clone(),
                    target_qualified_name: target_type.clone(),
                    kind: EdgeKind::Transforms,
                });
            }
        }
    }

    /// Phase B, part 2: Detect transforms from function signatures.
    ///
    /// When a function's parameter type differs from its return type, and both
    /// are project-local (contain `::`), emit a `Transforms` edge.
    /// Skip getter/accessor functions (heuristic: name starts with `get_`,
    /// `is_`, `has_` and takes only `&self`).
    fn detect_signature_transforms(&self, relations: &mut Vec<AnalysisRelation>) {
        let mut seen = HashSet::new();
        for sig in self.signatures.values() {
            // Skip getters/accessors.
            if is_getter_function(&sig.qualified_name, &sig.param_types) {
                continue;
            }

            let return_type = match sig.return_type {
                Some(ref rt) => rt,
                None => continue,
            };

            // Only consider project-local types (those containing `::`)
            if !is_project_local(return_type) {
                continue;
            }

            for (_param_name, param_type) in &sig.param_types {
                if !is_project_local(param_type) {
                    continue;
                }
                if param_type == return_type {
                    continue;
                }
                let key = (param_type.clone(), return_type.clone());
                if seen.insert(key) {
                    relations.push(AnalysisRelation {
                        source_qualified_name: param_type.clone(),
                        target_qualified_name: return_type.clone(),
                        kind: EdgeKind::Transforms,
                    });
                }
            }
        }
    }

    /// Phase C: Detect cross-module data flow from call chains.
    ///
    /// For each `calls` edge where caller and callee are in different modules,
    /// check if the caller's return type matches the callee's parameter type
    /// (or vice versa). If so, emit a `DataFlow` edge between the parent modules.
    fn detect_cross_module_data_flow(&self, relations: &mut Vec<AnalysisRelation>) {
        let mut seen = HashSet::<DataFlowKey>::new();

        for (caller_qn, callee_qn) in &self.calls {
            let caller_sig = match self.signatures.get(caller_qn) {
                Some(s) => s,
                None => continue,
            };
            let callee_sig = match self.signatures.get(callee_qn) {
                Some(s) => s,
                None => continue,
            };

            // Only cross-module calls.
            if caller_sig.parent_module == callee_sig.parent_module {
                continue;
            }

            // Check: caller's return type matches callee's param type (push direction).
            if let Some(ref caller_return) = caller_sig.return_type {
                for (_param_name, callee_param) in &callee_sig.param_types {
                    if caller_return == callee_param {
                        let key = DataFlowKey {
                            source_module: caller_sig.parent_module.clone(),
                            target_module: callee_sig.parent_module.clone(),
                            source_type: caller_return.clone(),
                            target_type: callee_param.clone(),
                        };
                        if seen.insert(key) {
                            relations.push(AnalysisRelation {
                                source_qualified_name: caller_sig.parent_module.clone(),
                                target_qualified_name: callee_sig.parent_module.clone(),
                                kind: EdgeKind::DataFlow,
                            });
                        }
                    }
                }
            }

            // Check: callee's return type matches caller's param type (pull direction).
            if let Some(ref callee_return) = callee_sig.return_type {
                for (_param_name, caller_param) in &caller_sig.param_types {
                    if callee_return == caller_param {
                        let key = DataFlowKey {
                            source_module: callee_sig.parent_module.clone(),
                            target_module: caller_sig.parent_module.clone(),
                            source_type: callee_return.clone(),
                            target_type: caller_param.clone(),
                        };
                        if seen.insert(key) {
                            relations.push(AnalysisRelation {
                                source_qualified_name: callee_sig.parent_module.clone(),
                                target_qualified_name: caller_sig.parent_module.clone(),
                                kind: EdgeKind::DataFlow,
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Check if an item's sub_kind indicates a function or method.
fn is_function_like(sub_kind: &str) -> bool {
    sub_kind == "function" || sub_kind == "method"
}

/// Extract a [`FunctionSignature`] from an [`AnalysisItem`]'s metadata.
///
/// Reads `param_types` and `return_type` fields from the JSON metadata.
/// Returns `None` if the item has no metadata or no type information.
fn extract_signature(item: &AnalysisItem) -> Option<FunctionSignature> {
    let meta = item.metadata.as_ref()?;

    let param_types_val = meta.get("param_types");
    let return_type_val = meta.get("return_type");

    // Need at least one of param_types or return_type.
    if param_types_val.is_none() && return_type_val.is_none() {
        return None;
    }

    let param_types = param_types_val
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let name = entry.get("name")?.as_str()?.to_string();
                    let type_name = entry.get("type")?.as_str()?.to_string();
                    Some((name, type_name))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let return_type = return_type_val.and_then(|v| v.as_str()).map(String::from);

    // Derive the parent module from the qualified name.
    let parent_module = item
        .parent_qualified_name
        .clone()
        .or_else(|| derive_parent_module(&item.qualified_name))
        .unwrap_or_default();

    Some(FunctionSignature {
        qualified_name: item.qualified_name.clone(),
        param_types,
        return_type,
        parent_module,
    })
}

/// Derive the parent module from a qualified name by stripping the last segment.
///
/// `"my_crate::module::function"` → `"my_crate::module"`.
fn derive_parent_module(qualified_name: &str) -> Option<String> {
    let pos = qualified_name.rfind("::")?;
    Some(qualified_name[..pos].to_string())
}

/// Check if a type is project-local (contains `::`, indicating a qualified path).
fn is_project_local(type_name: &str) -> bool {
    type_name.contains("::")
}

/// Heuristic: check if a function is a getter/accessor.
///
/// A function is considered a getter if its short name starts with `get_`, `is_`,
/// or `has_` and it takes only one parameter (typically `&self`).
fn is_getter_function(qualified_name: &str, param_types: &[(String, String)]) -> bool {
    let short_name = qualified_name.rsplit("::").next().unwrap_or(qualified_name);
    let is_getter_name = short_name.starts_with("get_")
        || short_name.starts_with("is_")
        || short_name.starts_with("has_");
    // Getters typically take only &self (which is not in param_types since it's
    // not a project-local type) — so param_types is empty or has at most one entry.
    is_getter_name && param_types.len() <= 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use svt_core::analysis::{AnalysisItem, AnalysisRelation, ParseResult};
    use svt_core::model::{EdgeKind, NodeKind};

    /// Helper to create a function item with type metadata.
    fn make_function_item(
        qualified_name: &str,
        parent: Option<&str>,
        param_types: &[(&str, &str)],
        return_type: Option<&str>,
        extra_meta: Option<serde_json::Value>,
    ) -> AnalysisItem {
        let mut meta = extra_meta.unwrap_or_else(|| serde_json::json!({}));
        let obj = meta.as_object_mut().expect("meta must be object");

        if !param_types.is_empty() {
            let params: Vec<serde_json::Value> = param_types
                .iter()
                .map(|(name, ty)| serde_json::json!({"name": name, "type": ty}))
                .collect();
            obj.insert("param_types".to_string(), serde_json::Value::Array(params));
        }
        if let Some(rt) = return_type {
            obj.insert(
                "return_type".to_string(),
                serde_json::Value::String(rt.to_string()),
            );
        }

        AnalysisItem {
            qualified_name: qualified_name.to_string(),
            kind: NodeKind::Unit,
            sub_kind: "function".to_string(),
            parent_qualified_name: parent.map(String::from),
            source_ref: "test.rs:1".to_string(),
            language: "rust".to_string(),
            metadata: Some(meta),
            tags: vec![],
        }
    }

    /// Helper to create a method item with From trait metadata.
    fn make_from_impl_method(
        qualified_name: &str,
        parent: &str,
        impl_for: &str,
        trait_name: &str,
    ) -> AnalysisItem {
        AnalysisItem {
            qualified_name: qualified_name.to_string(),
            kind: NodeKind::Unit,
            sub_kind: "method".to_string(),
            parent_qualified_name: Some(parent.to_string()),
            source_ref: "test.rs:1".to_string(),
            language: "rust".to_string(),
            metadata: Some(serde_json::json!({
                "trait": trait_name,
                "impl_for": impl_for,
            })),
            tags: vec![],
        }
    }

    // ---- Phase B: From/Into transform detection ----

    #[test]
    fn from_impl_emits_transforms_edge() {
        let result = ParseResult {
            items: vec![make_from_impl_method(
                "my_crate::TypeA::from",
                "my_crate::TypeA",
                "my_crate::TypeB",
                "From",
            )],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(transforms.len(), 1, "should emit one Transforms edge");
        assert_eq!(transforms[0].source_qualified_name, "my_crate::TypeA");
        assert_eq!(transforms[0].target_qualified_name, "my_crate::TypeB");
    }

    #[test]
    fn try_from_impl_emits_transforms_edge() {
        let result = ParseResult {
            items: vec![make_from_impl_method(
                "my_crate::Source::try_from",
                "my_crate::Source",
                "my_crate::Target",
                "TryFrom",
            )],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(
            transforms.len(),
            1,
            "should emit one Transforms edge for TryFrom"
        );
        assert_eq!(transforms[0].source_qualified_name, "my_crate::Source");
        assert_eq!(transforms[0].target_qualified_name, "my_crate::Target");
    }

    #[test]
    fn function_with_different_input_output_types_emits_transforms() {
        let result = ParseResult {
            items: vec![make_function_item(
                "my_crate::convert",
                Some("my_crate"),
                &[("input", "my_crate::InputType")],
                Some("my_crate::OutputType"),
                None,
            )],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].source_qualified_name, "my_crate::InputType");
        assert_eq!(transforms[0].target_qualified_name, "my_crate::OutputType");
    }

    #[test]
    fn function_with_same_input_output_type_no_transforms() {
        let result = ParseResult {
            items: vec![make_function_item(
                "my_crate::identity",
                Some("my_crate"),
                &[("input", "my_crate::SameType")],
                Some("my_crate::SameType"),
                None,
            )],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert!(
            transforms.is_empty(),
            "same input/output type should not emit Transforms"
        );
    }

    #[test]
    fn getter_functions_skipped() {
        let result = ParseResult {
            items: vec![
                make_function_item(
                    "my_crate::MyStruct::get_name",
                    Some("my_crate::MyStruct"),
                    &[],
                    Some("my_crate::Name"),
                    None,
                ),
                make_function_item(
                    "my_crate::MyStruct::is_valid",
                    Some("my_crate::MyStruct"),
                    &[],
                    Some("my_crate::Status"),
                    None,
                ),
                make_function_item(
                    "my_crate::MyStruct::has_children",
                    Some("my_crate::MyStruct"),
                    &[],
                    Some("my_crate::Children"),
                    None,
                ),
            ],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert!(
            transforms.is_empty(),
            "getter functions should not emit Transforms edges"
        );
    }

    #[test]
    fn non_project_local_types_skipped() {
        let result = ParseResult {
            items: vec![make_function_item(
                "my_crate::parse",
                Some("my_crate"),
                &[("input", "String")],
                Some("my_crate::ParsedResult"),
                None,
            )],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert!(
            transforms.is_empty(),
            "non-project-local param type (no ::) should not emit Transforms"
        );
    }

    #[test]
    fn duplicate_from_impls_deduplicated() {
        let result = ParseResult {
            items: vec![
                make_from_impl_method(
                    "my_crate::TypeA::from",
                    "my_crate::TypeA",
                    "my_crate::TypeB",
                    "From",
                ),
                // Duplicate From impl (e.g., from different files or parse passes).
                make_from_impl_method(
                    "my_crate::TypeA::from_2",
                    "my_crate::TypeA",
                    "my_crate::TypeB",
                    "From",
                ),
            ],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(
            transforms.len(),
            1,
            "duplicate From impls should be deduplicated"
        );
    }

    // ---- Phase C: Cross-module data flow ----

    #[test]
    fn cross_module_call_with_matching_types_emits_data_flow() {
        let result = ParseResult {
            items: vec![
                make_function_item(
                    "my_crate::module_a::produce",
                    Some("my_crate::module_a"),
                    &[],
                    Some("my_crate::SharedType"),
                    None,
                ),
                make_function_item(
                    "my_crate::module_b::consume",
                    Some("my_crate::module_b"),
                    &[("data", "my_crate::SharedType")],
                    None,
                    None,
                ),
            ],
            relations: vec![AnalysisRelation {
                source_qualified_name: "my_crate::module_a::produce".to_string(),
                target_qualified_name: "my_crate::module_b::consume".to_string(),
                kind: EdgeKind::Calls,
            }],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let data_flows: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::DataFlow)
            .collect();
        assert_eq!(data_flows.len(), 1, "should emit one DataFlow edge");
        assert_eq!(data_flows[0].source_qualified_name, "my_crate::module_a");
        assert_eq!(data_flows[0].target_qualified_name, "my_crate::module_b");
    }

    #[test]
    fn same_module_call_no_data_flow() {
        let result = ParseResult {
            items: vec![
                make_function_item(
                    "my_crate::module_a::produce",
                    Some("my_crate::module_a"),
                    &[],
                    Some("my_crate::SharedType"),
                    None,
                ),
                make_function_item(
                    "my_crate::module_a::consume",
                    Some("my_crate::module_a"),
                    &[("data", "my_crate::SharedType")],
                    None,
                    None,
                ),
            ],
            relations: vec![AnalysisRelation {
                source_qualified_name: "my_crate::module_a::produce".to_string(),
                target_qualified_name: "my_crate::module_a::consume".to_string(),
                kind: EdgeKind::Calls,
            }],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let data_flows: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::DataFlow)
            .collect();
        assert!(
            data_flows.is_empty(),
            "same-module calls should not emit DataFlow"
        );
    }

    #[test]
    fn data_flow_edges_deduplicated() {
        let result = ParseResult {
            items: vec![
                make_function_item(
                    "my_crate::module_a::produce1",
                    Some("my_crate::module_a"),
                    &[],
                    Some("my_crate::SharedType"),
                    None,
                ),
                make_function_item(
                    "my_crate::module_a::produce2",
                    Some("my_crate::module_a"),
                    &[],
                    Some("my_crate::SharedType"),
                    None,
                ),
                make_function_item(
                    "my_crate::module_b::consume1",
                    Some("my_crate::module_b"),
                    &[("data", "my_crate::SharedType")],
                    None,
                    None,
                ),
                make_function_item(
                    "my_crate::module_b::consume2",
                    Some("my_crate::module_b"),
                    &[("data", "my_crate::SharedType")],
                    None,
                    None,
                ),
            ],
            relations: vec![
                AnalysisRelation {
                    source_qualified_name: "my_crate::module_a::produce1".to_string(),
                    target_qualified_name: "my_crate::module_b::consume1".to_string(),
                    kind: EdgeKind::Calls,
                },
                AnalysisRelation {
                    source_qualified_name: "my_crate::module_a::produce2".to_string(),
                    target_qualified_name: "my_crate::module_b::consume2".to_string(),
                    kind: EdgeKind::Calls,
                },
            ],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let data_flows: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::DataFlow)
            .collect();
        assert_eq!(
            data_flows.len(),
            1,
            "multiple calls between same modules with same types should produce single DataFlow edge"
        );
    }

    #[test]
    fn cross_module_call_no_type_match_no_data_flow() {
        let result = ParseResult {
            items: vec![
                make_function_item(
                    "my_crate::module_a::produce",
                    Some("my_crate::module_a"),
                    &[],
                    Some("my_crate::TypeA"),
                    None,
                ),
                make_function_item(
                    "my_crate::module_b::consume",
                    Some("my_crate::module_b"),
                    &[("data", "my_crate::TypeB")],
                    None,
                    None,
                ),
            ],
            relations: vec![AnalysisRelation {
                source_qualified_name: "my_crate::module_a::produce".to_string(),
                target_qualified_name: "my_crate::module_b::consume".to_string(),
                kind: EdgeKind::Calls,
            }],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let data_flows: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::DataFlow)
            .collect();
        assert!(
            data_flows.is_empty(),
            "non-matching types across modules should not emit DataFlow"
        );
    }

    #[test]
    fn pull_direction_data_flow_detected() {
        // Callee returns a type that matches caller's param — pull direction.
        let result = ParseResult {
            items: vec![
                make_function_item(
                    "my_crate::module_a::caller",
                    Some("my_crate::module_a"),
                    &[("data", "my_crate::SharedType")],
                    None,
                    None,
                ),
                make_function_item(
                    "my_crate::module_b::provider",
                    Some("my_crate::module_b"),
                    &[],
                    Some("my_crate::SharedType"),
                    None,
                ),
            ],
            relations: vec![AnalysisRelation {
                source_qualified_name: "my_crate::module_a::caller".to_string(),
                target_qualified_name: "my_crate::module_b::provider".to_string(),
                kind: EdgeKind::Calls,
            }],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();

        let data_flows: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::DataFlow)
            .collect();
        assert_eq!(
            data_flows.len(),
            1,
            "should detect pull-direction data flow"
        );
        // Data flows from the provider's module (source of data) to the caller's module.
        assert_eq!(data_flows[0].source_qualified_name, "my_crate::module_b");
        assert_eq!(data_flows[0].target_qualified_name, "my_crate::module_a");
    }

    #[test]
    fn empty_parse_results_produces_no_relations() {
        let analysis = TypeFlowAnalysis::from_parse_results(&[]);
        let relations = analysis.analyze();
        assert!(relations.is_empty());
    }

    #[test]
    fn items_without_metadata_are_skipped() {
        let result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "my_crate::bare_fn".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "function".to_string(),
                parent_qualified_name: Some("my_crate".to_string()),
                source_ref: "test.rs:1".to_string(),
                language: "rust".to_string(),
                metadata: None,
                tags: vec![],
            }],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result]);
        let relations = analysis.analyze();
        assert!(
            relations.is_empty(),
            "items without metadata should produce no relations"
        );
    }

    #[test]
    fn multiple_parse_results_combined() {
        let result1 = ParseResult {
            items: vec![make_from_impl_method(
                "crate_a::TypeA::from",
                "crate_a::TypeA",
                "crate_a::TypeB",
                "From",
            )],
            relations: vec![],
            warnings: vec![],
        };
        let result2 = ParseResult {
            items: vec![make_from_impl_method(
                "crate_b::TypeC::from",
                "crate_b::TypeC",
                "crate_b::TypeD",
                "From",
            )],
            relations: vec![],
            warnings: vec![],
        };

        let analysis = TypeFlowAnalysis::from_parse_results(&[result1, result2]);
        let relations = analysis.analyze();

        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(
            transforms.len(),
            2,
            "should combine results from multiple ParseResults"
        );
    }

    // ---- Helper function tests ----

    #[test]
    fn is_function_like_identifies_functions_and_methods() {
        assert!(is_function_like("function"));
        assert!(is_function_like("method"));
        assert!(!is_function_like("struct"));
        assert!(!is_function_like("module"));
        assert!(!is_function_like("trait"));
    }

    #[test]
    fn is_project_local_requires_double_colon() {
        assert!(is_project_local("my_crate::MyType"));
        assert!(is_project_local("a::b::c"));
        assert!(!is_project_local("String"));
        assert!(!is_project_local("u32"));
    }

    #[test]
    fn derive_parent_module_strips_last_segment() {
        assert_eq!(
            derive_parent_module("my_crate::module::function"),
            Some("my_crate::module".to_string())
        );
        assert_eq!(
            derive_parent_module("my_crate::function"),
            Some("my_crate".to_string())
        );
        assert_eq!(derive_parent_module("bare_name"), None);
    }

    #[test]
    fn is_getter_function_detects_getters() {
        assert!(is_getter_function("my_crate::Foo::get_name", &[]));
        assert!(is_getter_function("my_crate::Foo::is_valid", &[]));
        assert!(is_getter_function("my_crate::Foo::has_children", &[]));
        // With one param, still a getter.
        assert!(is_getter_function(
            "my_crate::Foo::get_value",
            &[("key".to_string(), "my_crate::Key".to_string())]
        ));
        // With two params, not a getter.
        assert!(!is_getter_function(
            "my_crate::Foo::get_complex",
            &[
                ("key".to_string(), "my_crate::Key".to_string()),
                ("default".to_string(), "my_crate::Value".to_string()),
            ]
        ));
        // Non-getter name.
        assert!(!is_getter_function("my_crate::Foo::convert", &[]));
    }
}
