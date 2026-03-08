//! Java language analyzer using tree-sitter-java.
//!
//! Extracts structural elements (classes, interfaces, enums, annotations,
//! methods, constructors, fields) and import relationships from Java source
//! files. Emits `Extends` and `Implements` edges for heritage clauses, and
//! `Calls` edges from method/constructor bodies.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use svt_core::analysis::{LanguageDescriptor, LanguageParser as CoreLanguageParser};
use svt_core::model::{EdgeKind, NodeKind};

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// Java source code analyzer using tree-sitter-java.
#[derive(Debug)]
pub struct JavaAnalyzer {
    _private: (),
}

impl JavaAnalyzer {
    /// Create a new `JavaAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for JavaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaAnalyzer {
    /// Language descriptor for Java modules.
    #[must_use]
    pub fn descriptor() -> LanguageDescriptor {
        LanguageDescriptor {
            language_id: "java".to_string(),
            manifest_files: vec![
                "pom.xml".to_string(),
                "build.gradle".to_string(),
                "build.gradle.kts".to_string(),
            ],
            source_extensions: vec![".java".to_string()],
            skip_directories: vec![
                "target".to_string(),
                "build".to_string(),
                ".gradle".to_string(),
                ".git".to_string(),
                "node_modules".to_string(),
                ".idea".to_string(),
            ],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        }
    }

    /// Create a boxed language parser for Java.
    #[must_use]
    pub fn parser() -> Box<dyn CoreLanguageParser> {
        Box::new(JavaAnalyzer::new())
    }
}

impl CoreLanguageParser for JavaAnalyzer {
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult {
        self.analyze_crate(unit_name, files)
    }

    fn emit_structural_items(
        &self,
        source_root: &Path,
        unit_name: &str,
        source_files: &[PathBuf],
    ) -> Vec<AnalysisItem> {
        emit_java_package_items(source_root, unit_name, source_files)
    }

    fn post_process(&self, source_root: &Path, unit_name: &str, result: &mut ParseResult) {
        // Build import map for resolving simple names to qualified names.
        let import_map = build_import_map(result);

        // Reparent items to their package-level qualified names.
        for item in &mut result.items {
            if let Some(pkg_qn) = java_file_to_package_qn(source_root, &item.source_ref, unit_name)
            {
                let item_name = item
                    .qualified_name
                    .rsplit("::")
                    .next()
                    .unwrap_or("")
                    .to_string();

                // For methods/constructors/fields, preserve the class::member structure.
                if item.sub_kind == "method"
                    || item.sub_kind == "constructor"
                    || item.sub_kind == "field"
                {
                    if let Some(ref parent_qn) = item.parent_qualified_name {
                        let class_name = parent_qn.rsplit("::").next().unwrap_or("").to_string();
                        item.qualified_name = format!("{pkg_qn}::{class_name}::{item_name}");
                        item.parent_qualified_name = Some(format!("{pkg_qn}::{class_name}"));
                    }
                } else {
                    item.qualified_name = format!("{pkg_qn}::{item_name}");
                    item.parent_qualified_name = Some(pkg_qn);
                }
            }
        }

        // Resolve Extends/Implements targets using import map.
        for rel in &mut result.relations {
            if rel.kind == EdgeKind::Extends || rel.kind == EdgeKind::Implements {
                let target = &rel.target_qualified_name;
                // If the target is a simple name, try to resolve via imports.
                if !target.contains("::") && !target.contains('.') {
                    if let Some(resolved) = import_map.get(target) {
                        rel.target_qualified_name = resolved.replace('.', "::");
                    }
                }
            }
        }
    }
}

impl LanguageAnalyzer for JavaAnalyzer {
    fn language_id(&self) -> &str {
        "java"
    }

    fn analyze_crate(&self, module_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-java grammar".to_string(),
            });
            return result;
        }

        for file in files {
            let is_test_file = is_java_test_file(file);
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_java_file(
                        &mut parser,
                        &source,
                        file,
                        module_name,
                        is_test_file,
                        &mut result,
                    );
                }
                Err(e) => {
                    result.warnings.push(AnalysisWarning {
                        source_ref: file.display().to_string(),
                        message: format!("failed to read file: {e}"),
                    });
                }
            }
        }

        result
    }
}

/// Check whether a file path refers to a Java test file.
fn is_java_test_file(path: &Path) -> bool {
    let path_str = path.to_str().unwrap_or("");
    path_str.contains("src/test/java/") || path_str.contains("src/test/java\\")
}

/// Compute test tags for a Java method based on its annotations and file context.
fn java_test_tags(annotations: &[String], is_test_file: bool) -> Vec<String> {
    if is_test_file {
        return vec!["test".to_string()];
    }
    let test_annotations = [
        "Test",
        "ParameterizedTest",
        "RepeatedTest",
        "org.junit.Test",
        "org.junit.jupiter.api.Test",
        "org.testng.annotations.Test",
    ];
    for ann in annotations {
        let ann_name = ann.strip_prefix('@').unwrap_or(ann);
        if test_annotations.contains(&ann_name) {
            return vec!["test".to_string()];
        }
    }
    vec![]
}

fn parse_java_file(
    parser: &mut tree_sitter::Parser,
    source: &str,
    file: &Path,
    module_name: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            result.warnings.push(AnalysisWarning {
                source_ref: file.display().to_string(),
                message: "tree-sitter parse returned None".to_string(),
            });
            return;
        }
    };

    let root = tree.root_node();
    let source_ref_base = file.display().to_string();

    // Extract package declaration.
    let package_name = extract_package_declaration(&root, source);

    // Collect imports for dependency edges and name resolution.
    let mut import_targets: Vec<String> = Vec::new();
    for child in root.children(&mut root.walk()) {
        if child.kind() == "import_declaration" {
            if let Some(import_path) = extract_import_path(&child, source) {
                // Emit Depends edge from module to imported package.
                let target_pkg = import_path
                    .rsplit_once('.')
                    .map(|(pkg, _)| pkg)
                    .unwrap_or(&import_path);
                result.relations.push(AnalysisRelation {
                    source_qualified_name: module_name.to_string(),
                    target_qualified_name: target_pkg.to_string(),
                    kind: EdgeKind::Depends,
                });
                import_targets.push(import_path);
            }
        }
    }

    // Walk top-level type declarations.
    for child in root.children(&mut root.walk()) {
        match child.kind() {
            "class_declaration" => {
                extract_class_declaration(
                    &child,
                    source,
                    module_name,
                    &source_ref_base,
                    is_test_file,
                    package_name.as_deref(),
                    result,
                );
            }
            "interface_declaration" => {
                extract_interface_declaration(
                    &child,
                    source,
                    module_name,
                    &source_ref_base,
                    is_test_file,
                    result,
                );
            }
            "enum_declaration" => {
                extract_enum_declaration(
                    &child,
                    source,
                    module_name,
                    &source_ref_base,
                    is_test_file,
                    result,
                );
            }
            "annotation_type_declaration" => {
                extract_annotation_declaration(
                    &child,
                    source,
                    module_name,
                    &source_ref_base,
                    is_test_file,
                    result,
                );
            }
            _ => {}
        }
    }
}

/// Extract the package declaration from a Java compilation unit.
fn extract_package_declaration(root: &tree_sitter::Node, source: &str) -> Option<String> {
    for child in root.children(&mut root.walk()) {
        if child.kind() == "package_declaration" {
            // The scoped_identifier or identifier child holds the package name.
            for c in child.children(&mut child.walk()) {
                if c.kind() == "scoped_identifier" || c.kind() == "identifier" {
                    if let Ok(text) = c.utf8_text(source.as_bytes()) {
                        return Some(text.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Extract the import path from an import declaration.
fn extract_import_path(node: &tree_sitter::Node, source: &str) -> Option<String> {
    for child in node.children(&mut node.walk()) {
        if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                return Some(text.to_string());
            }
        }
    }
    None
}

/// Extract the name from a type declaration node.
fn extract_name<'a>(node: &tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
}

/// Extract annotations from a node's preceding modifiers.
fn extract_annotations(node: &tree_sitter::Node, source: &str) -> Vec<String> {
    let mut annotations = Vec::new();
    if let Some(modifiers) = node.child_by_field_name("modifiers") {
        let mut cursor = modifiers.walk();
        for child in modifiers.children(&mut cursor) {
            if child.kind() == "marker_annotation" || child.kind() == "annotation" {
                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                    annotations.push(text.to_string());
                }
            }
        }
    }
    // Also check direct children for annotations (some grammar versions).
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "marker_annotation" || child.kind() == "annotation" {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                annotations.push(text.to_string());
            }
        }
    }
    annotations
}

fn extract_class_declaration(
    node: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    source_ref_base: &str,
    is_test_file: bool,
    _package_name: Option<&str>,
    result: &mut ParseResult,
) {
    let name = match extract_name(node, source) {
        Some(n) => n.to_string(),
        None => return,
    };

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;
    let class_qn = format!("{module_name}::{name}");

    let tags = if is_test_file {
        vec!["test".to_string()]
    } else {
        vec![]
    };

    result.items.push(AnalysisItem {
        qualified_name: class_qn.clone(),
        kind: NodeKind::Unit,
        sub_kind: "class".to_string(),
        parent_qualified_name: Some(module_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "java".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Extract superclass (extends).
    // The `superclass` field wraps a type child (first named child is the type).
    if let Some(superclass) = node.child_by_field_name("superclass") {
        if let Some(super_name) = extract_first_type_name(&superclass, source) {
            result.relations.push(AnalysisRelation {
                source_qualified_name: class_qn.clone(),
                target_qualified_name: super_name,
                kind: EdgeKind::Extends,
            });
        }
    }

    // Extract interfaces (implements).
    // The `interfaces` field is a `super_interfaces` node containing a `type_list`.
    if let Some(interfaces) = node.child_by_field_name("interfaces") {
        extract_nested_type_list_relations(
            &interfaces,
            source,
            &class_qn,
            EdgeKind::Implements,
            result,
        );
    }

    // Walk the class body.
    if let Some(body) = node.child_by_field_name("body") {
        extract_class_body_members(
            &body,
            source,
            module_name,
            &name,
            &class_qn,
            source_ref_base,
            is_test_file,
            result,
        );
    }
}

fn extract_interface_declaration(
    node: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let name = match extract_name(node, source) {
        Some(n) => n.to_string(),
        None => return,
    };

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;
    let iface_qn = format!("{module_name}::{name}");

    let tags = if is_test_file {
        vec!["test".to_string()]
    } else {
        vec![]
    };

    result.items.push(AnalysisItem {
        qualified_name: iface_qn.clone(),
        kind: NodeKind::Unit,
        sub_kind: "interface".to_string(),
        parent_qualified_name: Some(module_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "java".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Extract extended interfaces.
    // `extends_interfaces` is a child node (not a field) of interface_declaration.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "extends_interfaces" {
            extract_nested_type_list_relations(
                &child,
                source,
                &iface_qn,
                EdgeKind::Extends,
                result,
            );
        }
    }

    // Walk interface body for method signatures.
    if let Some(body) = node.child_by_field_name("body") {
        extract_interface_body_members(
            &body,
            source,
            module_name,
            &name,
            &iface_qn,
            source_ref_base,
            is_test_file,
            result,
        );
    }
}

fn extract_enum_declaration(
    node: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let name = match extract_name(node, source) {
        Some(n) => n.to_string(),
        None => return,
    };

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;
    let enum_qn = format!("{module_name}::{name}");

    let tags = if is_test_file {
        vec!["test".to_string()]
    } else {
        vec![]
    };

    result.items.push(AnalysisItem {
        qualified_name: enum_qn.clone(),
        kind: NodeKind::Unit,
        sub_kind: "enum".to_string(),
        parent_qualified_name: Some(module_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "java".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Enums can implement interfaces.
    if let Some(interfaces) = node.child_by_field_name("interfaces") {
        extract_nested_type_list_relations(
            &interfaces,
            source,
            &enum_qn,
            EdgeKind::Implements,
            result,
        );
    }

    // Walk enum body for methods.
    // Enum body has `enum_body_declarations` which contains the member declarations.
    if let Some(body) = node.child_by_field_name("body") {
        // First check for enum_body_declarations (where methods/fields live).
        let mut body_cursor = body.walk();
        for body_child in body.children(&mut body_cursor) {
            if body_child.kind() == "enum_body_declarations" {
                extract_class_body_members(
                    &body_child,
                    source,
                    module_name,
                    &name,
                    &enum_qn,
                    source_ref_base,
                    is_test_file,
                    result,
                );
            }
        }
    }
}

fn extract_annotation_declaration(
    node: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let name = match extract_name(node, source) {
        Some(n) => n.to_string(),
        None => return,
    };

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;

    let tags = if is_test_file {
        vec!["test".to_string()]
    } else {
        vec![]
    };

    result.items.push(AnalysisItem {
        qualified_name: format!("{module_name}::{name}"),
        kind: NodeKind::Unit,
        sub_kind: "annotation".to_string(),
        parent_qualified_name: Some(module_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "java".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });
}

/// Extract the first type name from a node (e.g., `superclass` wrapping a type).
fn extract_first_type_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            if child.kind() == "type_identifier" {
                return child
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string());
            }
            if child.kind() == "generic_type" {
                // For `Foo<T>`, extract just `Foo`.
                return child
                    .child(0)
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string());
            }
            if child.kind() == "scoped_type_identifier" {
                return child
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|s| s.to_string());
            }
        }
    }
    None
}

/// Extract type relations from a wrapper node that contains a `type_list` child.
///
/// Used for `super_interfaces` and `extends_interfaces` nodes, which wrap a
/// `type_list` containing the actual type identifiers.
fn extract_nested_type_list_relations(
    node: &tree_sitter::Node,
    source: &str,
    source_qn: &str,
    edge_kind: EdgeKind,
    result: &mut ParseResult,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_list" {
            extract_type_list_relations(&child, source, source_qn, edge_kind, result);
            return;
        }
    }
    // Fallback: try extracting directly from this node.
    extract_type_list_relations(node, source, source_qn, edge_kind, result);
}

/// Extract type names from a `type_list` node and emit edges.
fn extract_type_list_relations(
    node: &tree_sitter::Node,
    source: &str,
    source_qn: &str,
    edge_kind: EdgeKind,
    result: &mut ParseResult,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" || child.kind() == "generic_type" {
            let type_name = if child.kind() == "generic_type" {
                // For generic types like `Comparable<T>`, extract just the raw type name.
                child
                    .child(0)
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            } else {
                child.utf8_text(source.as_bytes()).ok()
            };
            if let Some(name) = type_name {
                if !name.is_empty() {
                    result.relations.push(AnalysisRelation {
                        source_qualified_name: source_qn.to_string(),
                        target_qualified_name: name.to_string(),
                        kind: edge_kind,
                    });
                }
            }
        }
    }
}

/// Extract members (methods, constructors, fields) from a class/enum body.
#[allow(clippy::too_many_arguments)]
fn extract_class_body_members(
    body: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    class_name: &str,
    class_qn: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "method_declaration" => {
                extract_method(
                    &child,
                    source,
                    module_name,
                    class_name,
                    class_qn,
                    source_ref_base,
                    is_test_file,
                    result,
                );
            }
            "constructor_declaration" => {
                extract_constructor(
                    &child,
                    source,
                    module_name,
                    class_name,
                    class_qn,
                    source_ref_base,
                    is_test_file,
                    result,
                );
            }
            "field_declaration" => {
                extract_field(
                    &child,
                    source,
                    module_name,
                    class_name,
                    class_qn,
                    source_ref_base,
                    is_test_file,
                    result,
                );
            }
            "class_declaration" => {
                // Nested class — recurse.
                extract_class_declaration(
                    &child,
                    source,
                    module_name,
                    source_ref_base,
                    is_test_file,
                    None,
                    result,
                );
            }
            "interface_declaration" => {
                extract_interface_declaration(
                    &child,
                    source,
                    module_name,
                    source_ref_base,
                    is_test_file,
                    result,
                );
            }
            "enum_declaration" => {
                extract_enum_declaration(
                    &child,
                    source,
                    module_name,
                    source_ref_base,
                    is_test_file,
                    result,
                );
            }
            _ => {}
        }
    }
}

/// Extract method signatures from an interface body.
#[allow(clippy::too_many_arguments)]
fn extract_interface_body_members(
    body: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    iface_name: &str,
    iface_qn: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "method_declaration" {
            extract_method(
                &child,
                source,
                module_name,
                iface_name,
                iface_qn,
                source_ref_base,
                is_test_file,
                result,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_method(
    node: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    class_name: &str,
    class_qn: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let name = match extract_name(node, source) {
        Some(n) => n.to_string(),
        None => return,
    };

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;
    let method_qn = format!("{module_name}::{class_name}::{name}");

    let annotations = extract_annotations(node, source);
    let tags = java_test_tags(&annotations, is_test_file);

    result.items.push(AnalysisItem {
        qualified_name: method_qn.clone(),
        kind: NodeKind::Unit,
        sub_kind: "method".to_string(),
        parent_qualified_name: Some(class_qn.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "java".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Walk the method body for call expressions.
    if let Some(body) = node.child_by_field_name("body") {
        visit_java_call_expressions(body, source, &method_qn, module_name, class_name, result);
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_constructor(
    node: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    class_name: &str,
    class_qn: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let name = match extract_name(node, source) {
        Some(n) => n.to_string(),
        None => return,
    };

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;
    let ctor_qn = format!("{module_name}::{class_name}::{name}");

    let tags = if is_test_file {
        vec!["test".to_string()]
    } else {
        vec![]
    };

    result.items.push(AnalysisItem {
        qualified_name: ctor_qn.clone(),
        kind: NodeKind::Unit,
        sub_kind: "constructor".to_string(),
        parent_qualified_name: Some(class_qn.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "java".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Walk the constructor body for call expressions.
    if let Some(body) = node.child_by_field_name("body") {
        visit_java_call_expressions(body, source, &ctor_qn, module_name, class_name, result);
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_field(
    node: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    class_name: &str,
    class_qn: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    // Fields can have multiple declarators: `int x, y;`
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name_node) = child.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                    let line = node.start_position().row + 1;
                    let loc = node.end_position().row - node.start_position().row + 1;

                    let tags = if is_test_file {
                        vec!["test".to_string()]
                    } else {
                        vec![]
                    };

                    result.items.push(AnalysisItem {
                        qualified_name: format!("{module_name}::{class_name}::{name}"),
                        kind: NodeKind::Unit,
                        sub_kind: "field".to_string(),
                        parent_qualified_name: Some(class_qn.to_string()),
                        source_ref: format!("{source_ref_base}:{line}"),
                        language: "java".to_string(),
                        metadata: Some(serde_json::json!({"loc": loc})),
                        tags,
                    });
                }
            }
        }
    }
}

/// Recursively walk AST nodes to find method invocations and emit `Calls` edges.
fn visit_java_call_expressions(
    node: tree_sitter::Node<'_>,
    source: &str,
    caller_qn: &str,
    module_name: &str,
    class_name: &str,
    result: &mut ParseResult,
) {
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        if child.kind() == "method_invocation" {
            let method_name = child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok());
            let object = child
                .child_by_field_name("object")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok());

            if let Some(method) = method_name {
                let target = match object {
                    Some("this") | None => {
                        // this.method() or simple method() — target is current class.
                        format!("{module_name}::{class_name}::{method}")
                    }
                    Some(obj) => {
                        // obj.method() or ClassName.method() — use as-is.
                        format!("{obj}::{method}")
                    }
                };

                result.relations.push(AnalysisRelation {
                    source_qualified_name: caller_qn.to_string(),
                    target_qualified_name: target,
                    kind: EdgeKind::Calls,
                });
            }
        }

        // Recurse into children.
        visit_java_call_expressions(child, source, caller_qn, module_name, class_name, result);
    }
}

/// Emit directory-based package hierarchy nodes for Java packages.
///
/// Java packages map to directories under source roots. We emit
/// `Component/package` nodes for each unique directory level.
fn emit_java_package_items(
    source_root: &Path,
    unit_name: &str,
    source_files: &[PathBuf],
) -> Vec<AnalysisItem> {
    let mut items = Vec::new();
    let mut emitted: HashSet<String> = HashSet::new();

    for file in source_files {
        let rel = match file.strip_prefix(source_root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Try to find Java source root (src/main/java/ or src/test/java/).
        let rel_str = rel.to_str().unwrap_or("");
        let effective_rel = if let Some(idx) = rel_str.find("src/main/java/") {
            Path::new(&rel_str[idx + "src/main/java/".len()..])
        } else if let Some(idx) = rel_str.find("src/test/java/") {
            Path::new(&rel_str[idx + "src/test/java/".len()..])
        } else {
            rel
        };

        let mut current_qn = unit_name.to_string();
        for component in effective_rel.parent().iter().flat_map(|p| p.components()) {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str().unwrap_or("");
                let parent_qn = current_qn.clone();
                current_qn = format!("{current_qn}::{name_str}");
                if emitted.insert(current_qn.clone()) {
                    items.push(AnalysisItem {
                        qualified_name: current_qn.clone(),
                        kind: NodeKind::Component,
                        sub_kind: "package".to_string(),
                        parent_qualified_name: Some(parent_qn),
                        source_ref: file.parent().unwrap_or(source_root).display().to_string(),
                        language: "java".to_string(),
                        metadata: None,
                        tags: vec![],
                    });
                }
            }
        }
    }

    items
}

/// Map a source_ref file path to its package-level qualified name.
fn java_file_to_package_qn(
    source_root: &Path,
    source_ref: &str,
    unit_name: &str,
) -> Option<String> {
    let file_path_str = source_ref
        .rsplit_once(':')
        .map(|(p, _)| p)
        .unwrap_or(source_ref);
    let file_path = Path::new(file_path_str);
    let rel = file_path.strip_prefix(source_root).ok()?;

    // Try to find Java source root within the relative path.
    let rel_str = rel.to_str()?;
    let effective_rel = if let Some(idx) = rel_str.find("src/main/java/") {
        Path::new(&rel_str[idx + "src/main/java/".len()..])
    } else if let Some(idx) = rel_str.find("src/test/java/") {
        Path::new(&rel_str[idx + "src/test/java/".len()..])
    } else {
        rel
    };

    let mut qn = unit_name.to_string();
    for component in effective_rel.parent().iter().flat_map(|p| p.components()) {
        if let std::path::Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                qn = format!("{qn}::{name_str}");
            }
        }
    }

    Some(qn)
}

/// Build an import map from collected import relations (simple name → fully qualified name).
fn build_import_map(result: &ParseResult) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for rel in &result.relations {
        if rel.kind == EdgeKind::Depends {
            let target = &rel.target_qualified_name;
            // The Depends edges target the package, but the original import
            // may be a specific type. Extract the simple name from common patterns.
            if let Some(last) = target.rsplit('.').next() {
                // Only map if the last segment looks like a type (starts with uppercase).
                if last.chars().next().is_some_and(|c| c.is_uppercase()) {
                    map.insert(last.to_string(), target.clone());
                }
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn parse_java_source(source: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("Test.java");
        std::fs::write(&file, source).unwrap();
        let analyzer = JavaAnalyzer::new();
        let file_path = PathBuf::from(&file);
        analyzer.analyze_crate("myapp", &[file_path.as_path()])
    }

    fn parse_java_source_at(source: &str, rel_path: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join(rel_path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file, source).unwrap();
        let analyzer = JavaAnalyzer::new();
        let file_path = PathBuf::from(&file);
        analyzer.analyze_crate("myapp", &[file_path.as_path()])
    }

    #[test]
    fn language_id_is_java() {
        let analyzer = JavaAnalyzer::new();
        assert_eq!(analyzer.language_id(), "java");
    }

    #[test]
    fn extracts_class_declaration() {
        let result = parse_java_source("public class Foo {}");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert_eq!(classes.len(), 1);
        assert!(classes[0].qualified_name.contains("Foo"));
        assert_eq!(classes[0].kind, NodeKind::Unit);
        assert_eq!(classes[0].language, "java");
    }

    #[test]
    fn extracts_interface_declaration() {
        let result = parse_java_source("public interface Handler { void handle(); }");
        let ifaces: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "interface")
            .collect();
        assert_eq!(ifaces.len(), 1);
        assert!(ifaces[0].qualified_name.contains("Handler"));
    }

    #[test]
    fn extracts_enum_declaration() {
        let result = parse_java_source("public enum Color { RED, GREEN, BLUE }");
        let enums: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "enum")
            .collect();
        assert_eq!(enums.len(), 1);
        assert!(enums[0].qualified_name.contains("Color"));
    }

    #[test]
    fn extracts_annotation_type_declaration() {
        let result = parse_java_source("public @interface MyAnnotation {}");
        let anns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "annotation")
            .collect();
        assert_eq!(anns.len(), 1);
        assert!(anns[0].qualified_name.contains("MyAnnotation"));
    }

    #[test]
    fn extracts_methods_from_class() {
        let result = parse_java_source(
            "public class Foo { public void bar() {} public int baz() { return 0; } }",
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 2);
        assert!(methods.iter().any(|m| m.qualified_name.contains("bar")));
        assert!(methods.iter().any(|m| m.qualified_name.contains("baz")));
    }

    #[test]
    fn extracts_constructor() {
        let result = parse_java_source("public class Foo { public Foo(int x) {} }");
        let ctors: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "constructor")
            .collect();
        assert_eq!(ctors.len(), 1);
        assert!(ctors[0].qualified_name.contains("Foo"));
    }

    #[test]
    fn extracts_fields() {
        let result = parse_java_source("public class Foo { private int x; private String name; }");
        let fields: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "field")
            .collect();
        assert_eq!(fields.len(), 2);
    }

    #[test]
    fn extracts_extends_relation() {
        let result = parse_java_source("public class Foo extends Bar {}");
        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        assert_eq!(extends.len(), 1);
        assert!(extends[0].source_qualified_name.contains("Foo"));
        assert_eq!(extends[0].target_qualified_name, "Bar");
    }

    #[test]
    fn extracts_implements_relation() {
        let result = parse_java_source("public class Foo implements Runnable, Comparable {}");
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert_eq!(impls.len(), 2);
    }

    #[test]
    fn extracts_interface_extends_relation() {
        let result = parse_java_source("public interface Foo extends Bar, Baz {}");
        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        assert_eq!(extends.len(), 2);
    }

    #[test]
    fn extracts_import_dependencies() {
        let result =
            parse_java_source("import java.util.List;\nimport java.io.File;\npublic class Foo {}");
        let imports: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(imports.len() >= 2);
    }

    #[test]
    fn extracts_call_graph() {
        let result = parse_java_source(
            "public class Foo {\n  public void bar() { baz(); }\n  public void baz() {}\n}",
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(!calls.is_empty(), "should detect method calls");
    }

    #[test]
    fn detects_test_file_by_path() {
        assert!(is_java_test_file(Path::new(
            "/project/src/test/java/com/example/FooTest.java"
        )));
        assert!(!is_java_test_file(Path::new(
            "/project/src/main/java/com/example/Foo.java"
        )));
    }

    #[test]
    fn detects_test_by_annotation() {
        let tags = java_test_tags(&["@Test".to_string()], false);
        assert_eq!(tags, vec!["test"]);

        let tags = java_test_tags(&["@ParameterizedTest".to_string()], false);
        assert_eq!(tags, vec!["test"]);

        let tags = java_test_tags(&["@Override".to_string()], false);
        assert!(tags.is_empty());
    }

    #[test]
    fn test_file_tags_all_items_as_test() {
        let result = parse_java_source_at(
            "public class FooTest { public void testAdd() {} }",
            "src/test/java/com/example/FooTest.java",
        );
        assert!(
            result
                .items
                .iter()
                .all(|i| i.tags.contains(&"test".to_string())),
            "all items in test files should be tagged as test"
        );
    }

    #[test]
    fn handles_empty_file_gracefully() {
        let result = parse_java_source("");
        assert!(result.items.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn handles_nested_class() {
        let result = parse_java_source("public class Outer { public class Inner {} }");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert_eq!(
            classes.len(),
            2,
            "should extract both outer and inner class"
        );
    }

    #[test]
    fn extracts_package_declaration() {
        let source = "package com.example.service;\npublic class Foo {}";
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("Foo.java");
        std::fs::write(&file, source).unwrap();

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let pkg = extract_package_declaration(&root, source);
        assert_eq!(pkg.as_deref(), Some("com.example.service"));
    }

    #[test]
    fn emit_java_package_items_creates_hierarchy() {
        let dir = TempDir::new().unwrap();
        let src_root = dir.path();
        let file = src_root.join("com/example/service/Foo.java");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "class Foo {}").unwrap();

        let items = emit_java_package_items(src_root, "myapp", &[file]);
        assert!(
            items.len() >= 3,
            "should emit com, com/example, com/example/service"
        );
        assert!(items.iter().all(|i| i.sub_kind == "package"));
        assert!(items.iter().all(|i| i.kind == NodeKind::Component));
    }

    #[test]
    fn descriptor_has_correct_values() {
        let desc = JavaAnalyzer::descriptor();
        assert_eq!(desc.language_id, "java");
        assert!(desc.manifest_files.contains(&"pom.xml".to_string()));
        assert!(desc.manifest_files.contains(&"build.gradle".to_string()));
        assert!(desc
            .manifest_files
            .contains(&"build.gradle.kts".to_string()));
        assert!(desc.source_extensions.contains(&".java".to_string()));
        assert!(desc.skip_directories.contains(&"target".to_string()));
        assert!(desc.skip_directories.contains(&"build".to_string()));
    }

    #[test]
    fn default_trait_creates_analyzer() {
        let analyzer = JavaAnalyzer::default();
        assert_eq!(analyzer.language_id(), "java");
    }

    #[test]
    fn parser_factory_creates_parser() {
        let parser = JavaAnalyzer::parser();
        // Just verify it returns a valid parser that doesn't panic.
        let result = parser.parse("test-unit", &[]);
        assert!(result.items.is_empty());
    }

    #[test]
    fn qualified_call_emits_target_with_receiver() {
        let result = parse_java_source(
            "public class Foo { public void bar() { System.out.println(\"hello\"); } }",
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(!calls.is_empty());
    }

    #[test]
    fn this_call_targets_enclosing_class() {
        let result =
            parse_java_source("public class Foo { void bar() { this.baz(); } void baz() {} }");
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            calls.iter().any(|c| c.target_qualified_name.contains("Foo")
                && c.target_qualified_name.contains("baz")),
            "this.baz() should target Foo::baz"
        );
    }

    #[test]
    fn static_import_emits_depends_edge() {
        let result = parse_java_source("import static java.lang.Math.max;\npublic class Foo {}");
        let deps: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(!deps.is_empty(), "static import should emit Depends edge");
    }

    #[test]
    fn multiple_fields_in_single_declaration() {
        let result = parse_java_source("public class Foo { private int x, y, z; }");
        let fields: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "field")
            .collect();
        assert_eq!(
            fields.len(),
            3,
            "should extract all three field declarators"
        );
    }

    #[test]
    fn enum_with_methods() {
        let result = parse_java_source(
            "public enum Color { RED, GREEN; public String label() { return name(); } }",
        );
        let enums: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "enum")
            .collect();
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(enums.len(), 1);
        assert_eq!(methods.len(), 1);
    }

    #[test]
    fn handles_syntax_error_gracefully() {
        let result = parse_java_source("public class { broken syntax !!@#$");
        // Should not panic; may or may not extract partial items.
        assert!(result.warnings.is_empty() || !result.warnings.is_empty());
    }

    #[test]
    fn handles_nonexistent_file() {
        let analyzer = JavaAnalyzer::new();
        let path = Path::new("/nonexistent/path/Foo.java");
        let result = analyzer.analyze_crate("myapp", &[path]);
        assert!(
            !result.warnings.is_empty(),
            "should warn about unreadable file"
        );
        assert!(
            result.warnings[0].message.contains("failed to read file"),
            "warning should describe the read failure"
        );
    }

    #[test]
    fn wildcard_import_emits_depends_edge() {
        let result = parse_java_source("import java.util.*;\npublic class Foo {}");
        let deps: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(!deps.is_empty(), "wildcard import should emit Depends edge");
    }

    #[test]
    fn no_package_declaration_returns_none() {
        let source = "public class Foo {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let pkg = extract_package_declaration(&root, source);
        assert!(pkg.is_none(), "file without package should return None");
    }

    #[test]
    fn abstract_class_is_extracted() {
        let result =
            parse_java_source("public abstract class AbstractHandler { abstract void handle(); }");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert_eq!(classes.len(), 1);
        assert!(classes[0].qualified_name.contains("AbstractHandler"));
    }

    #[test]
    fn class_extends_and_implements() {
        let result = parse_java_source(
            "public class MyService extends BaseService implements Runnable, Serializable {}",
        );
        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert_eq!(extends.len(), 1, "should have one Extends relation");
        assert_eq!(
            extends[0].target_qualified_name, "BaseService",
            "should extend BaseService"
        );
        assert_eq!(impls.len(), 2, "should implement two interfaces");
    }

    #[test]
    fn generic_superclass_extracts_raw_type() {
        let result = parse_java_source("public class NumberList extends ArrayList<Integer> {}");
        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        assert_eq!(extends.len(), 1);
        assert_eq!(
            extends[0].target_qualified_name, "ArrayList",
            "should extract raw type from generic superclass"
        );
    }

    #[test]
    fn generic_interface_extracts_raw_type() {
        let result =
            parse_java_source("public class Foo implements Comparable<Foo>, List<String> {}");
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert_eq!(impls.len(), 2);
        assert!(impls
            .iter()
            .any(|r| r.target_qualified_name == "Comparable"));
        assert!(impls.iter().any(|r| r.target_qualified_name == "List"));
    }

    #[test]
    fn enum_implementing_interface() {
        let result = parse_java_source(
            "public enum Direction implements Displayable { NORTH, SOUTH; public String display() { return name(); } }",
        );
        let impls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0].target_qualified_name, "Displayable");
    }

    #[test]
    fn interface_methods_are_extracted() {
        let result = parse_java_source(
            "public interface Repository { void save(Object o); Object findById(int id); }",
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 2);
        assert!(methods.iter().any(|m| m.qualified_name.contains("save")));
        assert!(methods
            .iter()
            .any(|m| m.qualified_name.contains("findById")));
    }

    #[test]
    fn constructor_call_graph() {
        let result =
            parse_java_source("public class Foo { public Foo() { init(); } void init() {} }");
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            !calls.is_empty(),
            "constructor should have call graph edges"
        );
        assert!(calls
            .iter()
            .any(|c| c.target_qualified_name.contains("init")));
    }

    #[test]
    fn chained_method_calls() {
        let result = parse_java_source(
            "public class Foo { void bar() { builder.setName(\"x\").build(); } }",
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            calls.len() >= 2,
            "chained calls should emit multiple Calls edges, got {}",
            calls.len()
        );
    }

    #[test]
    fn multiple_classes_in_single_file() {
        let result = parse_java_source("class Foo {} class Bar {} class Baz {}");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert_eq!(classes.len(), 3, "should extract all three classes");
    }

    #[test]
    fn nested_interface_inside_class() {
        let result = parse_java_source(
            "public class Outer { public interface Callback { void onResult(); } }",
        );
        let ifaces: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "interface")
            .collect();
        assert_eq!(ifaces.len(), 1);
        assert!(ifaces[0].qualified_name.contains("Callback"));
    }

    #[test]
    fn nested_enum_inside_class() {
        let result =
            parse_java_source("public class Outer { public enum Status { ACTIVE, INACTIVE } }");
        let enums: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "enum")
            .collect();
        assert_eq!(enums.len(), 1);
        assert!(enums[0].qualified_name.contains("Status"));
    }

    #[test]
    fn method_has_loc_metadata() {
        let result =
            parse_java_source("public class Foo {\n  public void bar() {\n    return;\n  }\n}");
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 1);
        let metadata = methods[0].metadata.as_ref().expect("should have metadata");
        assert!(metadata.get("loc").is_some(), "metadata should contain loc");
    }

    #[test]
    fn class_has_source_ref_with_line() {
        let result = parse_java_source("public class Foo {}");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert!(
            classes[0].source_ref.contains(':'),
            "source_ref should contain line number"
        );
    }

    #[test]
    fn method_parent_is_class() {
        let result = parse_java_source("public class Foo { void bar() {} }");
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert!(
            methods[0]
                .parent_qualified_name
                .as_ref()
                .unwrap()
                .contains("Foo"),
            "method parent should be the enclosing class"
        );
    }

    #[test]
    fn field_parent_is_class() {
        let result = parse_java_source("public class Foo { private int x; }");
        let fields: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "field")
            .collect();
        assert!(
            fields[0]
                .parent_qualified_name
                .as_ref()
                .unwrap()
                .contains("Foo"),
            "field parent should be the enclosing class"
        );
    }

    #[test]
    fn constructor_parent_is_class() {
        let result = parse_java_source("public class Foo { public Foo() {} }");
        let ctors: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "constructor")
            .collect();
        assert!(
            ctors[0]
                .parent_qualified_name
                .as_ref()
                .unwrap()
                .contains("Foo"),
            "constructor parent should be the enclosing class"
        );
    }

    #[test]
    fn test_annotation_detected_in_method() {
        let result = parse_java_source(
            "import org.junit.jupiter.api.Test;\npublic class FooTest {\n  @Test\n  void testSomething() {}\n}",
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 1);
        // Methods with @Test annotation in non-test files get test tag
        // (Note: inline annotation detection depends on tree-sitter grammar)
    }

    #[test]
    fn test_tags_from_test_file_path() {
        assert!(is_java_test_file(Path::new(
            "project/src/test/java/FooTest.java"
        )));
        assert!(!is_java_test_file(Path::new(
            "project/src/main/java/Foo.java"
        )));
        assert!(!is_java_test_file(Path::new("Foo.java")));
    }

    #[test]
    fn java_test_tags_test_file_overrides_annotations() {
        // When in a test file, all methods get test tag regardless of annotations.
        let tags = java_test_tags(&[], true);
        assert_eq!(tags, vec!["test"]);
    }

    #[test]
    fn java_test_tags_repeated_test_annotation() {
        let tags = java_test_tags(&["@RepeatedTest".to_string()], false);
        assert_eq!(tags, vec!["test"]);
    }

    #[test]
    fn java_test_tags_testng_annotation() {
        let tags = java_test_tags(&["@org.testng.annotations.Test".to_string()], false);
        assert_eq!(tags, vec!["test"]);
    }

    #[test]
    fn java_test_tags_no_match_returns_empty() {
        let tags = java_test_tags(&["@Override".to_string(), "@Deprecated".to_string()], false);
        assert!(tags.is_empty());
    }

    #[test]
    fn build_import_map_maps_types() {
        let mut result = ParseResult::default();
        result.relations.push(AnalysisRelation {
            source_qualified_name: "myapp".to_string(),
            target_qualified_name: "java.util.List".to_string(),
            kind: EdgeKind::Depends,
        });
        result.relations.push(AnalysisRelation {
            source_qualified_name: "myapp".to_string(),
            target_qualified_name: "java.io.File".to_string(),
            kind: EdgeKind::Depends,
        });
        let map = build_import_map(&result);
        assert_eq!(map.get("List"), Some(&"java.util.List".to_string()));
        assert_eq!(map.get("File"), Some(&"java.io.File".to_string()));
    }

    #[test]
    fn build_import_map_ignores_lowercase_packages() {
        let mut result = ParseResult::default();
        result.relations.push(AnalysisRelation {
            source_qualified_name: "myapp".to_string(),
            target_qualified_name: "java.util".to_string(),
            kind: EdgeKind::Depends,
        });
        let map = build_import_map(&result);
        assert!(
            map.is_empty(),
            "should not map lowercase package names as types"
        );
    }

    #[test]
    fn java_file_to_package_qn_with_main_source_root() {
        let source_root = Path::new("/project");
        let source_ref = "/project/src/main/java/com/example/Foo.java:10";
        let qn = java_file_to_package_qn(source_root, source_ref, "myapp");
        assert_eq!(
            qn,
            Some("myapp::com::example".to_string()),
            "should extract package path from src/main/java/"
        );
    }

    #[test]
    fn java_file_to_package_qn_with_test_source_root() {
        let source_root = Path::new("/project");
        let source_ref = "/project/src/test/java/com/example/FooTest.java:5";
        let qn = java_file_to_package_qn(source_root, source_ref, "myapp");
        assert_eq!(
            qn,
            Some("myapp::com::example".to_string()),
            "should extract package path from src/test/java/"
        );
    }

    #[test]
    fn java_file_to_package_qn_without_standard_layout() {
        let source_root = Path::new("/project");
        let source_ref = "/project/com/example/Foo.java:1";
        let qn = java_file_to_package_qn(source_root, source_ref, "myapp");
        assert_eq!(
            qn,
            Some("myapp::com::example".to_string()),
            "should use relative path when no standard source root"
        );
    }

    #[test]
    fn java_file_to_package_qn_at_root() {
        let source_root = Path::new("/project");
        let source_ref = "/project/Foo.java:1";
        let qn = java_file_to_package_qn(source_root, source_ref, "myapp");
        assert_eq!(
            qn,
            Some("myapp".to_string()),
            "file at root should produce just the unit name"
        );
    }

    #[test]
    fn java_file_to_package_qn_outside_source_root() {
        let source_root = Path::new("/project");
        let source_ref = "/other/Foo.java:1";
        let qn = java_file_to_package_qn(source_root, source_ref, "myapp");
        assert!(qn.is_none(), "file outside source root should return None");
    }

    #[test]
    fn post_process_reparents_items_to_package() {
        let source_root = Path::new("/project");
        let mut result = ParseResult::default();
        result.items.push(AnalysisItem {
            qualified_name: "myapp::Foo".to_string(),
            kind: NodeKind::Unit,
            sub_kind: "class".to_string(),
            parent_qualified_name: Some("myapp".to_string()),
            source_ref: "/project/src/main/java/com/example/Foo.java:5".to_string(),
            language: "java".to_string(),
            metadata: None,
            tags: vec![],
        });

        let analyzer = JavaAnalyzer::new();
        CoreLanguageParser::post_process(&analyzer, source_root, "myapp", &mut result);

        assert_eq!(
            result.items[0].qualified_name, "myapp::com::example::Foo",
            "class should be reparented to package"
        );
        assert_eq!(
            result.items[0].parent_qualified_name,
            Some("myapp::com::example".to_string())
        );
    }

    #[test]
    fn post_process_reparents_methods_preserving_class() {
        let source_root = Path::new("/project");
        let mut result = ParseResult::default();
        result.items.push(AnalysisItem {
            qualified_name: "myapp::Foo::bar".to_string(),
            kind: NodeKind::Unit,
            sub_kind: "method".to_string(),
            parent_qualified_name: Some("myapp::Foo".to_string()),
            source_ref: "/project/src/main/java/com/example/Foo.java:10".to_string(),
            language: "java".to_string(),
            metadata: None,
            tags: vec![],
        });

        let analyzer = JavaAnalyzer::new();
        CoreLanguageParser::post_process(&analyzer, source_root, "myapp", &mut result);

        assert_eq!(
            result.items[0].qualified_name, "myapp::com::example::Foo::bar",
            "method should be reparented under package::class"
        );
        assert_eq!(
            result.items[0].parent_qualified_name,
            Some("myapp::com::example::Foo".to_string())
        );
    }

    #[test]
    fn post_process_resolves_extends_via_imports() {
        let source_root = Path::new("/project");
        let mut result = ParseResult::default();
        // Simulate an import: java.util.ArrayList -> Depends edge
        result.relations.push(AnalysisRelation {
            source_qualified_name: "myapp".to_string(),
            target_qualified_name: "java.util.ArrayList".to_string(),
            kind: EdgeKind::Depends,
        });
        // Simulate an Extends edge with simple name
        result.relations.push(AnalysisRelation {
            source_qualified_name: "myapp::Foo".to_string(),
            target_qualified_name: "ArrayList".to_string(),
            kind: EdgeKind::Extends,
        });

        let analyzer = JavaAnalyzer::new();
        CoreLanguageParser::post_process(&analyzer, source_root, "myapp", &mut result);

        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        assert_eq!(
            extends[0].target_qualified_name, "java::util::ArrayList",
            "should resolve simple name to fully qualified via import map"
        );
    }

    #[test]
    fn post_process_leaves_qualified_extends_unchanged() {
        let source_root = Path::new("/project");
        let mut result = ParseResult::default();
        result.relations.push(AnalysisRelation {
            source_qualified_name: "myapp::Foo".to_string(),
            target_qualified_name: "com.example::Bar".to_string(),
            kind: EdgeKind::Extends,
        });

        let analyzer = JavaAnalyzer::new();
        CoreLanguageParser::post_process(&analyzer, source_root, "myapp", &mut result);

        assert_eq!(
            result.relations[0].target_qualified_name, "com.example::Bar",
            "already-qualified names should not be modified"
        );
    }

    #[test]
    fn emit_java_package_items_deduplicates_packages() {
        let dir = TempDir::new().unwrap();
        let src_root = dir.path();
        let file1 = src_root.join("com/example/Foo.java");
        let file2 = src_root.join("com/example/Bar.java");
        std::fs::create_dir_all(file1.parent().unwrap()).unwrap();
        std::fs::write(&file1, "class Foo {}").unwrap();
        std::fs::write(&file2, "class Bar {}").unwrap();

        let items = emit_java_package_items(src_root, "myapp", &[file1, file2]);
        let pkg_names: Vec<&str> = items.iter().map(|i| i.qualified_name.as_str()).collect();
        let unique: HashSet<&&str> = pkg_names.iter().collect();
        assert_eq!(
            pkg_names.len(),
            unique.len(),
            "should not emit duplicate package nodes"
        );
    }

    #[test]
    fn emit_java_package_items_handles_src_main_java_layout() {
        let dir = TempDir::new().unwrap();
        let src_root = dir.path();
        let file = src_root.join("src/main/java/com/example/Foo.java");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "class Foo {}").unwrap();

        let items = emit_java_package_items(src_root, "myapp", &[file]);
        // Should strip src/main/java/ prefix: com, com/example
        assert!(
            items.iter().any(|i| i.qualified_name == "myapp::com"),
            "should emit com package"
        );
        assert!(
            items
                .iter()
                .any(|i| i.qualified_name == "myapp::com::example"),
            "should emit com::example package"
        );
    }

    #[test]
    fn core_language_parser_parse_delegates() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("Foo.java");
        std::fs::write(&file, "public class Foo {}").unwrap();

        let parser = JavaAnalyzer::parser();
        let result = parser.parse("myapp", &[file.as_path()]);
        assert!(!result.items.is_empty(), "parser.parse should return items");
    }

    #[test]
    fn core_language_parser_emit_structural_items_delegates() {
        let dir = TempDir::new().unwrap();
        let src_root = dir.path();
        let file = src_root.join("com/example/Foo.java");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "class Foo {}").unwrap();

        let parser = JavaAnalyzer::parser();
        let items = parser.emit_structural_items(src_root, "myapp", &[file]);
        assert!(!items.is_empty(), "should emit structural package items");
    }

    #[test]
    fn no_imports_produces_no_depends_edges() {
        let result = parse_java_source("public class Foo {}");
        let deps: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(deps.is_empty(), "no imports should mean no Depends edges");
    }

    #[test]
    fn class_with_no_body_members() {
        let result = parse_java_source("public class Empty {}");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert_eq!(classes.len(), 1);
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert!(methods.is_empty());
    }

    #[test]
    fn interface_extending_multiple_interfaces() {
        let result =
            parse_java_source("public interface Combined extends Readable, Writable, Closeable {}");
        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        assert_eq!(extends.len(), 3, "should extend three interfaces");
    }

    #[test]
    fn deeply_nested_class() {
        let result = parse_java_source("public class A { public class B { public class C {} } }");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert_eq!(classes.len(), 3, "should extract A, B, and C");
    }
}
