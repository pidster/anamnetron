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
    /// Source language identifier (e.g. `"rust"`, `"go"`), used to apply
    /// language-appropriate locality rules and to prevent cross-language
    /// false matches on bare type names.
    language: String,
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
    /// Index of type-declaration items, keyed by `(language, short_type_name)`
    /// and mapping to every qualified name that declares that short name.
    ///
    /// Non-Rust parsers record bare type names (e.g. `Proposal`) in signature
    /// metadata, but the graph keys nodes by qualified names (`votes::Proposal`).
    /// This index lets [`Self::resolve_type`] qualify a bare name to the item
    /// it refers to so the resulting relation resolves during graph mapping.
    type_index: HashMap<(String, String), Vec<String>>,
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
        let mut type_index: HashMap<(String, String), Vec<String>> = HashMap::new();

        for result in results {
            // Index function signatures from item metadata.
            for item in &result.items {
                // Index type declarations by their short name so bare type names
                // in non-Rust signatures can later be qualified to the item QN.
                if is_type_like(&item.sub_kind) {
                    let short = item
                        .qualified_name
                        .rsplit("::")
                        .next()
                        .unwrap_or(&item.qualified_name)
                        .to_string();
                    let entry = type_index
                        .entry((item.language.clone(), short))
                        .or_default();
                    if !entry.contains(&item.qualified_name) {
                        entry.push(item.qualified_name.clone());
                    }
                }

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
            type_index,
        }
    }

    /// Resolve a type name to the qualified name of the item that declares it.
    ///
    /// Rust type names in signature metadata are already fully qualified
    /// (locality requires a `::` separator), so they are returned unchanged and
    /// the Rust path is left exactly as before.
    ///
    /// Non-Rust parsers emit bare type names (e.g. `Proposal`). Graph mapping
    /// resolves relation endpoints by exact match against item qualified names
    /// (`votes::Proposal`), so a bare name never resolves and the edge is
    /// silently dropped. This looks the bare name up in the type index and
    /// qualifies it.
    ///
    /// `module` is the module the reference occurs in — the module of the
    /// function whose signature is being resolved (see [`Self::resolve_type_unique`]
    /// for the resolution ordering and soundness argument). A bare name that
    /// cannot be uniquely resolved is returned unchanged and dropped at mapping.
    fn resolve_type(&self, type_name: &str, language: &str, module: &str) -> String {
        self.resolve_type_unique(type_name, language, module)
            .unwrap_or_else(|| type_name.to_string())
    }

    /// Resolve a type name to a UNIQUELY-qualified item QN, or `None`.
    ///
    /// Rust names are already fully qualified (returned as-is). Non-Rust bare
    /// names are resolved against the `module` the reference occurs in, using a
    /// same-module-first policy:
    ///
    /// 1. **Same-module exact match** — if the short name is declared *within*
    ///    `module`, use that declaration. This is sound because a module cannot
    ///    declare two types with the same short name, so a same-module match is
    ///    unique *by construction*. A bare reference inside a module denotes that
    ///    module's own type, even when the short name is reused in other modules.
    /// 2. **Global-unique fallback** — otherwise, if the short name is declared
    ///    exactly once across the whole project, use that single declaration
    ///    (the P0 rule; safe because there is only one candidate).
    /// 3. **Otherwise `None`** — zero declarations, or globally ambiguous with no
    ///    same-module match. The name is left bare and dropped at mapping.
    ///
    /// Both non-`None` branches are individually sound, so this is strictly more
    /// recall than the global-unique-only rule with no new unsoundness.
    ///
    /// Callers that compare two resolved types (e.g. cross-module data flow) must
    /// use this and treat `None` as "no match" — never fall back to the bare
    /// name for comparison, or two *distinct* same-named types in different
    /// modules would compare equal and produce a false edge.
    fn resolve_type_unique(&self, type_name: &str, language: &str, module: &str) -> Option<String> {
        if language == "rust" {
            return Some(type_name.to_string());
        }
        let candidates = self
            .type_index
            .get(&(language.to_string(), type_name.to_string()))?;

        // 1. Same-module first: unique by construction (a module declares at most
        //    one type per short name).
        if let Some(qn) = candidates.iter().find(|qn| module_of(qn) == module) {
            return Some(qn.clone());
        }

        // 2. Global-unique fallback (P0 rule).
        match candidates.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
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

            // Only consider project-local types.
            if !is_project_local(return_type, &sig.language) {
                continue;
            }

            for (_param_name, param_type) in &sig.param_types {
                if !is_project_local(param_type, &sig.language) {
                    continue;
                }
                if param_type == return_type {
                    continue;
                }
                // Qualify bare (non-Rust) type names to the item they refer to,
                // so the relation resolves during graph mapping. Both endpoints
                // are referenced from the function's own module, so resolve them
                // against it (same-module-first). Rust names pass through
                // unchanged. Deduplicate on the resolved endpoints.
                let module = module_of(&sig.qualified_name);
                let source = self.resolve_type(param_type, &sig.language, module);
                let target = self.resolve_type(return_type, &sig.language, module);
                let key = (source.clone(), target.clone());
                if seen.insert(key) {
                    relations.push(AnalysisRelation {
                        source_qualified_name: source,
                        target_qualified_name: target,
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

            // Only match within the same language. Non-Rust type names are bare
            // identifiers, so an unrelated `Proposal` in Go and `Proposal` in
            // Python must not be treated as the same data type.
            if caller_sig.language != callee_sig.language {
                continue;
            }

            // A shared data type is a match only when BOTH endpoints resolve to
            // the SAME uniquely-qualified type. Comparing bare names would be
            // unsound: two *distinct* same-named types in different modules
            // (`a::Proposal` vs `b::Proposal`) would compare equal and emit a
            // false DataFlow edge that — unlike Transforms — survives mapping
            // because its endpoints are modules. `resolve_type_unique` returns
            // `None` on ambiguity/unknown, so such pairs never match.
            let lang = &caller_sig.language;

            // Resolve each side's bare type names against the module it is
            // referenced from: the caller's return/params against the caller's
            // module, the callee's params/return against the callee's module.
            // A same-module type therefore binds to that side's own declaration,
            // so two *distinct* same-named types in different modules never
            // compare equal (F1 soundness).
            let caller_module = module_of(&caller_sig.qualified_name);
            let callee_module = module_of(&callee_sig.qualified_name);

            // Check: caller's return type matches callee's param type (push direction).
            if let Some(resolved_return) = caller_sig
                .return_type
                .as_deref()
                .and_then(|rt| self.resolve_type_unique(rt, lang, caller_module))
            {
                for (_param_name, callee_param) in &callee_sig.param_types {
                    if self
                        .resolve_type_unique(callee_param, lang, callee_module)
                        .as_deref()
                        == Some(resolved_return.as_str())
                    {
                        let key = DataFlowKey {
                            source_module: caller_sig.parent_module.clone(),
                            target_module: callee_sig.parent_module.clone(),
                            source_type: resolved_return.clone(),
                            target_type: resolved_return.clone(),
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
            if let Some(resolved_return) = callee_sig
                .return_type
                .as_deref()
                .and_then(|rt| self.resolve_type_unique(rt, lang, callee_module))
            {
                for (_param_name, caller_param) in &caller_sig.param_types {
                    if self
                        .resolve_type_unique(caller_param, lang, caller_module)
                        .as_deref()
                        == Some(resolved_return.as_str())
                    {
                        let key = DataFlowKey {
                            source_module: callee_sig.parent_module.clone(),
                            target_module: caller_sig.parent_module.clone(),
                            source_type: resolved_return.clone(),
                            target_type: resolved_return.clone(),
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

/// Check if an item's sub_kind indicates a type declaration.
///
/// Covers the type-declaration kinds emitted by the language parsers (Rust
/// structs/enums/traits, Go structs, TypeScript/Java classes and interfaces,
/// Python classes, type aliases, records). These are the items a bare type name
/// in a signature can refer to, so they populate the type-resolution index.
fn is_type_like(sub_kind: &str) -> bool {
    matches!(
        sub_kind,
        "struct" | "class" | "interface" | "enum" | "trait" | "record" | "type_alias" | "type"
    )
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
        language: item.language.clone(),
    })
}

/// Return the module portion of a qualified name — everything before the last
/// `::` separator — or `""` when the name has no separator (a top-level item).
///
/// This is the total, borrowing form of [`derive_parent_module`]. All parsers
/// build qualified names with `::` as the segment separator (Go/Java/Python/
/// TypeScript reparent items to `package::…::name`, Rust uses `crate::…::name`),
/// so stripping the last `::`-delimited segment yields the enclosing module.
///
/// `"pkg::sub::Convert"` → `"pkg::sub"`, `"pkg::Convert"` → `"pkg"`,
/// `"Convert"` → `""`.
fn module_of(qualified_name: &str) -> &str {
    match qualified_name.rfind("::") {
        Some(pos) => &qualified_name[..pos],
        None => "",
    }
}

/// Derive the parent module from a qualified name by stripping the last segment.
///
/// `"my_crate::module::function"` → `Some("my_crate::module")`. Returns `None`
/// for a top-level name with no `::` separator (where [`module_of`] returns `""`).
fn derive_parent_module(qualified_name: &str) -> Option<String> {
    match module_of(qualified_name) {
        "" => None,
        module => Some(module.to_string()),
    }
}

/// Check if a type is project-local, using language-appropriate rules.
///
/// The heuristic for locality is separator-aware because different language
/// parsers use different type-name conventions:
///
/// - **Rust** resolves types to fully qualified paths (e.g.
///   `my_crate::model::Node`), so a `::` separator is a reliable signal that the
///   type is a named project/library type rather than a bare primitive. This
///   preserves the original Rust behaviour exactly.
/// - **All other languages** (Go, TypeScript, Java, Python, …) emit bare type
///   identifiers (e.g. `Proposal`). Their parsers filter primitives and
///   standard-library types up front via `build_data_flow_metadata`, so any
///   non-empty type name that survives into a signature is a candidate
///   project-local type.
fn is_project_local(type_name: &str, language: &str) -> bool {
    if language == "rust" {
        type_name.contains("::")
    } else {
        !type_name.trim().is_empty()
    }
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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

    /// Build a non-Rust type-declaration item (struct/class) for a given
    /// language, so tests can exercise the bare-name resolution path.
    fn make_type_item(qualified_name: &str, language: &str) -> AnalysisItem {
        AnalysisItem {
            qualified_name: qualified_name.to_string(),
            kind: NodeKind::Unit,
            sub_kind: "struct".to_string(),
            parent_qualified_name: derive_parent_module(qualified_name),
            source_ref: "test.go:1".to_string(),
            language: language.to_string(),
            metadata: None,
            tags: vec![],
        }
    }

    /// Build a non-Rust function item carrying bare param/return type names.
    fn make_go_function_item(
        qualified_name: &str,
        param_types: &[(&str, &str)],
        return_type: Option<&str>,
    ) -> AnalysisItem {
        let mut item = make_function_item(qualified_name, None, param_types, return_type, None);
        item.language = "go".to_string();
        item
    }

    #[test]
    fn bare_non_rust_type_names_are_qualified_to_the_declaring_item() {
        // A Go function whose bare param/return type names each match exactly
        // one local type declaration. The emitted Transforms edge must be
        // qualified to the declaring items so it resolves during graph mapping.
        let result = ParseResult {
            items: vec![
                make_type_item("pkg::Proposal", "go"),
                make_type_item("pkg::Report", "go"),
                make_go_function_item("pkg::Convert", &[("p", "Proposal")], Some("Report")),
            ],
            relations: vec![],
            warnings: vec![],
            ..Default::default()
        };

        let relations = TypeFlowAnalysis::from_parse_results(&[result]).analyze();
        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].source_qualified_name, "pkg::Proposal");
        assert_eq!(transforms[0].target_qualified_name, "pkg::Report");
    }

    #[test]
    fn ambiguous_bare_type_name_with_no_same_module_match_is_left_unqualified_not_guessed() {
        // Soundness policy: when a bare type name matches MORE THAN ONE local
        // declaration (`Proposal` declared in packages `a` and `b`) AND the
        // referencing function is in NEITHER of those modules (module `c`), there
        // is no same-module match and no global-unique winner, so it must be left
        // bare rather than guessed — the edge is dropped at mapping, never
        // mis-attributed to `a::Proposal` or `b::Proposal`. The unambiguous local
        // `c::Report` still resolves.
        let result = ParseResult {
            items: vec![
                make_type_item("a::Proposal", "go"),
                make_type_item("b::Proposal", "go"),
                make_type_item("c::Report", "go"),
                make_go_function_item("c::Convert", &[("p", "Proposal")], Some("Report")),
            ],
            relations: vec![],
            warnings: vec![],
            ..Default::default()
        };

        let relations = TypeFlowAnalysis::from_parse_results(&[result]).analyze();
        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(transforms.len(), 1);
        // Ambiguous source stays bare (would be dropped by map_to_graph), never
        // guessed to a::Proposal or b::Proposal.
        assert_eq!(transforms[0].source_qualified_name, "Proposal");
        assert!(!transforms[0].source_qualified_name.contains("::"));
        // Unambiguous target is still resolved.
        assert_eq!(transforms[0].target_qualified_name, "c::Report");
    }

    #[test]
    fn locally_unambiguous_type_resolves_via_same_module() {
        // Recall recovery: `Config` is GLOBALLY ambiguous (declared in packages
        // `a` and `b`) but LOCALLY unambiguous inside package `a`. The P0
        // global-unique-only rule dropped this edge; same-module-first resolves
        // the bare `Config` referenced from `a::Convert` to `a::Config` (a module
        // declares at most one `Config`, so the match is unique by construction).
        let result = ParseResult {
            items: vec![
                make_type_item("a::Config", "go"),
                make_type_item("b::Config", "go"),
                make_type_item("a::Report", "go"),
                make_go_function_item("a::Convert", &[("c", "Config")], Some("Report")),
            ],
            relations: vec![],
            warnings: vec![],
            ..Default::default()
        };

        let relations = TypeFlowAnalysis::from_parse_results(&[result]).analyze();
        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(transforms.len(), 1, "recovered Transforms edge expected");
        // The globally-ambiguous source now resolves to the same-module type,
        // never to `b::Config`.
        assert_eq!(transforms[0].source_qualified_name, "a::Config");
        assert_eq!(transforms[0].target_qualified_name, "a::Report");
    }

    #[test]
    fn unknown_bare_type_name_is_left_unqualified() {
        // Zero-match branch: a bare type name with no local declaration is left
        // unchanged (dropped at mapping), exactly as before the resolver existed.
        let result = ParseResult {
            items: vec![
                make_type_item("pkg::Report", "go"),
                make_go_function_item("pkg::Convert", &[("p", "Missing")], Some("Report")),
            ],
            relations: vec![],
            warnings: vec![],
            ..Default::default()
        };

        let relations = TypeFlowAnalysis::from_parse_results(&[result]).analyze();
        let transforms: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Transforms)
            .collect();
        assert_eq!(transforms.len(), 1);
        assert_eq!(transforms[0].source_qualified_name, "Missing");
        assert_eq!(transforms[0].target_qualified_name, "pkg::Report");
    }

    /// Build a `Calls` relation between two qualified names.
    fn make_calls_relation(source: &str, target: &str) -> AnalysisRelation {
        AnalysisRelation {
            source_qualified_name: source.to_string(),
            target_qualified_name: target.to_string(),
            kind: EdgeKind::Calls,
        }
    }

    #[test]
    fn distinct_same_named_types_across_modules_emit_no_false_data_flow() {
        // F1 soundness: package `a` and package `b` each declare a DISTINCT Go
        // type that happens to share the short name `Proposal`. A cross-module
        // call `a::Producer -> b::Consume` where the caller returns `Proposal`
        // and the callee takes `Proposal` must NOT emit a DataFlow edge: the two
        // bare names are ambiguous (resolve to `None`), so they are not the same
        // type. Comparing bare strings would wrongly emit a persistent a->b edge.
        let result = ParseResult {
            items: vec![
                make_type_item("a::Proposal", "go"),
                make_type_item("b::Proposal", "go"),
                make_go_function_item("a::Producer", &[], Some("Proposal")),
                make_go_function_item("b::Consume", &[("p", "Proposal")], None),
            ],
            relations: vec![make_calls_relation("a::Producer", "b::Consume")],
            warnings: vec![],
            ..Default::default()
        };

        let relations = TypeFlowAnalysis::from_parse_results(&[result]).analyze();
        let data_flow: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::DataFlow)
            .collect();
        assert!(
            data_flow.is_empty(),
            "distinct same-named types must not produce a DataFlow edge, got: {data_flow:?}"
        );
    }

    #[test]
    fn shared_type_across_modules_emits_data_flow() {
        // Positive control for F1: a single `shared::Widget` type flows from
        // `a::Producer` (returns Widget) to `b::Consume` (takes Widget). The
        // bare name resolves uniquely in both, so a real DataFlow edge a->b is
        // emitted — the fix must not suppress genuine cross-module flow.
        let result = ParseResult {
            items: vec![
                make_type_item("shared::Widget", "go"),
                make_go_function_item("a::Producer", &[], Some("Widget")),
                make_go_function_item("b::Consume", &[("w", "Widget")], None),
            ],
            relations: vec![make_calls_relation("a::Producer", "b::Consume")],
            warnings: vec![],
            ..Default::default()
        };

        let relations = TypeFlowAnalysis::from_parse_results(&[result]).analyze();
        let data_flow: Vec<_> = relations
            .iter()
            .filter(|r| r.kind == EdgeKind::DataFlow)
            .collect();
        assert_eq!(data_flow.len(), 1, "expected one DataFlow edge");
        assert_eq!(data_flow[0].source_qualified_name, "a");
        assert_eq!(data_flow[0].target_qualified_name, "b");
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
    fn is_project_local_rust_requires_double_colon() {
        assert!(is_project_local("my_crate::MyType", "rust"));
        assert!(is_project_local("a::b::c", "rust"));
        assert!(!is_project_local("String", "rust"));
        assert!(!is_project_local("u32", "rust"));
    }

    #[test]
    fn is_project_local_non_rust_accepts_bare_names() {
        // Non-Rust parsers emit bare identifiers and pre-filter primitives, so a
        // non-empty bare name is treated as project-local.
        assert!(is_project_local("Proposal", "go"));
        assert!(is_project_local("UserService", "typescript"));
        assert!(is_project_local("Order", "java"));
        assert!(is_project_local("DataFrame", "python"));
        assert!(!is_project_local("", "go"));
        assert!(!is_project_local("   ", "python"));
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
    fn module_of_returns_prefix_or_empty_for_top_level() {
        // Nested and single-level names strip their last `::` segment.
        assert_eq!(module_of("my_crate::module::function"), "my_crate::module");
        assert_eq!(module_of("pkg::Convert"), "pkg");
        // Edge case: a top-level / module-less item has no separator, so its
        // module is the empty string — which lets a module-less function and a
        // module-less top-level type match each other (both `""`) without ever
        // matching a namespaced declaration.
        assert_eq!(module_of("Convert"), "");
        assert_eq!(module_of(""), "");
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
