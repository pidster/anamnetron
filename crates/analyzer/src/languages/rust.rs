//! Rust language analyzer using tree-sitter-rust.
//!
//! Extracts structural elements (modules, structs, enums, traits, functions)
//! and relationships (use dependencies, trait implementations, function calls)
//! from Rust source files using tree-sitter parsing.

use std::path::Path;

use svt_core::model::{EdgeKind, NodeKind};

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

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

    let source_bytes = source.as_bytes();
    let root = tree.root_node();

    // The initial module context is just the crate name (top-level items belong to the crate).
    let module_context = vec![crate_name.to_string()];

    visit_children(
        root,
        source_bytes,
        file_path,
        &module_context,
        items,
        relations,
        warnings,
    );
}

/// Visit all named children of a node, extracting structural items and relationships.
fn visit_children(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    items: &mut Vec<AnalysisItem>,
    relations: &mut Vec<AnalysisRelation>,
    warnings: &mut Vec<AnalysisWarning>,
) {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            visit_node(
                child,
                source,
                file_path,
                module_context,
                items,
                relations,
                warnings,
            );
        }
    }
}

/// Visit a single tree-sitter node and extract structural items and relationships.
fn visit_node(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    items: &mut Vec<AnalysisItem>,
    relations: &mut Vec<AnalysisRelation>,
    warnings: &mut Vec<AnalysisWarning>,
) {
    match node.kind() {
        "mod_item" => {
            visit_mod_item(
                node,
                source,
                file_path,
                module_context,
                items,
                relations,
                warnings,
            );
        }
        "struct_item" => {
            extract_named_item(
                node,
                source,
                file_path,
                module_context,
                NodeKind::Unit,
                "struct",
                items,
            );
        }
        "enum_item" => {
            extract_named_item(
                node,
                source,
                file_path,
                module_context,
                NodeKind::Unit,
                "enum",
                items,
            );
            // Extract enum variants as child Unit nodes.
            visit_enum_variants(node, source, file_path, module_context, items);
        }
        "trait_item" => {
            extract_named_item(
                node,
                source,
                file_path,
                module_context,
                NodeKind::Unit,
                "trait",
                items,
            );
        }
        "function_item" => {
            extract_named_item(
                node,
                source,
                file_path,
                module_context,
                NodeKind::Unit,
                "function",
                items,
            );
            // Descend into the function body to find call expressions.
            if let Some(body) = node.child_by_field_name("body") {
                visit_call_expressions(
                    body,
                    source,
                    file_path,
                    module_context,
                    relations,
                    warnings,
                );
            }
        }
        "impl_item" => {
            visit_impl_item(
                node,
                source,
                file_path,
                module_context,
                items,
                relations,
                warnings,
            );
        }
        "use_declaration" => {
            visit_use_declaration(node, source, module_context, relations);
        }
        _ => {
            // Recurse into children in case they contain items
            // (e.g., items inside cfg-gated blocks).
            visit_children(
                node,
                source,
                file_path,
                module_context,
                items,
                relations,
                warnings,
            );
        }
    }
}

/// Extract a named structural item (struct, enum, trait, function).
fn extract_named_item(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    kind: NodeKind,
    sub_kind: &str,
    items: &mut Vec<AnalysisItem>,
) {
    let Some(name) = item_name(node, source) else {
        return;
    };

    let parent_qualified_name = build_qualified_name(module_context);
    let qualified_name = format!("{parent_qualified_name}::{name}");
    let line = node.start_position().row + 1;
    let source_ref = format!("{}:{line}", file_path.display());

    items.push(AnalysisItem {
        qualified_name,
        kind,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: Some(parent_qualified_name),
        source_ref,
        language: "rust".to_string(),
    });
}

/// Extract variants from an `enum_item` node.
///
/// Each variant is emitted as a `Unit` node with sub_kind `"variant"`,
/// parented under the enum's qualified name. This enables `must_contain`
/// constraints to detect specific enum variants (e.g., CLI subcommands).
fn visit_enum_variants(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    items: &mut Vec<AnalysisItem>,
) {
    let Some(enum_name) = item_name(node, source) else {
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
            if let Some(variant_name) = item_name(child, source) {
                let line = child.start_position().row + 1;
                items.push(AnalysisItem {
                    qualified_name: format!("{enum_qn}::{variant_name}"),
                    kind: NodeKind::Unit,
                    sub_kind: "variant".to_string(),
                    parent_qualified_name: Some(enum_qn.clone()),
                    source_ref: format!("{}:{line}", file_path.display()),
                    language: "rust".to_string(),
                });
            }
        }
    }
}

/// Handle a `mod_item` node. If it has a body (inline module), descend into it.
/// Otherwise, it's a declaration-only module (`mod foo;`).
fn visit_mod_item(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    items: &mut Vec<AnalysisItem>,
    relations: &mut Vec<AnalysisRelation>,
    warnings: &mut Vec<AnalysisWarning>,
) {
    let Some(name) = item_name(node, source) else {
        return;
    };

    let parent_qualified_name = build_qualified_name(module_context);
    let qualified_name = format!("{parent_qualified_name}::{name}");
    let line = node.start_position().row + 1;
    let source_ref = format!("{}:{line}", file_path.display());

    items.push(AnalysisItem {
        qualified_name: qualified_name.clone(),
        kind: NodeKind::Component,
        sub_kind: "module".to_string(),
        parent_qualified_name: Some(parent_qualified_name),
        source_ref,
        language: "rust".to_string(),
    });

    // If the module has a body (inline module), descend into its declarations.
    if let Some(body) = node.child_by_field_name("body") {
        let mut child_context = module_context.to_vec();
        child_context.push(name);
        visit_children(
            body,
            source,
            file_path,
            &child_context,
            items,
            relations,
            warnings,
        );
    }
}

/// Handle an `impl_item` node. Extract methods as functions scoped under the
/// type being implemented, and emit `Implements` relations for trait impls.
fn visit_impl_item(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    items: &mut Vec<AnalysisItem>,
    relations: &mut Vec<AnalysisRelation>,
    warnings: &mut Vec<AnalysisWarning>,
) {
    // Check for `impl Trait for Type` — emit an Implements relation.
    let trait_node = node.child_by_field_name("trait");
    let type_node = node.child_by_field_name("type");

    if let (Some(trait_n), Some(type_n)) = (trait_node, type_node) {
        if let (Ok(trait_name), Ok(type_name)) = (
            trait_n.utf8_text(source.as_ref()),
            type_n.utf8_text(source.as_ref()),
        ) {
            let module_qn = build_qualified_name(module_context);
            let source_qn = format!("{module_qn}::{type_name}");
            let target_qn = trait_name.to_string();

            relations.push(AnalysisRelation {
                source_qualified_name: source_qn,
                target_qualified_name: target_qn,
                kind: EdgeKind::Implements,
            });
        }
    }

    // Find the body of the impl block and extract function items from it.
    if let Some(body) = node.child_by_field_name("body") {
        // Methods inside impl blocks are extracted with the current module context
        // (not scoped under the type name — that would create a false hierarchy).
        visit_children(
            body,
            source,
            file_path,
            module_context,
            items,
            relations,
            warnings,
        );
    }
}

/// Handle a `use_declaration` node. Extract the use path and emit a `Depends` relation.
///
/// For simple uses like `use foo::bar;`, the argument is `foo::bar`.
/// For grouped uses like `use foo::{bar, baz};`, emit a dependency on the parent path.
fn visit_use_declaration(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    module_context: &[String],
    relations: &mut Vec<AnalysisRelation>,
) {
    let source_qn = build_qualified_name(module_context);

    if let Some(argument) = node.child_by_field_name("argument") {
        let target = argument.utf8_text(source).unwrap_or_default().to_string();
        let target = target.replace(' ', "");

        if target.is_empty() {
            return;
        }

        // For grouped uses like `foo::{bar, baz}`, extract the parent path.
        if target.contains('{') {
            if let Some(parent_path) = target.split("::{").next() {
                if !parent_path.is_empty() {
                    relations.push(AnalysisRelation {
                        source_qualified_name: source_qn,
                        target_qualified_name: parent_path.to_string(),
                        kind: EdgeKind::Depends,
                    });
                }
            }
        } else {
            relations.push(AnalysisRelation {
                source_qualified_name: source_qn,
                target_qualified_name: target,
                kind: EdgeKind::Depends,
            });
        }
    }
}

/// Recursively walk a subtree looking for `call_expression` nodes
/// and emit `Calls` relations for syntactically resolvable calls.
///
/// Method calls (field expressions like `x.foo()`) cannot be resolved
/// by tree-sitter alone (no type information), so they emit a warning instead.
fn visit_call_expressions(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    relations: &mut Vec<AnalysisRelation>,
    warnings: &mut Vec<AnalysisWarning>,
) {
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        if child.kind() == "call_expression" {
            if let Some(function) = child.child_by_field_name("function") {
                let source_qn = build_qualified_name(module_context);
                let line = child.start_position().row + 1;

                match function.kind() {
                    "identifier" | "scoped_identifier" => {
                        // Simple function call or path-qualified call (e.g., `foo()` or `mod::foo()`).
                        if let Ok(callee) = function.utf8_text(source) {
                            let callee = callee.replace(' ', "");
                            if !callee.is_empty() {
                                relations.push(AnalysisRelation {
                                    source_qualified_name: source_qn,
                                    target_qualified_name: callee,
                                    kind: EdgeKind::Calls,
                                });
                            }
                        }
                    }
                    "field_expression" => {
                        // Method call (e.g., `x.foo()`) — cannot resolve the receiver type.
                        if let Ok(callee_text) = function.utf8_text(source) {
                            warnings.push(AnalysisWarning {
                                source_ref: format!("{}:{line}", file_path.display()),
                                message: format!(
                                    "method call `{callee_text}(...)` cannot be resolved \
                                     without type information"
                                ),
                            });
                        }
                    }
                    _ => {
                        // Other call forms (closures, etc.) — skip silently.
                    }
                }
            }
        }

        // Recurse into children to find nested call expressions.
        visit_call_expressions(
            child,
            source,
            file_path,
            module_context,
            relations,
            warnings,
        );
    }
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
            methods.iter().any(|m| m.qualified_name == "my_crate::bar"),
            "should extract impl method, got: {:?}",
            methods
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
    fn grouped_use_generates_depends_on_parent_path() {
        let result = parse_source("my_crate", "use std::collections::{HashMap, HashSet};");
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            !depends.is_empty(),
            "grouped use should generate Depends relation on parent path"
        );
        assert_eq!(
            depends[0].target_qualified_name, "std::collections",
            "grouped use should depend on parent path"
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
            fn main() {
                let x = String::new();
                x.push_str("hello");
            }
        "#,
        );
        // Method calls can't be resolved by tree-sitter — should produce a warning.
        let method_warnings: Vec<_> = result
            .warnings
            .iter()
            .filter(|w| w.message.contains("cannot be resolved"))
            .collect();
        assert!(
            !method_warnings.is_empty(),
            "method call should produce a warning about unresolvable receiver"
        );
    }

    #[test]
    fn method_call_handled_gracefully() {
        let result = parse_source(
            "my_crate",
            r#"
            fn main() {
                let x = String::new();
                x.push_str("hello");
            }
        "#,
        );
        // Method calls can't be resolved by tree-sitter — should not crash.
        // Should not produce a Calls relation for method calls.
        let method_calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls && r.target_qualified_name.contains("push_str"))
            .collect();
        assert!(
            method_calls.is_empty(),
            "method call should not generate a Calls relation"
        );
    }
}
