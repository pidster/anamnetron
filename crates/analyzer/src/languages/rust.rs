//! Rust language analyzer using tree-sitter-rust.
//!
//! Extracts structural elements (modules, structs, enums, traits, functions)
//! and relationships (use dependencies, trait implementations, function calls)
//! from Rust source files using tree-sitter parsing.

use std::collections::HashMap;
use std::path::Path;

use svt_core::analysis::LanguageParser as CoreLanguageParser;
use svt_core::model::{EdgeKind, NodeKind};

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// Well-known Rust container/wrapper types.
///
/// When the analyzer encounters `impl WellKnown<InnerType>`, it resolves
/// methods to `InnerType` rather than creating a phantom node for the
/// well-known type.
const RUST_WELL_KNOWN_CONTAINERS: &[&str] = &[
    // Smart pointers / wrappers
    "Arc",
    "Rc",
    "Box",
    "Cow",
    "Cell",
    "RefCell",
    "Mutex",
    "RwLock",
    "MutexGuard",
    "RwLockReadGuard",
    "RwLockWriteGuard",
    "Pin",
    // Optional/Result
    "Option",
    "Result",
    // Collections
    "Vec",
    "VecDeque",
    "LinkedList",
    "BTreeSet",
    "HashSet",
    // Iterators
    "Iter",
    "IntoIter",
];

/// Accumulated mutable state for parsing a single file.
///
/// Bundles all the per-file output collections and counters that were
/// previously threaded through every `visit_*` function as separate
/// mutable references.
struct FileParseState<'a> {
    source: &'a [u8],
    file_path: &'a Path,
    items: Vec<AnalysisItem>,
    relations: Vec<AnalysisRelation>,
    warnings: Vec<AnalysisWarning>,
    unresolved_method_calls: usize,
    resolved_method_calls: usize,
    use_aliases: HashMap<String, String>,
    well_known_containers: &'a [&'a str],
}

/// Rust source code analyzer using tree-sitter-rust.
///
/// Extracts structural elements (modules, structs, enums, traits, functions)
/// and relationships (use dependencies, trait implementations, function calls)
/// from Rust source files.
#[derive(Debug)]
pub struct RustAnalyzer {
    _private: (),
}

impl RustAnalyzer {
    /// Create a new `RustAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for RustAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreLanguageParser for RustAnalyzer {
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult {
        self.analyze_crate(unit_name, files)
    }

    fn well_known_container_types(&self) -> &[&str] {
        RUST_WELL_KNOWN_CONTAINERS
    }
}

impl LanguageAnalyzer for RustAnalyzer {
    fn language_id(&self) -> &str {
        "rust"
    }

    fn analyze_crate(&self, crate_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-rust grammar".to_string(),
            });
            return result;
        }

        for file in files {
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_file(
                        &mut parser,
                        crate_name,
                        file,
                        &source,
                        &mut result.items,
                        &mut result.relations,
                        &mut result.warnings,
                    );
                }
                Err(err) => {
                    result.warnings.push(AnalysisWarning {
                        source_ref: file.display().to_string(),
                        message: format!("failed to read file: {err}"),
                    });
                }
            }
        }

        result
    }
}

/// Parse a single Rust source file and extract structural items and relationships.
fn parse_file(
    parser: &mut tree_sitter::Parser,
    crate_name: &str,
    file_path: &Path,
    source: &str,
    items: &mut Vec<AnalysisItem>,
    relations: &mut Vec<AnalysisRelation>,
    warnings: &mut Vec<AnalysisWarning>,
) {
    let Some(tree) = parser.parse(source, None) else {
        warnings.push(AnalysisWarning {
            source_ref: file_path.display().to_string(),
            message: "tree-sitter failed to parse file".to_string(),
        });
        return;
    };

    let root = tree.root_node();
    let module_context = vec![crate_name.to_string()];

    let mut state = FileParseState {
        source: source.as_bytes(),
        file_path,
        items: Vec::new(),
        relations: Vec::new(),
        warnings: Vec::new(),
        unresolved_method_calls: 0,
        resolved_method_calls: 0,
        use_aliases: HashMap::new(),
        well_known_containers: RUST_WELL_KNOWN_CONTAINERS,
    };

    visit_children(root, &mut state, &module_context, None, false);

    let total = state.resolved_method_calls + state.unresolved_method_calls;
    if total > 0 {
        state.warnings.push(AnalysisWarning {
            source_ref: file_path.display().to_string(),
            message: format!(
                "{total} method call(s): {} resolved, \
                 {} could not be resolved without type information",
                state.resolved_method_calls, state.unresolved_method_calls
            ),
        });
    }

    items.extend(state.items);
    relations.extend(state.relations);
    warnings.extend(state.warnings);
}

/// Visit all named children of a node, extracting structural items and relationships.
fn visit_children(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    impl_type: Option<&str>,
    is_test_context: bool,
) {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            visit_node(child, state, module_context, impl_type, is_test_context);
        }
    }
}

/// Visit a single tree-sitter node and extract structural items and relationships.
fn visit_node(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    impl_type: Option<&str>,
    is_test_context: bool,
) {
    match node.kind() {
        "mod_item" => {
            visit_mod_item(node, state, module_context, is_test_context);
        }
        "struct_item" => {
            let tags = if is_test_context {
                vec!["test".to_string()]
            } else {
                vec![]
            };
            extract_named_item(node, state, module_context, NodeKind::Unit, "struct", tags);
        }
        "enum_item" => {
            let tags = if is_test_context {
                vec!["test".to_string()]
            } else {
                vec![]
            };
            extract_named_item(
                node,
                state,
                module_context,
                NodeKind::Unit,
                "enum",
                tags.clone(),
            );
            visit_enum_variants(node, state, module_context, is_test_context);
        }
        "trait_item" => {
            let tags = if is_test_context {
                vec!["test".to_string()]
            } else {
                vec![]
            };
            extract_named_item(node, state, module_context, NodeKind::Unit, "trait", tags);
        }
        "function_item" => {
            let is_test_fn = is_test_context || has_attribute(node, state.source, "test");
            let tags = if is_test_fn {
                vec!["test".to_string()]
            } else {
                vec![]
            };
            if let Some(type_qn) = impl_type {
                // Method inside an impl block — parent under the type.
                if let Some(name) = item_name(node, state.source) {
                    let qualified_name = format!("{type_qn}::{name}");
                    let line = node.start_position().row + 1;
                    let source_ref = format!("{}:{line}", state.file_path.display());
                    let loc = node.end_position().row - node.start_position().row + 1;
                    state.items.push(AnalysisItem {
                        qualified_name,
                        kind: NodeKind::Unit,
                        sub_kind: "function".to_string(),
                        parent_qualified_name: Some(type_qn.to_string()),
                        source_ref,
                        language: "rust".to_string(),
                        metadata: Some(serde_json::json!({"loc": loc})),
                        tags,
                    });
                }
            } else {
                extract_named_item(
                    node,
                    state,
                    module_context,
                    NodeKind::Unit,
                    "function",
                    tags,
                );
            }
            // Build local type map from function parameters and body.
            let mut local_type_map = node
                .child_by_field_name("parameters")
                .map(|params| {
                    extract_param_types(params, state.source, module_context, &state.use_aliases)
                })
                .unwrap_or_default();
            if let Some(body) = node.child_by_field_name("body") {
                local_type_map.extend(build_local_type_map(
                    body,
                    state.source,
                    module_context,
                    &state.use_aliases,
                ));
            }

            // Descend into the function body to find call expressions.
            if let Some(body) = node.child_by_field_name("body") {
                visit_call_expressions(body, state, module_context, impl_type, &local_type_map);
            }
        }
        "impl_item" => {
            visit_impl_item(node, state, module_context, is_test_context);
        }
        "use_declaration" => {
            visit_use_declaration(node, state, module_context);
        }
        _ => {
            // Recurse into children in case they contain items
            // (e.g., items inside cfg-gated blocks).
            visit_children(node, state, module_context, impl_type, is_test_context);
        }
    }
}

/// Extract a named structural item (struct, enum, trait, function).
fn extract_named_item(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    kind: NodeKind,
    sub_kind: &str,
    tags: Vec<String>,
) {
    let Some(name) = item_name(node, state.source) else {
        return;
    };

    let parent_qualified_name = build_qualified_name(module_context);
    let qualified_name = format!("{parent_qualified_name}::{name}");
    let line = node.start_position().row + 1;
    let source_ref = format!("{}:{line}", state.file_path.display());
    let loc = node.end_position().row - node.start_position().row + 1;

    state.items.push(AnalysisItem {
        qualified_name,
        kind,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: Some(parent_qualified_name),
        source_ref,
        language: "rust".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });
}

/// Extract variants from an `enum_item` node.
///
/// Each variant is emitted as a `Unit` node with sub_kind `"variant"`,
/// parented under the enum's qualified name. This enables `must_contain`
/// constraints to detect specific enum variants (e.g., CLI subcommands).
fn visit_enum_variants(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    is_test_context: bool,
) {
    let Some(enum_name) = item_name(node, state.source) else {
        return;
    };

    let enum_qn = format!("{}::{}", build_qualified_name(module_context), enum_name);

    let Some(body) = node.child_by_field_name("body") else {
        return;
    };

    for i in 0..body.named_child_count() {
        let Some(child) = body.named_child(i) else {
            continue;
        };
        if child.kind() == "enum_variant" {
            if let Some(variant_name) = item_name(child, state.source) {
                let line = child.start_position().row + 1;
                let loc = child.end_position().row - child.start_position().row + 1;
                let tags = if is_test_context {
                    vec!["test".to_string()]
                } else {
                    vec![]
                };
                state.items.push(AnalysisItem {
                    qualified_name: format!("{enum_qn}::{variant_name}"),
                    kind: NodeKind::Unit,
                    sub_kind: "variant".to_string(),
                    parent_qualified_name: Some(enum_qn.clone()),
                    source_ref: format!("{}:{line}", state.file_path.display()),
                    language: "rust".to_string(),
                    metadata: Some(serde_json::json!({"loc": loc})),
                    tags,
                });
            }
        }
    }
}

/// Handle a `mod_item` node. If it has a body (inline module), descend into it.
/// Otherwise, it's a declaration-only module (`mod foo;`).
fn visit_mod_item(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    is_test_context: bool,
) {
    let Some(name) = item_name(node, state.source) else {
        return;
    };

    // Check if this module has #[cfg(test)] — children inherit test context.
    let mod_is_test = is_test_context || has_attribute(node, state.source, "cfg(test)");

    let tags = if mod_is_test {
        vec!["test".to_string()]
    } else {
        vec![]
    };

    let parent_qualified_name = build_qualified_name(module_context);
    let qualified_name = format!("{parent_qualified_name}::{name}");
    let line = node.start_position().row + 1;
    let source_ref = format!("{}:{line}", state.file_path.display());
    let loc = node.end_position().row - node.start_position().row + 1;

    state.items.push(AnalysisItem {
        qualified_name: qualified_name.clone(),
        kind: NodeKind::Component,
        sub_kind: "module".to_string(),
        parent_qualified_name: Some(parent_qualified_name),
        source_ref,
        language: "rust".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // If the module has a body (inline module), descend into its declarations.
    if let Some(body) = node.child_by_field_name("body") {
        let mut child_context = module_context.to_vec();
        child_context.push(name);

        // Scope use_aliases: inner module gets a fresh alias map; outer
        // aliases are restored when we return from the module body.
        let parent_aliases = std::mem::take(&mut state.use_aliases);
        visit_children(body, state, &child_context, None, mod_is_test);
        state.use_aliases = parent_aliases;
    }
}

/// Handle an `impl_item` node. Extract methods as functions scoped under the
/// type being implemented, and emit `Implements` relations for trait impls.
///
/// When the impl target is a well-known container type (e.g., `Arc<Foo>`),
/// the methods are parented under the inner type (`Foo`) instead.
fn visit_impl_item(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    is_test_context: bool,
) {
    // Check for `impl Trait for Type` — emit an Implements relation.
    let trait_node = node.child_by_field_name("trait");
    let type_node = node.child_by_field_name("type");

    // Extract type parameter names (e.g., T, E from `impl<T, E>`) so we can
    // detect blanket impls where the type IS a parameter.
    let type_params = impl_type_param_names(node, state.source);

    // Resolve the impl target type, potentially unwrapping well-known containers.
    let (impl_type_qn, wrapper_type) = resolve_impl_target(
        type_node,
        state.source,
        &type_params,
        module_context,
        &state.use_aliases,
        state.well_known_containers,
    );

    // Emit an Implements relation for trait impls (only when the source type
    // resolves to a real node).
    if let (Some(trait_n), Some(source_qn)) = (trait_node, impl_type_qn.as_ref()) {
        if let Ok(trait_name) = trait_n.utf8_text(state.source) {
            state.relations.push(AnalysisRelation {
                source_qualified_name: source_qn.clone(),
                target_qualified_name: trait_name.to_string(),
                kind: EdgeKind::Implements,
            });
        }
    }

    // Find the body of the impl block and extract function items from it.
    // If this was a well-known container unwrap, annotate methods with wrapper metadata.
    if let Some(body) = node.child_by_field_name("body") {
        if let Some(ref wrapper) = wrapper_type {
            visit_impl_body_with_wrapper(
                body,
                state,
                module_context,
                impl_type_qn.as_deref(),
                is_test_context,
                wrapper,
            );
        } else {
            visit_children(
                body,
                state,
                module_context,
                impl_type_qn.as_deref(),
                is_test_context,
            );
        }
    }
}

/// Visit the body of an impl block that was unwrapped from a well-known container,
/// annotating emitted methods with `wrapped_by` metadata.
fn visit_impl_body_with_wrapper(
    body: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    impl_type: Option<&str>,
    is_test_context: bool,
    wrapper_type: &str,
) {
    let items_before = state.items.len();
    visit_children(body, state, module_context, impl_type, is_test_context);
    // Annotate any newly emitted function items with the wrapper metadata.
    for item in &mut state.items[items_before..] {
        if item.sub_kind == "function" {
            let meta = item.metadata.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = meta.as_object_mut() {
                obj.insert(
                    "wrapped_by".to_string(),
                    serde_json::Value::String(wrapper_type.to_string()),
                );
            }
        }
    }
}

/// Resolve the impl target type, unwrapping well-known containers if applicable.
///
/// Returns `(resolved_qn, wrapper_type_name)`. When the outer type is a
/// well-known container (e.g., `Arc<Foo>`), `resolved_qn` points to the inner
/// type (`Foo`) and `wrapper_type_name` is `Some("Arc")`.
fn resolve_impl_target(
    type_node: Option<tree_sitter::Node<'_>>,
    source: &[u8],
    type_params: &[String],
    module_context: &[String],
    use_aliases: &HashMap<String, String>,
    well_known_containers: &[&str],
) -> (Option<String>, Option<String>) {
    let Some(tn) = type_node else {
        return (None, None);
    };

    let base_name = match type_name_without_generics(tn, source) {
        Some(n) => n,
        None => return (None, None),
    };

    // Check if the base type is a well-known container.
    if well_known_containers.contains(&base_name.as_str()) {
        // Extract the first concrete type argument.
        let type_args = extract_type_arguments(tn, source);
        if let Some(inner) = type_args
            .iter()
            .find(|arg| !type_params.contains(arg) && !is_rust_primitive(arg))
        {
            let inner_qn = resolve_impl_type_qn(inner, type_params, module_context, use_aliases);
            return (inner_qn, Some(base_name));
        }
        // All type args are params/primitives — skip entirely (no useful parent).
        return (None, Some(base_name));
    }

    let qn = resolve_impl_type_qn(&base_name, type_params, module_context, use_aliases);
    (qn, None)
}

/// Handle a `use_declaration` node using tree-sitter traversal.
///
/// Extracts individual use paths, populates `use_aliases`, emits `Depends`
/// relations for each imported item, and emits `Exports` for `pub use`.
fn visit_use_declaration(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
) {
    let source_qn = build_qualified_name(module_context);
    let is_pub = has_visibility_modifier(node);

    if let Some(argument) = node.child_by_field_name("argument") {
        parse_use_tree(argument, state.source, "", &source_qn, is_pub, state);
    }
}

/// Recursively parse a use tree node, emitting `Depends`/`Exports` relations
/// and populating `use_aliases`.
fn parse_use_tree(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    prefix: &str,
    source_qn: &str,
    is_pub: bool,
    state: &mut FileParseState<'_>,
) {
    match node.kind() {
        "identifier" => {
            let name = node.utf8_text(source).unwrap_or_default().trim();
            if name.is_empty() {
                return;
            }
            let full_path = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}::{name}")
            };
            state
                .use_aliases
                .insert(name.to_string(), full_path.clone());
            emit_use_relation(source_qn, &full_path, is_pub, state);
        }
        "scoped_identifier" => {
            let text = node.utf8_text(source).unwrap_or_default().replace(' ', "");
            if text.is_empty() {
                return;
            }
            let full_path = if prefix.is_empty() {
                text.clone()
            } else {
                format!("{prefix}::{text}")
            };
            // The alias is the last segment of the path.
            if let Some(last) = text.rsplit("::").next() {
                state
                    .use_aliases
                    .insert(last.to_string(), full_path.clone());
            }
            emit_use_relation(source_qn, &full_path, is_pub, state);
        }
        "use_as_clause" => {
            // `use foo::bar as baz` — alias field is the local name, value field is the path.
            let path_node = node.child_by_field_name("path");
            let alias_node = node.child_by_field_name("alias");
            if let (Some(pn), Some(an)) = (path_node, alias_node) {
                let path_text = pn.utf8_text(source).unwrap_or_default().replace(' ', "");
                let alias_text = an.utf8_text(source).unwrap_or_default().trim().to_string();
                if !path_text.is_empty() && !alias_text.is_empty() {
                    let full_path = if prefix.is_empty() {
                        path_text.clone()
                    } else {
                        format!("{prefix}::{path_text}")
                    };
                    state.use_aliases.insert(alias_text, full_path.clone());
                    emit_use_relation(source_qn, &full_path, is_pub, state);
                }
            }
        }
        "scoped_use_list" => {
            // `foo::{bar, baz}` — extract the path prefix and recurse into the list.
            let path_node = node.child_by_field_name("path");
            let list_node = node.child_by_field_name("list");

            let new_prefix = if let Some(pn) = path_node {
                let p = pn.utf8_text(source).unwrap_or_default().replace(' ', "");
                if prefix.is_empty() {
                    p
                } else {
                    format!("{prefix}::{p}")
                }
            } else {
                prefix.to_string()
            };

            if let Some(ln) = list_node {
                parse_use_tree(ln, source, &new_prefix, source_qn, is_pub, state);
            }
        }
        "use_list" => {
            // Iterate named children and recurse.
            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    parse_use_tree(child, source, prefix, source_qn, is_pub, state);
                }
            }
        }
        "use_wildcard" => {
            // `use foo::*` — emit Depends on parent, no alias.
            if !prefix.is_empty() {
                emit_use_relation(source_qn, prefix, is_pub, state);
            }
        }
        _ => {
            // Fallback: try text-based extraction for unknown node kinds.
            let text = node.utf8_text(source).unwrap_or_default().replace(' ', "");
            if !text.is_empty() {
                let full_path = if prefix.is_empty() {
                    text.clone()
                } else {
                    format!("{prefix}::{text}")
                };
                if let Some(last) = text.rsplit("::").next() {
                    if !last.contains('{') && !last.contains('*') {
                        state
                            .use_aliases
                            .insert(last.to_string(), full_path.clone());
                    }
                }
                if !full_path.contains('{') {
                    emit_use_relation(source_qn, &full_path, is_pub, state);
                }
            }
        }
    }
}

/// Emit a `Depends` relation (and `Exports` if the use is public).
fn emit_use_relation(source_qn: &str, target: &str, is_pub: bool, state: &mut FileParseState<'_>) {
    state.relations.push(AnalysisRelation {
        source_qualified_name: source_qn.to_string(),
        target_qualified_name: target.to_string(),
        kind: EdgeKind::Depends,
    });
    if is_pub {
        state.relations.push(AnalysisRelation {
            source_qualified_name: source_qn.to_string(),
            target_qualified_name: target.to_string(),
            kind: EdgeKind::Exports,
        });
    }
}

/// Check whether a `use_declaration` node has a `visibility_modifier` child
/// (i.e., `pub use`).
fn has_visibility_modifier(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return true;
        }
    }
    false
}

/// Build a map of local variable names to inferred type qualified names.
///
/// Walks `let_declaration` nodes in a function body and extracts type
/// information from three sources:
/// - **Explicit type annotation**: `let x: Foo = ...`
/// - **Constructor call**: `let x = Foo::new()`
/// - **Struct expression**: `let x = Foo { ... }`
fn build_local_type_map(
    body: tree_sitter::Node<'_>,
    source: &[u8],
    module_context: &[String],
    use_aliases: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut type_map = HashMap::new();
    collect_let_declarations(body, source, module_context, use_aliases, &mut type_map);
    type_map
}

/// Recursively collect `let_declaration` nodes and extract variable → type mappings.
fn collect_let_declarations(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    module_context: &[String],
    use_aliases: &HashMap<String, String>,
    type_map: &mut HashMap<String, String>,
) {
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        if child.kind() == "let_declaration" {
            if let Some(pattern) = child.child_by_field_name("pattern") {
                if let Ok(var_name) = pattern.utf8_text(source) {
                    let var_name = var_name.trim().to_string();
                    if var_name.is_empty() || var_name.contains(' ') {
                        // Skip destructuring patterns
                        continue;
                    }

                    // Try explicit type annotation first: `let x: Foo = ...`
                    if let Some(type_node) = child.child_by_field_name("type") {
                        if let Some(type_qn) = extract_type_from_annotation(
                            type_node,
                            source,
                            module_context,
                            use_aliases,
                        ) {
                            type_map.insert(var_name, type_qn);
                            continue;
                        }
                    }

                    // Try inferring from the value expression
                    if let Some(value) = child.child_by_field_name("value") {
                        if let Some(type_qn) =
                            infer_type_from_value(value, source, module_context, use_aliases)
                        {
                            type_map.insert(var_name, type_qn);
                        }
                    }
                }
            }
        } else {
            // Recurse into child nodes (e.g., blocks within the function body)
            collect_let_declarations(child, source, module_context, use_aliases, type_map);
        }
    }
}

/// Extract the base type name from a type node, stripping generic parameters.
///
/// For `Result<T, E>` returns `"Result"`. For `Foo` returns `"Foo"`.
/// For `std::io::Result<T>` returns `"std::io::Result"`.
fn type_name_without_generics(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" => node.utf8_text(source).ok().map(String::from),
        "generic_type" => {
            let base = node.child_by_field_name("type")?;
            type_name_without_generics(base, source)
        }
        "scoped_type_identifier" => {
            // e.g., path::to::Type — get full text, already no generics at this level
            node.utf8_text(source).ok().map(|s| s.replace(' ', ""))
        }
        _ => {
            // Fallback: strip anything from first '<' onward
            let text = node.utf8_text(source).ok()?;
            Some(text.split('<').next().unwrap_or(text).trim().to_string())
        }
    }
}

/// Extract concrete type argument names from a `generic_type` node.
///
/// For `Arc<RaftEngine>` returns `["RaftEngine"]`.
/// For `Result<Foo, Bar>` returns `["Foo", "Bar"]`.
/// Filters out type parameters and primitives.
fn extract_type_arguments(node: tree_sitter::Node<'_>, source: &[u8]) -> Vec<String> {
    let mut args = Vec::new();

    // The node might be a `generic_type` with a `type_arguments` field.
    let type_args_node = if node.kind() == "generic_type" {
        node.child_by_field_name("type_arguments")
    } else {
        None
    };

    let Some(ta) = type_args_node else {
        return args;
    };

    for i in 0..ta.named_child_count() {
        let Some(child) = ta.named_child(i) else {
            continue;
        };
        if let Some(name) = type_name_without_generics(child, source) {
            args.push(name);
        }
    }

    args
}

/// Extract type parameter names declared on an `impl` block.
///
/// For `impl<T, E>`, returns `["T", "E"]`. For `impl<'a, T: Display>`,
/// returns `["T"]` (lifetimes are excluded).
fn impl_type_param_names(impl_node: tree_sitter::Node<'_>, source: &[u8]) -> Vec<String> {
    let Some(type_params) = impl_node.child_by_field_name("type_parameters") else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut cursor = type_params.walk();
    for child in type_params.children(&mut cursor) {
        // tree-sitter-rust wraps each type param in a `type_parameter` node
        // with a `name` field (e.g., `T` or `T: Display`).
        if child.kind() == "type_parameter" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    names.push(name.to_string());
                }
            }
        }
        // `lifetime_parameter` nodes are skipped (not type params).
    }
    names
}

/// Check whether a type name is a Rust primitive (not a user-defined node).
fn is_rust_primitive(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "char"
            | "str"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
    )
}

/// Build the qualified name for an impl block's target type, or `None` if the
/// type is a type parameter or primitive (which would produce a phantom node).
///
/// Resolution order:
/// 1. Type parameter or primitive → `None`
/// 2. Contains `::` → keep as-is (already qualified)
/// 3. Found in `use_aliases` → return the resolved path
/// 4. Default → prepend module context
fn resolve_impl_type_qn(
    type_name: &str,
    type_params: &[String],
    module_context: &[String],
    use_aliases: &HashMap<String, String>,
) -> Option<String> {
    if type_params.contains(&type_name.to_string()) || is_rust_primitive(type_name) {
        return None;
    }
    if type_name.contains("::") {
        // Already scoped (e.g., `other_crate::Type`) — keep as-is.
        Some(type_name.to_string())
    } else if let Some(resolved) = use_aliases.get(type_name) {
        Some(resolved.clone())
    } else {
        let module_qn = build_qualified_name(module_context);
        Some(format!("{module_qn}::{type_name}"))
    }
}

/// Extract a type qualified name from a type annotation node.
///
/// Handles `type_identifier` (e.g., `Foo`), `generic_type` (e.g., `Vec<Foo>`
/// → extracts `Vec`), and `scoped_type_identifier` (e.g., `std::io::Stdout`).
fn extract_type_from_annotation(
    type_node: tree_sitter::Node<'_>,
    source: &[u8],
    module_context: &[String],
    use_aliases: &HashMap<String, String>,
) -> Option<String> {
    match type_node.kind() {
        "type_identifier" => {
            let name = type_node.utf8_text(source).ok()?;
            if name.starts_with(|c: char| c.is_uppercase()) {
                if let Some(resolved) = use_aliases.get(name) {
                    Some(resolved.clone())
                } else {
                    let module_qn = build_qualified_name(module_context);
                    Some(format!("{module_qn}::{name}"))
                }
            } else {
                None
            }
        }
        "generic_type" => {
            // Extract the base type from generic (e.g., `Vec<T>` → `Vec`)
            let base = type_node.child_by_field_name("type")?;
            extract_type_from_annotation(base, source, module_context, use_aliases)
        }
        "scoped_type_identifier" => {
            // Fully qualified type path — keep as-is
            let text = type_node.utf8_text(source).ok()?;
            Some(text.replace(' ', ""))
        }
        "reference_type" => {
            // &Type or &mut Type — extract the inner type
            let inner = type_node.child_by_field_name("type")?;
            extract_type_from_annotation(inner, source, module_context, use_aliases)
        }
        _ => None,
    }
}

/// Infer a type from a value expression.
///
/// Handles constructor calls (`Foo::new()`) and struct expressions (`Foo { ... }`).
fn infer_type_from_value(
    value: tree_sitter::Node<'_>,
    source: &[u8],
    module_context: &[String],
    use_aliases: &HashMap<String, String>,
) -> Option<String> {
    match value.kind() {
        "call_expression" => {
            // Check for constructor pattern: `Foo::new()` or `Foo::default()`
            let function = value.child_by_field_name("function")?;
            if function.kind() == "scoped_identifier" {
                let text = function.utf8_text(source).ok()?.replace(' ', "");
                let segments: Vec<&str> = text.split("::").collect();
                if segments.len() == 2 {
                    let type_seg = segments[0];
                    if type_seg.starts_with(|c: char| c.is_uppercase()) {
                        if let Some(resolved) = use_aliases.get(type_seg) {
                            return Some(resolved.clone());
                        }
                        let module_qn = build_qualified_name(module_context);
                        return Some(format!("{module_qn}::{type_seg}"));
                    }
                }
            }
            None
        }
        "struct_expression" => {
            // `Foo { field: value }` — extract type from name
            let name_node = value.child_by_field_name("name")?;
            let name = name_node.utf8_text(source).ok()?;
            if name.starts_with(|c: char| c.is_uppercase()) {
                if let Some(resolved) = use_aliases.get(name) {
                    return Some(resolved.clone());
                }
                let module_qn = build_qualified_name(module_context);
                Some(format!("{module_qn}::{name}"))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract parameter name → type mappings from a function's parameter list.
///
/// Skips `self` parameters. Handles `&Type` and `&mut Type` by stripping
/// reference wrappers to extract the underlying type.
fn extract_param_types(
    params_node: tree_sitter::Node<'_>,
    source: &[u8],
    module_context: &[String],
    use_aliases: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut type_map = HashMap::new();

    for i in 0..params_node.named_child_count() {
        let Some(child) = params_node.named_child(i) else {
            continue;
        };

        if child.kind() == "parameter" {
            let pattern = match child.child_by_field_name("pattern") {
                Some(p) => p,
                None => continue,
            };
            let type_node = match child.child_by_field_name("type") {
                Some(t) => t,
                None => continue,
            };

            if let Ok(param_name) = pattern.utf8_text(source) {
                let param_name = param_name.trim();
                if param_name == "self" || param_name.is_empty() {
                    continue;
                }
                if let Some(type_qn) =
                    extract_type_from_annotation(type_node, source, module_context, use_aliases)
                {
                    type_map.insert(param_name.to_string(), type_qn);
                }
            }
        }
        // Skip self_parameter nodes
    }

    type_map
}

/// Resolve a scoped call path to a fully qualified name.
///
/// Applies heuristic resolution for common call patterns:
/// - `Self::method` → replace `Self` with the `impl_type` QN (when available)
/// - `Type::method` (single-segment Type starting with uppercase) → check
///   `use_aliases` first, then prepend `module_context`
/// - Multi-segment paths (e.g., `std::io::stdout`) → keep as-is
fn resolve_scoped_call(
    callee: &str,
    module_context: &[String],
    impl_type: Option<&str>,
    use_aliases: &HashMap<String, String>,
) -> String {
    // Self::method → impl_type::method
    if let Some(rest) = callee.strip_prefix("Self::") {
        if let Some(type_qn) = impl_type {
            return format!("{type_qn}::{rest}");
        }
    }

    // Check if it's a two-segment path with uppercase first segment (local type).
    // Skip "Self::" without impl_type — it can't be resolved.
    let segments: Vec<&str> = callee.split("::").collect();
    if segments.len() == 2 {
        let first = segments[0];
        if first != "Self" && first.starts_with(|c: char| c.is_uppercase()) {
            if let Some(resolved) = use_aliases.get(first) {
                return format!("{resolved}::{}", segments[1]);
            }
            let module_qn = build_qualified_name(module_context);
            return format!("{module_qn}::{callee}");
        }
    }

    // Multi-segment or lowercase path — keep as-is.
    callee.to_string()
}

/// Recursively walk a subtree looking for `call_expression` nodes
/// and emit `Calls` relations for syntactically resolvable calls.
///
/// When `impl_type` is `Some`, `self.method()` calls are resolved to
/// `ImplType::method`. When `local_type_map` contains a mapping for
/// a receiver variable, `x.method()` calls are resolved to
/// `Type::method`. Other method calls remain unresolved.
fn visit_call_expressions(
    node: tree_sitter::Node<'_>,
    state: &mut FileParseState<'_>,
    module_context: &[String],
    impl_type: Option<&str>,
    local_type_map: &HashMap<String, String>,
) {
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        if child.kind() == "call_expression" {
            if let Some(function) = child.child_by_field_name("function") {
                let source_qn = build_qualified_name(module_context);

                match function.kind() {
                    "identifier" | "scoped_identifier" => {
                        // Simple function call or path-qualified call (e.g., `foo()` or `mod::foo()`).
                        if let Ok(callee) = function.utf8_text(state.source) {
                            let callee = callee.replace(' ', "");
                            if !callee.is_empty() {
                                let resolved = resolve_scoped_call(
                                    &callee,
                                    module_context,
                                    impl_type,
                                    &state.use_aliases,
                                );
                                state.relations.push(AnalysisRelation {
                                    source_qualified_name: source_qn,
                                    target_qualified_name: resolved,
                                    kind: EdgeKind::Calls,
                                });
                            }
                        }
                    }
                    "field_expression" => {
                        // Method call (e.g., `self.foo()` or `x.foo()`)
                        let mut resolved = false;

                        if let (Some(receiver), Some(method)) = (
                            function.child_by_field_name("value"),
                            function.child_by_field_name("field"),
                        ) {
                            if let (Ok(receiver_text), Ok(method_name)) = (
                                receiver.utf8_text(state.source),
                                method.utf8_text(state.source),
                            ) {
                                if receiver_text == "self" {
                                    // self.method() → resolve via impl_type
                                    if let Some(type_qn) = impl_type {
                                        state.relations.push(AnalysisRelation {
                                            source_qualified_name: build_qualified_name(
                                                module_context,
                                            ),
                                            target_qualified_name: format!(
                                                "{type_qn}::{method_name}"
                                            ),
                                            kind: EdgeKind::Calls,
                                        });
                                        resolved = true;
                                    }
                                } else if receiver.kind() == "identifier" {
                                    // x.method() → look up x in local_type_map
                                    if let Some(type_qn) = local_type_map.get(receiver_text) {
                                        state.relations.push(AnalysisRelation {
                                            source_qualified_name: build_qualified_name(
                                                module_context,
                                            ),
                                            target_qualified_name: format!(
                                                "{type_qn}::{method_name}"
                                            ),
                                            kind: EdgeKind::Calls,
                                        });
                                        state.resolved_method_calls += 1;
                                        resolved = true;
                                    }
                                }
                                // Chained calls (receiver is call_expression) and field
                                // access (self.field.method()) are not resolved.
                            }
                        }

                        if !resolved {
                            state.unresolved_method_calls += 1;
                        }
                    }
                    _ => {
                        // Other call forms (closures, etc.) — skip silently.
                    }
                }
            }
        }

        // Recurse into children to find nested call expressions.
        visit_call_expressions(child, state, module_context, impl_type, local_type_map);
    }
}

/// Check whether a tree-sitter node has a preceding `attribute_item` sibling
/// containing the given attribute text (e.g., "test" or "cfg(test)").
///
/// Walks backward through siblings looking for `attribute_item` nodes whose
/// text (between `#[` and `]`) matches the given attribute string.
fn has_attribute(node: tree_sitter::Node<'_>, source: &[u8], attr_text: &str) -> bool {
    let mut sibling = node.prev_named_sibling();
    while let Some(sib) = sibling {
        if sib.kind() == "attribute_item" {
            if let Ok(text) = sib.utf8_text(source) {
                // Attribute text looks like `#[test]` or `#[cfg(test)]`.
                // Strip `#[` prefix and `]` suffix, then compare.
                let inner = text
                    .trim()
                    .strip_prefix("#[")
                    .and_then(|s| s.strip_suffix(']'));
                if let Some(inner) = inner {
                    if inner.trim() == attr_text {
                        return true;
                    }
                }
            }
            // Keep looking — there can be multiple attributes.
            sibling = sib.prev_named_sibling();
        } else {
            // Non-attribute node — stop looking.
            break;
        }
    }
    false
}

/// Extract the name of a tree-sitter item node.
///
/// Looks for the "name" field first, which covers most item types.
fn item_name<'a>(node: tree_sitter::Node<'a>, source: &'a [u8]) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(String::from)
}

/// Build a qualified name from the module context stack.
///
/// The context is `["crate_name", "mod1", "mod2", ...]` and the result
/// is `"crate_name::mod1::mod2"`.
fn build_qualified_name(context: &[String]) -> String {
    context.join("::")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_source(crate_name: &str, source: &str) -> ParseResult {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        write!(file, "{}", source).unwrap();
        let analyzer = RustAnalyzer::new();
        analyzer.analyze_crate(crate_name, &[file.path()])
    }

    #[test]
    fn extracts_module_declaration() {
        let result = parse_source("my_crate", "pub mod handlers;");
        let modules: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "module")
            .collect();
        assert!(
            modules
                .iter()
                .any(|m| m.qualified_name == "my_crate::handlers"),
            "should extract module 'my_crate::handlers', got: {:?}",
            modules
        );
    }

    #[test]
    fn extracts_struct() {
        let result = parse_source("my_crate", "pub struct MyStruct { field: u32 }");
        let structs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "struct")
            .collect();
        assert!(
            structs
                .iter()
                .any(|s| s.qualified_name == "my_crate::MyStruct"),
            "should extract struct, got: {:?}",
            structs
        );
    }

    #[test]
    fn extracts_enum() {
        let result = parse_source("my_crate", "pub enum Status { Active, Inactive }");
        let enums: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "enum")
            .collect();
        assert!(enums.iter().any(|e| e.qualified_name == "my_crate::Status"));
    }

    #[test]
    fn extracts_trait() {
        let result = parse_source("my_crate", "pub trait Storage { fn get(&self); }");
        let traits: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "trait")
            .collect();
        assert!(traits
            .iter()
            .any(|t| t.qualified_name == "my_crate::Storage"));
    }

    #[test]
    fn extracts_function() {
        let result = parse_source("my_crate", "pub fn process_data(x: u32) -> u32 { x }");
        let fns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(fns
            .iter()
            .any(|f| f.qualified_name == "my_crate::process_data"));
    }

    #[test]
    fn parent_set_correctly_for_inline_module() {
        let result = parse_source(
            "my_crate",
            r#"
            pub mod inner {
                pub struct Foo;
            }
        "#,
        );
        let foo = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("Foo"));
        assert!(foo.is_some(), "should find Foo");
        assert_eq!(
            foo.unwrap().parent_qualified_name,
            Some("my_crate::inner".to_string())
        );
    }

    #[test]
    fn does_not_emit_crate_item() {
        // Crate items are emitted by the orchestrator, not by tree-sitter analysis
        let result = parse_source("my_crate", "pub fn main() {}");
        let crate_item = result.items.iter().find(|i| i.sub_kind == "crate");
        assert!(
            crate_item.is_none(),
            "tree-sitter analyzer should not emit crate-level items"
        );
    }

    #[test]
    fn module_parent_is_crate() {
        let result = parse_source("my_crate", "pub mod utils;");
        let module = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::utils")
            .expect("should find module");
        assert_eq!(
            module.parent_qualified_name,
            Some("my_crate".to_string()),
            "top-level module parent should be the crate"
        );
        assert_eq!(module.kind, NodeKind::Component);
    }

    #[test]
    fn struct_parent_is_crate() {
        let result = parse_source("my_crate", "pub struct Foo;");
        let item = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::Foo")
            .expect("should find Foo");
        assert_eq!(item.parent_qualified_name, Some("my_crate".to_string()));
        assert_eq!(item.kind, NodeKind::Unit);
    }

    #[test]
    fn extracts_nested_modules() {
        let result = parse_source(
            "my_crate",
            r#"
            pub mod outer {
                pub mod inner {
                    pub fn deep() {}
                }
            }
        "#,
        );

        let outer = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::outer");
        assert!(outer.is_some(), "should find outer module");

        let inner = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::outer::inner");
        assert!(inner.is_some(), "should find inner module");
        assert_eq!(
            inner.unwrap().parent_qualified_name,
            Some("my_crate::outer".to_string())
        );

        let deep = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::outer::inner::deep");
        assert!(deep.is_some(), "should find deep function");
        assert_eq!(
            deep.unwrap().parent_qualified_name,
            Some("my_crate::outer::inner".to_string())
        );
    }

    #[test]
    fn source_ref_contains_line_number() {
        let result = parse_source("my_crate", "pub struct Foo;");
        let item = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::Foo")
            .expect("should find Foo");
        // Line number should be present (format is "path:line")
        assert!(
            item.source_ref.contains(':'),
            "source_ref should contain a colon separator, got: {}",
            item.source_ref
        );
    }

    #[test]
    fn language_is_rust() {
        let result = parse_source("my_crate", "pub struct Foo;");
        let item = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::Foo")
            .expect("should find Foo");
        assert_eq!(item.language, "rust");
    }

    #[test]
    fn extracts_enum_variants() {
        let result = parse_source(
            "my_crate",
            "pub enum Commands { Import, Check, Analyze, Export }",
        );
        let variants: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "variant")
            .collect();
        assert_eq!(
            variants.len(),
            4,
            "should extract 4 enum variants, got: {:?}",
            variants
        );
        assert!(variants
            .iter()
            .any(|v| v.qualified_name == "my_crate::Commands::Check"));
        assert!(variants
            .iter()
            .any(|v| v.qualified_name == "my_crate::Commands::Import"));
    }

    #[test]
    fn enum_variant_parent_is_enum() {
        let result = parse_source("my_crate", "pub enum Status { Active, Inactive }");
        let variant = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::Status::Active")
            .expect("should find Active variant");
        assert_eq!(
            variant.parent_qualified_name,
            Some("my_crate::Status".to_string())
        );
        assert_eq!(variant.kind, NodeKind::Unit);
        assert_eq!(variant.sub_kind, "variant");
    }

    #[test]
    fn extracts_impl_methods() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn bar(&self) {}
            }
        "#,
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            methods
                .iter()
                .any(|m| m.qualified_name == "my_crate::Foo::bar"),
            "should extract impl method scoped under type, got: {:?}",
            methods
        );
    }

    #[test]
    fn impl_method_parented_under_type() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn bar(&self) {}
                pub fn baz(&self) {}
            }
        "#,
        );
        let bar = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function" && i.qualified_name.ends_with("bar"));
        assert!(bar.is_some(), "should find method bar");
        let bar = bar.unwrap();
        assert_eq!(
            bar.qualified_name, "my_crate::Foo::bar",
            "method should be scoped under its type"
        );
        assert_eq!(
            bar.parent_qualified_name,
            Some("my_crate::Foo".to_string()),
            "method parent should be the impl type"
        );
    }

    #[test]
    fn multiple_files_combined() {
        let mut file1 = NamedTempFile::with_suffix(".rs").unwrap();
        write!(file1, "pub struct Alpha;").unwrap();

        let mut file2 = NamedTempFile::with_suffix(".rs").unwrap();
        write!(file2, "pub struct Beta;").unwrap();

        let analyzer = RustAnalyzer::new();
        let result = analyzer.analyze_crate("my_crate", &[file1.path(), file2.path()]);

        assert!(result
            .items
            .iter()
            .any(|i| i.qualified_name == "my_crate::Alpha"));
        assert!(result
            .items
            .iter()
            .any(|i| i.qualified_name == "my_crate::Beta"));
    }

    #[test]
    fn empty_file_produces_no_items() {
        let result = parse_source("my_crate", "");
        assert!(result.items.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn private_items_also_extracted() {
        // The analyzer extracts all items regardless of visibility —
        // visibility filtering is done elsewhere if needed.
        let result = parse_source("my_crate", "struct Private;");
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my_crate::Private"),
            "private items should also be extracted"
        );
    }

    // --- Relationship extraction tests ---

    #[test]
    fn use_statement_generates_depends_relation() {
        let result = parse_source("my_crate", "use other_crate::something;");
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            !depends.is_empty(),
            "use statement should generate Depends relation"
        );
        assert_eq!(
            depends[0].target_qualified_name, "other_crate::something",
            "target should be the use path"
        );
        assert_eq!(
            depends[0].source_qualified_name, "my_crate",
            "source should be the current module"
        );
    }

    #[test]
    fn grouped_use_generates_individual_depends() {
        let result = parse_source("my_crate", "use std::collections::{HashMap, HashSet};");
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            depends.len() >= 2,
            "grouped use should generate individual Depends relations, got: {:?}",
            depends
        );
        let targets: Vec<&str> = depends
            .iter()
            .map(|d| d.target_qualified_name.as_str())
            .collect();
        assert!(
            targets.contains(&"std::collections::HashMap"),
            "should depend on HashMap, got: {:?}",
            targets
        );
        assert!(
            targets.contains(&"std::collections::HashSet"),
            "should depend on HashSet, got: {:?}",
            targets
        );
    }

    #[test]
    fn impl_trait_generates_implements_relation() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait Foo {}
            pub struct Bar;
            impl Foo for Bar {}
        "#,
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(
            !impls.is_empty(),
            "impl Trait for Type should generate Implements relation"
        );
        assert_eq!(
            impls[0].source_qualified_name, "my_crate::Bar",
            "source should be the implementing type"
        );
        assert_eq!(
            impls[0].target_qualified_name, "Foo",
            "target should be the trait"
        );
    }

    #[test]
    fn inherent_impl_does_not_generate_implements_relation() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn bar(&self) {}
            }
        "#,
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(
            impls.is_empty(),
            "inherent impl (no trait) should not generate Implements relation"
        );
    }

    #[test]
    fn function_call_generates_calls_relation() {
        let result = parse_source(
            "my_crate",
            r#"
            fn helper() {}
            fn main() {
                helper();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            !calls.is_empty(),
            "function call should generate Calls relation"
        );
        assert_eq!(
            calls[0].target_qualified_name, "helper",
            "target should be the called function"
        );
    }

    #[test]
    fn scoped_function_call_generates_calls_relation() {
        let result = parse_source(
            "my_crate",
            r#"
            fn main() {
                std::io::stdout();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            !calls.is_empty(),
            "scoped function call should generate Calls relation"
        );
        assert_eq!(
            calls[0].target_qualified_name, "std::io::stdout",
            "target should be the fully qualified call path"
        );
    }

    #[test]
    fn method_call_generates_warning() {
        let result = parse_source(
            "my_crate",
            r#"
            fn get_thing() -> u32 { 0 }
            fn main() {
                let x = get_thing();
                x.do_stuff();
            }
        "#,
        );
        let method_warnings: Vec<_> = result
            .warnings
            .iter()
            .filter(|w| w.message.contains("could not be resolved"))
            .collect();
        assert!(
            !method_warnings.is_empty(),
            "method call should produce an aggregated warning"
        );
        assert_eq!(
            method_warnings.len(),
            1,
            "should produce exactly one aggregated warning per file"
        );
    }

    #[test]
    fn method_call_handled_gracefully() {
        let result = parse_source(
            "my_crate",
            r#"
            fn get_thing() -> u32 { 0 }
            fn main() {
                let x = get_thing();
                x.do_stuff();
            }
        "#,
        );
        // Method calls on opaque types can't be resolved — should not crash.
        let method_calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("do_stuff"))
            .collect();
        assert!(
            method_calls.is_empty(),
            "method call on opaque type should not generate a Calls relation"
        );
    }

    #[test]
    fn constructor_method_call_is_resolved() {
        let result = parse_source(
            "my_crate",
            r#"
            fn main() {
                let x = String::new();
                x.push_str("hello");
            }
        "#,
        );
        // With heuristic type resolution, constructor patterns are resolved.
        let method_calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("push_str"))
            .collect();
        assert!(
            !method_calls.is_empty(),
            "method call on constructor-initialized variable should be resolved"
        );
    }

    #[test]
    fn self_method_call_generates_calls_relation() {
        let result = parse_source(
            "my_crate",
            r#"
        pub struct Foo;
        impl Foo {
            pub fn bar(&self) {
                self.baz();
            }
            pub fn baz(&self) {}
        }
    "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::baz"),
            "self.baz() inside impl Foo should resolve to my_crate::Foo::baz, got: {:?}",
            calls
        );
    }

    #[test]
    fn non_self_method_call_remains_unresolved() {
        let result = parse_source(
            "my_crate",
            r#"
        fn get_thing() -> u32 { 0 }
        pub struct Foo;
        impl Foo {
            pub fn bar(&self) {
                let x = get_thing();
                x.do_stuff();
            }
        }
    "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("do_stuff"))
            .collect();
        assert!(
            calls.is_empty(),
            "opaque method call should not generate a Calls relation, got: {:?}",
            calls
        );
        let method_warnings: Vec<_> = result
            .warnings
            .iter()
            .filter(|w| w.message.contains("could not be resolved"))
            .collect();
        assert!(
            !method_warnings.is_empty(),
            "opaque method call should still produce unresolved warning"
        );
    }

    // --- Scoped call resolution tests (Task 2) ---

    #[test]
    fn self_type_associated_call_resolved() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn create() -> Self {
                    Self::new()
                }
                pub fn new() -> Self { Foo }
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::new"),
            "Self::new() inside impl Foo should resolve to my_crate::Foo::new, got: {:?}",
            calls
        );
    }

    #[test]
    fn local_type_associated_call_resolved() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn new() -> Self { Foo }
            }
            fn main() {
                Foo::new();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("new"))
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::new"),
            "Foo::new() in same module should resolve to my_crate::Foo::new, got: {:?}",
            calls
        );
    }

    #[test]
    fn fully_qualified_call_preserved() {
        let result = parse_source(
            "my_crate",
            r#"
            fn main() {
                std::io::stdout();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "std::io::stdout"),
            "std::io::stdout() should stay as-is, got: {:?}",
            calls
        );
    }

    // --- Local type resolution tests (Task 3) ---

    #[test]
    fn let_with_type_annotation_resolves_method_call() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo { pub fn bar(&self) {} }
            fn main() {
                let x: Foo = Foo;
                x.bar();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("bar"))
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::bar"),
            "x.bar() with type annotation should resolve to my_crate::Foo::bar, got: {:?}",
            calls
        );
    }

    #[test]
    fn let_with_constructor_resolves_method_call() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn new() -> Self { Foo }
                pub fn bar(&self) {}
            }
            fn main() {
                let x = Foo::new();
                x.bar();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("bar"))
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::bar"),
            "x.bar() with constructor should resolve to my_crate::Foo::bar, got: {:?}",
            calls
        );
    }

    #[test]
    fn let_with_struct_expression_resolves_method_call() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo { val: u32 }
            impl Foo { pub fn bar(&self) {} }
            fn main() {
                let x = Foo { val: 42 };
                x.bar();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("bar"))
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::bar"),
            "x.bar() with struct expression should resolve to my_crate::Foo::bar, got: {:?}",
            calls
        );
    }

    #[test]
    fn function_parameter_type_resolves_method_call() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo { pub fn bar(&self) {} }
            fn process(x: Foo) {
                x.bar();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("bar"))
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::bar"),
            "x.bar() with parameter type should resolve to my_crate::Foo::bar, got: {:?}",
            calls
        );
    }

    #[test]
    fn reference_parameter_resolves_method_call() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo { pub fn bar(&self) {} }
            fn process(x: &Foo) {
                x.bar();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("bar"))
            .collect();
        assert!(
            calls
                .iter()
                .any(|c| c.target_qualified_name == "my_crate::Foo::bar"),
            "x.bar() with &Foo parameter should resolve through reference, got: {:?}",
            calls
        );
    }

    #[test]
    fn unknown_variable_still_unresolved() {
        let result = parse_source(
            "my_crate",
            r#"
            fn get_something() -> u32 { 0 }
            fn main() {
                let x = get_something();
                x.bar();
            }
        "#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("bar"))
            .collect();
        assert!(
            calls.is_empty(),
            "opaque let x = something() should remain unresolved, got: {:?}",
            calls
        );
        let method_warnings: Vec<_> = result
            .warnings
            .iter()
            .filter(|w| w.message.contains("could not be resolved"))
            .collect();
        assert!(
            !method_warnings.is_empty(),
            "should still produce unresolved warning"
        );
    }

    #[test]
    fn chained_calls_remain_unresolved() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn new() -> Self { Foo }
                pub fn bar(&self) -> &Self { self }
                pub fn baz(&self) {}
            }
            fn main() {
                Foo::new().bar().baz();
            }
        "#,
        );
        // The chained call .baz() should remain unresolved since
        // the receiver is a call expression, not an identifier.
        let method_warnings: Vec<_> = result
            .warnings
            .iter()
            .filter(|w| w.message.contains("could not be resolved"))
            .collect();
        assert!(
            !method_warnings.is_empty(),
            "chained method calls should produce unresolved warning"
        );
    }

    #[test]
    fn resolve_scoped_call_unit_tests() {
        let ctx = vec!["my_crate".to_string(), "module".to_string()];
        let impl_type = Some("my_crate::Foo");
        let aliases = HashMap::new();

        // Self::method → impl_type::method
        assert_eq!(
            resolve_scoped_call("Self::new", &ctx, impl_type, &aliases),
            "my_crate::Foo::new"
        );

        // Type::method (uppercase, 2 segments) → module_context::Type::method
        assert_eq!(
            resolve_scoped_call("Bar::create", &ctx, impl_type, &aliases),
            "my_crate::module::Bar::create"
        );

        // Multi-segment → kept as-is
        assert_eq!(
            resolve_scoped_call("std::io::stdout", &ctx, impl_type, &aliases),
            "std::io::stdout"
        );

        // Simple function (no ::) → kept as-is
        assert_eq!(
            resolve_scoped_call("helper", &ctx, impl_type, &aliases),
            "helper"
        );

        // Self:: without impl_type → kept as-is
        assert_eq!(
            resolve_scoped_call("Self::new", &ctx, None, &aliases),
            "Self::new"
        );

        // Lowercase 2-segment → kept as-is (not a type)
        assert_eq!(
            resolve_scoped_call("module::func", &ctx, impl_type, &aliases),
            "module::func"
        );
    }

    // --- Generic type parameter stripping tests ---

    #[test]
    fn generic_impl_methods_have_correct_parent() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Wrapper<T> { val: T }
            impl<T> Wrapper<T> {
                pub fn get(&self) -> &T { &self.val }
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("get"))
            .expect("should find method get");
        assert_eq!(
            method.parent_qualified_name,
            Some("my_crate::Wrapper".to_string()),
            "method parent should be Wrapper, not Wrapper<T>"
        );
        assert_eq!(
            method.qualified_name, "my_crate::Wrapper::get",
            "method qualified name should not contain generics"
        );
    }

    #[test]
    fn generic_trait_impl_strips_generics_from_source() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait MyTrait {}
            pub struct MyResult<T, E> { ok: T, err: E }
            impl<T, E> MyTrait for MyResult<T, E> {}
        "#,
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(!impls.is_empty(), "should generate Implements relation");
        assert_eq!(
            impls[0].source_qualified_name, "my_crate::MyResult",
            "Implements source should be MyResult, not MyResult<T, E>"
        );
    }

    #[test]
    fn lifetime_generics_stripped_from_impl() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Guard<'a> { data: &'a str }
            impl<'a> Guard<'a> {
                pub fn new(data: &'a str) -> Self { Guard { data } }
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("new"))
            .expect("should find method new");
        assert_eq!(
            method.parent_qualified_name,
            Some("my_crate::Guard".to_string()),
            "method parent should be Guard, not Guard<'a>"
        );
    }

    // --- Blanket impl and primitive type filtering tests ---

    #[test]
    fn blanket_impl_methods_not_parented() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait MyTrait { fn do_thing(&self); }
            impl<T> MyTrait for T {
                fn do_thing(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("do_thing"));
        // In a blanket impl, methods should not be parented under type parameter T.
        // They become module-scoped instead.
        assert!(
            method.is_none()
                || method.unwrap().parent_qualified_name.as_deref() != Some("my_crate::T"),
            "blanket impl method should not be parented under type parameter T"
        );
    }

    #[test]
    fn blanket_impl_skips_implements_relation() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait MyTrait {}
            impl<T> MyTrait for T {}
        "#,
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(
            impls.is_empty(),
            "blanket impl should not emit Implements relation (source node is phantom), got: {:?}",
            impls
        );
    }

    #[test]
    fn primitive_impl_methods_not_parented() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait MyTrait { fn do_thing(&self); }
            impl MyTrait for u64 {
                fn do_thing(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("do_thing"));
        assert!(
            method.is_none()
                || method.unwrap().parent_qualified_name.as_deref() != Some("my_crate::u64"),
            "primitive impl method should not be parented under u64"
        );
    }

    #[test]
    fn constrained_type_param_detected() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait Display { fn fmt(&self); }
            impl<T: Display> Display for T {
                fn fmt(&self) {}
            }
        "#,
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(
            impls.is_empty(),
            "impl with constrained type param should not emit Implements (phantom source), got: {:?}",
            impls
        );
    }

    #[test]
    fn scoped_impl_type_not_prefixed_with_module() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait MyTrait {}
            impl MyTrait for other_crate::Foo {}
        "#,
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(
            !impls.is_empty(),
            "scoped type impl should still emit Implements relation"
        );
        assert_eq!(
            impls[0].source_qualified_name, "other_crate::Foo",
            "scoped type should not get module prefix prepended"
        );
    }

    #[test]
    fn normal_inherent_impl_still_works() {
        // Regression: ensure normal impls are not broken by the filtering.
        let result = parse_source(
            "my_crate",
            r#"
            pub struct Foo;
            impl Foo {
                pub fn bar(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::Foo::bar")
            .expect("should find method bar");
        assert_eq!(
            method.parent_qualified_name,
            Some("my_crate::Foo".to_string()),
            "inherent impl method should still be parented under its type"
        );
    }

    #[test]
    fn loc_computed_for_struct() {
        let result = parse_source(
            "my_crate",
            r#"pub struct Foo {
    field_a: u32,
    field_b: String,
}"#,
        );
        let item = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::Foo")
            .expect("should find struct Foo");
        let meta = item.metadata.as_ref().expect("should have metadata");
        let loc = meta["loc"].as_u64().expect("loc should be a number");
        assert_eq!(loc, 4, "struct spanning 4 lines should have loc=4");
    }

    // --- Test code detection tests ---

    #[test]
    fn test_fn_tagged_as_test() {
        let result = parse_source(
            "my_crate",
            r#"
            #[test]
            fn my_test() {
                assert!(true);
            }
        "#,
        );
        let test_fn = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::my_test")
            .expect("should find test function");
        assert!(
            test_fn.tags.contains(&"test".to_string()),
            "function with #[test] should have 'test' tag, got: {:?}",
            test_fn.tags
        );
    }

    #[test]
    fn cfg_test_module_tags_descendants() {
        let result = parse_source(
            "my_crate",
            r#"
            #[cfg(test)]
            mod tests {
                fn helper() {}
                #[test]
                fn it_works() {}
            }
        "#,
        );
        let test_mod = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::tests")
            .expect("should find tests module");
        assert!(
            test_mod.tags.contains(&"test".to_string()),
            "#[cfg(test)] module should have 'test' tag"
        );

        let helper = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::tests::helper")
            .expect("should find helper function");
        assert!(
            helper.tags.contains(&"test".to_string()),
            "function inside #[cfg(test)] module should inherit 'test' tag"
        );

        let it_works = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::tests::it_works")
            .expect("should find it_works function");
        assert!(
            it_works.tags.contains(&"test".to_string()),
            "#[test] fn inside #[cfg(test)] module should have 'test' tag"
        );
    }

    #[test]
    fn non_test_fn_has_no_test_tag() {
        let result = parse_source(
            "my_crate",
            r#"
            pub fn process_data() -> u32 { 42 }
            pub struct Foo;
        "#,
        );
        for item in &result.items {
            assert!(
                !item.tags.contains(&"test".to_string()),
                "non-test item '{}' should not have 'test' tag",
                item.qualified_name
            );
        }
    }

    #[test]
    fn test_fn_inside_non_test_module_detected() {
        let result = parse_source(
            "my_crate",
            r#"
            mod utils {
                #[test]
                fn test_helper() {}
                fn regular_fn() {}
            }
        "#,
        );
        let test_fn = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::utils::test_helper")
            .expect("should find test_helper");
        assert!(
            test_fn.tags.contains(&"test".to_string()),
            "#[test] fn should be tagged even outside #[cfg(test)] module"
        );

        let regular_fn = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::utils::regular_fn")
            .expect("should find regular_fn");
        assert!(
            !regular_fn.tags.contains(&"test".to_string()),
            "non-test fn in regular module should not be tagged"
        );

        let module = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my_crate::utils")
            .expect("should find utils module");
        assert!(
            !module.tags.contains(&"test".to_string()),
            "regular module should not be tagged as test"
        );
    }

    // --- Use-alias resolution tests (Phase 2) ---

    #[test]
    fn use_alias_resolves_impl_type() {
        let result = parse_source(
            "my_crate",
            r#"
            use other::Foo;
            impl Foo {
                pub fn bar(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("bar"))
            .expect("should find method bar");
        assert_eq!(
            method.parent_qualified_name,
            Some("other::Foo".to_string()),
            "use alias should resolve impl type to other::Foo"
        );
    }

    #[test]
    fn use_as_alias_resolves_impl_type() {
        let result = parse_source(
            "my_crate",
            r#"
            use other::Foo as Bar;
            impl Bar {
                pub fn baz(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("baz"))
            .expect("should find method baz");
        assert_eq!(
            method.parent_qualified_name,
            Some("other::Foo".to_string()),
            "use-as alias should resolve Bar back to other::Foo"
        );
    }

    #[test]
    fn grouped_use_emits_individual_depends_relations() {
        let result = parse_source("my_crate", "use std::collections::{HashMap, HashSet};");
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        let targets: Vec<&str> = depends
            .iter()
            .map(|d| d.target_qualified_name.as_str())
            .collect();
        assert!(
            targets.contains(&"std::collections::HashMap"),
            "should emit individual Depends for HashMap, got: {:?}",
            targets
        );
        assert!(
            targets.contains(&"std::collections::HashSet"),
            "should emit individual Depends for HashSet, got: {:?}",
            targets
        );
    }

    #[test]
    fn pub_use_emits_exports_edge() {
        let result = parse_source("my_crate", "pub use other::Foo;");
        let exports: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Exports)
            .collect();
        assert!(
            !exports.is_empty(),
            "pub use should emit Exports edge, got relations: {:?}",
            result.relations
        );
        assert_eq!(
            exports[0].target_qualified_name, "other::Foo",
            "Exports target should be the use path"
        );
    }

    #[test]
    fn use_aliases_scoped_to_module() {
        let result = parse_source(
            "my_crate",
            r#"
            mod inner {
                use other::Foo;
                impl Foo {
                    pub fn inner_method(&self) {}
                }
            }
            // This Foo should NOT resolve via inner's alias.
            impl Foo {
                pub fn outer_method(&self) {}
            }
        "#,
        );
        let inner_method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("inner_method"));
        assert!(inner_method.is_some(), "should find inner_method");
        assert_eq!(
            inner_method.unwrap().parent_qualified_name,
            Some("other::Foo".to_string()),
            "inner method should resolve via alias to other::Foo"
        );

        let outer_method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("outer_method"));
        assert!(outer_method.is_some(), "should find outer_method");
        assert_eq!(
            outer_method.unwrap().parent_qualified_name,
            Some("my_crate::Foo".to_string()),
            "outer method should NOT use inner module's alias"
        );
    }

    // --- Well-known container type tests (Phase 3) ---

    #[test]
    fn well_known_arc_impl_parents_under_inner_type() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct RaftEngine;
            pub trait Display { fn fmt(&self); }
            impl Display for Arc<RaftEngine> {
                fn fmt(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("fmt"));
        assert!(method.is_some(), "should find method fmt");
        assert_eq!(
            method.unwrap().parent_qualified_name,
            Some("my_crate::RaftEngine".to_string()),
            "impl Display for Arc<RaftEngine> should parent methods under RaftEngine"
        );
    }

    #[test]
    fn well_known_box_impl_parents_under_inner_type() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct MyError;
            pub trait Error { fn source(&self); }
            impl Error for Box<MyError> {
                fn source(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("source"));
        assert!(method.is_some(), "should find method source");
        assert_eq!(
            method.unwrap().parent_qualified_name,
            Some("my_crate::MyError".to_string()),
            "impl Error for Box<MyError> should parent methods under MyError"
        );
    }

    #[test]
    fn well_known_with_only_type_params_skips_impl() {
        let result = parse_source(
            "my_crate",
            r#"
            pub trait MyTrait { fn do_thing(&self); }
            impl<T> MyTrait for Arc<T> {
                fn do_thing(&self) {}
            }
        "#,
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(
            impls.is_empty(),
            "impl for Arc<T> with only type params should not emit Implements, got: {:?}",
            impls
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("do_thing"));
        assert!(
            method.is_none()
                || method.unwrap().parent_qualified_name.as_deref() != Some("my_crate::Arc"),
            "should not create phantom node for Arc"
        );
    }

    #[test]
    fn well_known_wrapper_metadata_annotated() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct RaftEngine;
            pub trait Display { fn fmt(&self); }
            impl Display for Arc<RaftEngine> {
                fn fmt(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("fmt"))
            .expect("should find method fmt");
        let meta = method.metadata.as_ref().expect("should have metadata");
        assert_eq!(
            meta["wrapped_by"].as_str(),
            Some("Arc"),
            "method metadata should annotate wrapped_by=Arc"
        );
    }

    #[test]
    fn non_well_known_generic_not_unwrapped() {
        let result = parse_source(
            "my_crate",
            r#"
            pub struct MyWrapper<T> { val: T }
            pub struct Foo;
            impl MyWrapper<Foo> {
                pub fn bar(&self) {}
            }
        "#,
        );
        let method = result
            .items
            .iter()
            .find(|i| i.qualified_name.ends_with("bar"))
            .expect("should find method bar");
        assert_eq!(
            method.parent_qualified_name,
            Some("my_crate::MyWrapper".to_string()),
            "non-well-known generic type should NOT be unwrapped"
        );
    }

    #[test]
    fn well_known_container_types_trait_method() {
        let analyzer = RustAnalyzer::new();
        let containers = CoreLanguageParser::well_known_container_types(&analyzer);
        assert!(
            containers.contains(&"Arc"),
            "should include Arc in well-known containers"
        );
        assert!(
            containers.contains(&"Box"),
            "should include Box in well-known containers"
        );
        assert!(
            containers.contains(&"Result"),
            "should include Result in well-known containers"
        );
    }
}
