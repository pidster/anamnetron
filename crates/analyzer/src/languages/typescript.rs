//! TypeScript language analyzer using tree-sitter-typescript.
//!
//! Extracts structural elements (classes, functions, interfaces, type aliases,
//! enums) and import relationships from TypeScript source files. Descends into
//! class/interface/enum bodies to extract members, and emits `Extends` /
//! `Implements` edges for heritage clauses. Also handles Svelte files by
//! extracting their `<script>` blocks first.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use svt_core::analysis::{LanguageDescriptor, LanguageParser as CoreLanguageParser};
use svt_core::model::{EdgeKind, NodeKind};

use crate::languages::svelte;
use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// TypeScript source code analyzer using tree-sitter-typescript.
#[derive(Debug)]
pub struct TypeScriptAnalyzer {
    _private: (),
}

impl TypeScriptAnalyzer {
    /// Create a new `TypeScriptAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for TypeScriptAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeScriptAnalyzer {
    /// Language descriptor for TypeScript/Svelte packages.
    #[must_use]
    pub fn descriptor() -> LanguageDescriptor {
        LanguageDescriptor {
            language_id: "typescript".to_string(),
            manifest_files: vec!["package.json".to_string()],
            source_extensions: vec![".ts".to_string(), ".tsx".to_string(), ".svelte".to_string()],
            skip_directories: vec![
                "node_modules".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".svt".to_string(),
                "target".to_string(),
                ".git".to_string(),
            ],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
        }
    }

    /// Create a boxed language parser for TypeScript.
    #[must_use]
    pub fn parser() -> Box<dyn CoreLanguageParser> {
        Box::new(TypeScriptAnalyzer::new())
    }
}

impl CoreLanguageParser for TypeScriptAnalyzer {
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult {
        self.analyze_crate(unit_name, files)
    }

    fn emit_structural_items(
        &self,
        source_root: &Path,
        unit_name: &str,
        source_files: &[PathBuf],
    ) -> Vec<AnalysisItem> {
        emit_ts_module_items(source_root, unit_name, source_files)
    }

    fn post_process(&self, source_root: &Path, unit_name: &str, result: &mut ParseResult) {
        // Reparent items to their file-level module qualified names.
        for item in &mut result.items {
            if let Some(file_module_qn) =
                file_to_module_qn(source_root, &item.source_ref, unit_name)
            {
                let item_name = item
                    .qualified_name
                    .rsplit("::")
                    .next()
                    .unwrap_or("")
                    .to_string();
                item.qualified_name = format!("{file_module_qn}::{item_name}");
                item.parent_qualified_name = Some(file_module_qn);
            }
        }

        // Resolve relative import paths to qualified names.
        result.relations.retain_mut(|rel| {
            if rel.target_qualified_name.starts_with("./")
                || rel.target_qualified_name.starts_with("../")
            {
                if let Some(resolved) = resolve_ts_import(&rel.target_qualified_name, unit_name) {
                    rel.target_qualified_name = resolved;
                    true
                } else {
                    false
                }
            } else {
                true
            }
        });
    }
}

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn language_id(&self) -> &str {
        "typescript"
    }

    fn analyze_crate(&self, package_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-typescript grammar".to_string(),
            });
            return result;
        }

        for file in files {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_test = is_typescript_test_file(file);

            match ext {
                "svelte" => match std::fs::read_to_string(file) {
                    Ok(source) => {
                        let blocks = svelte::extract_script_blocks(&source);
                        for block in &blocks {
                            parse_typescript_source(
                                &mut parser,
                                package_name,
                                file,
                                &block.content,
                                block.line_offset,
                                is_test,
                                &mut result,
                            );
                        }
                    }
                    Err(err) => {
                        result.warnings.push(AnalysisWarning {
                            source_ref: file.display().to_string(),
                            message: format!("failed to read file: {err}"),
                        });
                    }
                },
                _ => {
                    // .ts or .tsx files
                    match std::fs::read_to_string(file) {
                        Ok(source) => {
                            parse_typescript_source(
                                &mut parser,
                                package_name,
                                file,
                                &source,
                                0,
                                is_test,
                                &mut result,
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
            }
        }

        result
    }
}

/// Check whether a file path refers to a TypeScript test file.
fn is_typescript_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    // Check *.test.ts, *.spec.ts, *.test.tsx, *.spec.tsx patterns
    if name.contains(".test.") || name.contains(".spec.") {
        return true;
    }
    // Check if file is in a __tests__ directory
    path.components()
        .any(|c| matches!(c, std::path::Component::Normal(s) if s.to_str() == Some("__tests__")))
}

/// Parse TypeScript source and extract items and import relations.
///
/// Processes both exported and non-exported top-level declarations, descending
/// into class/interface/enum bodies to extract members.
fn parse_typescript_source(
    parser: &mut tree_sitter::Parser,
    package_name: &str,
    file_path: &Path,
    source: &str,
    line_offset: usize,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let Some(tree) = parser.parse(source, None) else {
        result.warnings.push(AnalysisWarning {
            source_ref: file_path.display().to_string(),
            message: "tree-sitter failed to parse TypeScript".to_string(),
        });
        return;
    };

    let source_bytes = source.as_bytes();
    let root = tree.root_node();

    // The module context for this file is just the package name
    // (directory-based module context is handled by the orchestrator)
    let module_context = package_name.to_string();

    for i in 0..root.named_child_count() {
        let Some(child) = root.named_child(i) else {
            continue;
        };

        match child.kind() {
            "export_statement" => {
                extract_export(
                    child,
                    source_bytes,
                    file_path,
                    &module_context,
                    line_offset,
                    is_test_file,
                    result,
                );
            }
            "import_statement" => {
                extract_import(child, source_bytes, &module_context, &mut result.relations);
            }
            // Non-exported top-level declarations.
            "function_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration" => {
                extract_declaration(
                    child,
                    source_bytes,
                    file_path,
                    &module_context,
                    line_offset,
                    false,
                    is_test_file,
                    result,
                );
            }
            _ => {}
        }
    }
}

/// Extract an exported declaration from an `export_statement` node.
fn extract_export(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &str,
    line_offset: usize,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    // export_statement can contain: function_declaration, class_declaration,
    // interface_declaration, type_alias_declaration, enum_declaration, or
    // lexical_declaration.
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        match child.kind() {
            "function_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration" => {
                extract_declaration(
                    child,
                    source,
                    file_path,
                    module_context,
                    line_offset,
                    true,
                    is_test_file,
                    result,
                );
            }
            _ => {}
        }
    }
}

/// Extract a single declaration node, emitting the item plus any members/edges.
#[allow(clippy::too_many_arguments)]
fn extract_declaration(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &str,
    line_offset: usize,
    exported: bool,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let (kind, sub_kind) = match node.kind() {
        "function_declaration" => (NodeKind::Unit, "function"),
        "class_declaration" => (NodeKind::Unit, "class"),
        "interface_declaration" => (NodeKind::Unit, "interface"),
        "type_alias_declaration" => (NodeKind::Unit, "type-alias"),
        "enum_declaration" => (NodeKind::Unit, "enum"),
        _ => return,
    };

    let Some(name) = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
    else {
        return;
    };

    let line = node.start_position().row + 1 + line_offset;
    let source_ref = format!("{}:{line}", file_path.display());
    let loc = node.end_position().row - node.start_position().row + 1;
    let qualified_name = format!("{module_context}::{name}");

    let mut tags = Vec::new();
    if exported {
        tags.push("exported".to_string());
    }
    if is_test_file {
        tags.push("test".to_string());
    }

    result.items.push(AnalysisItem {
        qualified_name: qualified_name.clone(),
        kind,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: Some(module_context.to_string()),
        source_ref,
        language: "typescript".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Descend into body for classes, interfaces, and enums.
    match node.kind() {
        "class_declaration" => {
            extract_class_members(
                node,
                source,
                file_path,
                module_context,
                &qualified_name,
                line_offset,
                result,
            );
            extract_heritage_clauses(node, source, &qualified_name, result);
        }
        "interface_declaration" => {
            extract_interface_members(
                node,
                source,
                file_path,
                &qualified_name,
                line_offset,
                result,
            );
        }
        "enum_declaration" => {
            extract_enum_members(
                node,
                source,
                file_path,
                &qualified_name,
                line_offset,
                result,
            );
        }
        "function_declaration" => {
            // Walk the function body for call expressions.
            if let Some(body) = node.child_by_field_name("body") {
                visit_ts_call_expressions(
                    body,
                    source,
                    &qualified_name,
                    module_context,
                    None,
                    result,
                );
            }
        }
        _ => {}
    }
}

/// Extract class members (methods, properties, constructor) from a class body.
fn extract_class_members(
    class_node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &str,
    class_qn: &str,
    line_offset: usize,
    result: &mut ParseResult,
) {
    let Some(body) = class_node.child_by_field_name("body") else {
        return;
    };

    for i in 0..body.named_child_count() {
        let Some(child) = body.named_child(i) else {
            continue;
        };

        let (member_name, sub_kind) = match child.kind() {
            "method_definition" => {
                // Constructor is a method_definition with name "constructor".
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok());
                match name {
                    Some("constructor") => ("constructor".to_string(), "constructor"),
                    Some(n) => (n.to_string(), "method"),
                    None => continue,
                }
            }
            "public_field_definition" | "property_definition" => {
                let name = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok());
                match name {
                    Some(n) => (n.to_string(), "property"),
                    None => continue,
                }
            }
            _ => continue,
        };

        let member_qn = format!("{class_qn}::{member_name}");
        let line = child.start_position().row + 1 + line_offset;
        let loc = child.end_position().row - child.start_position().row + 1;

        result.items.push(AnalysisItem {
            qualified_name: member_qn.clone(),
            kind: NodeKind::Unit,
            sub_kind: sub_kind.to_string(),
            parent_qualified_name: Some(class_qn.to_string()),
            source_ref: format!("{}:{line}", file_path.display()),
            language: "typescript".to_string(),
            metadata: Some(serde_json::json!({"loc": loc})),
            tags: vec![],
        });

        // Walk method/constructor bodies for call expressions.
        if sub_kind == "method" || sub_kind == "constructor" {
            if let Some(method_body) = child.child_by_field_name("body") {
                visit_ts_call_expressions(
                    method_body,
                    source,
                    &member_qn,
                    module_context,
                    Some(class_qn),
                    result,
                );
            }
        }
    }
}

/// Extract heritage clauses (`extends` / `implements`) from a class declaration
/// and emit `Extends` / `Implements` edges.
fn extract_heritage_clauses(
    class_node: tree_sitter::Node<'_>,
    source: &[u8],
    class_qn: &str,
    result: &mut ParseResult,
) {
    // In tree-sitter-typescript, heritage clauses appear as child nodes of the
    // class_declaration. We iterate over all children looking for
    // `extends_clause`, `implements_clause`, or `class_heritage` wrapper.
    for i in 0..class_node.child_count() {
        let Some(child) = class_node.child(i) else {
            continue;
        };

        match child.kind() {
            "extends_clause" | "class_heritage" | "implements_clause" => {
                visit_heritage_node(child, source, class_qn, result);
            }
            _ => {}
        }
    }
}

/// Recursively visit a heritage clause node to find type identifiers and emit edges.
fn visit_heritage_node(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    class_qn: &str,
    result: &mut ParseResult,
) {
    match node.kind() {
        "extends_clause" => {
            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    if let Some(type_name) = extract_type_name(child, source) {
                        result.relations.push(AnalysisRelation {
                            source_qualified_name: class_qn.to_string(),
                            target_qualified_name: type_name,
                            kind: EdgeKind::Extends,
                        });
                    }
                }
            }
        }
        "implements_clause" => {
            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    if let Some(type_name) = extract_type_name(child, source) {
                        result.relations.push(AnalysisRelation {
                            source_qualified_name: class_qn.to_string(),
                            target_qualified_name: type_name,
                            kind: EdgeKind::Implements,
                        });
                    }
                }
            }
        }
        "class_heritage" => {
            // class_heritage wraps extends_clause and implements_clause.
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    visit_heritage_node(child, source, class_qn, result);
                }
            }
        }
        _ => {}
    }
}

/// Extract a type name from a type identifier or generic type node.
fn extract_type_name(node: tree_sitter::Node<'_>, source: &[u8]) -> Option<String> {
    match node.kind() {
        "type_identifier" | "identifier" => node.utf8_text(source).ok().map(|s| s.to_string()),
        "generic_type" => {
            // generic_type has a "name" child that is a type_identifier.
            node.child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .map(|s| s.to_string())
        }
        _ => {
            // Fallback: try to get text from the first named child.
            node.named_child(0)
                .and_then(|n| n.utf8_text(source).ok())
                .map(|s| s.to_string())
        }
    }
}

/// Extract interface members (method signatures, property signatures).
fn extract_interface_members(
    iface_node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    iface_qn: &str,
    line_offset: usize,
    result: &mut ParseResult,
) {
    let Some(body) = iface_node.child_by_field_name("body") else {
        return;
    };

    for i in 0..body.named_child_count() {
        let Some(child) = body.named_child(i) else {
            continue;
        };

        let sub_kind = match child.kind() {
            "method_signature" => "method",
            "property_signature" => "property",
            _ => continue,
        };

        let Some(name) = child
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
        else {
            continue;
        };

        let line = child.start_position().row + 1 + line_offset;
        let loc = child.end_position().row - child.start_position().row + 1;

        result.items.push(AnalysisItem {
            qualified_name: format!("{iface_qn}::{name}"),
            kind: NodeKind::Unit,
            sub_kind: sub_kind.to_string(),
            parent_qualified_name: Some(iface_qn.to_string()),
            source_ref: format!("{}:{line}", file_path.display()),
            language: "typescript".to_string(),
            metadata: Some(serde_json::json!({"loc": loc})),
            tags: vec![],
        });
    }
}

/// Extract enum members as variant nodes.
///
/// In tree-sitter-typescript, the enum AST structure is:
/// - `enum_declaration` → `enum_body` (named child, not a field)
/// - `enum_body` contains `property_identifier` (simple member) or
///   `enum_assignment` (member with value, wrapping a `property_identifier`)
fn extract_enum_members(
    enum_node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    enum_qn: &str,
    line_offset: usize,
    result: &mut ParseResult,
) {
    // Find the enum_body child (not accessible via field name).
    let mut body = None;
    for i in 0..enum_node.named_child_count() {
        if let Some(child) = enum_node.named_child(i) {
            if child.kind() == "enum_body" {
                body = Some(child);
                break;
            }
        }
    }
    let Some(body) = body else {
        return;
    };

    for i in 0..body.named_child_count() {
        let Some(child) = body.named_child(i) else {
            continue;
        };

        // Simple member: `property_identifier` (e.g., `Red`)
        // Member with value: `enum_assignment` (e.g., `Green = "green"`)
        //   which wraps a `property_identifier` as its first named child.
        let (name_node, member_node) = match child.kind() {
            "property_identifier" => (child, child),
            "enum_assignment" => {
                if let Some(name_child) = child.named_child(0) {
                    (name_child, child)
                } else {
                    continue;
                }
            }
            _ => continue,
        };

        let Some(name) = name_node.utf8_text(source).ok() else {
            continue;
        };

        let line = member_node.start_position().row + 1 + line_offset;
        let loc = member_node.end_position().row - member_node.start_position().row + 1;

        result.items.push(AnalysisItem {
            qualified_name: format!("{enum_qn}::{name}"),
            kind: NodeKind::Unit,
            sub_kind: "variant".to_string(),
            parent_qualified_name: Some(enum_qn.to_string()),
            source_ref: format!("{}:{line}", file_path.display()),
            language: "typescript".to_string(),
            metadata: Some(serde_json::json!({"loc": loc})),
            tags: vec![],
        });
    }
}

/// Recursively walk AST nodes to find call expressions and emit `Calls` edges.
///
/// For each `call_expression` found, extracts the callee and emits an
/// `AnalysisRelation` with `EdgeKind::Calls` from the containing function
/// to the target function/method.
fn visit_ts_call_expressions(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    caller_qn: &str,
    module_context: &str,
    class_context: Option<&str>,
    result: &mut ParseResult,
) {
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        if child.kind() == "call_expression" {
            if let Some(function) = child.child_by_field_name("function") {
                match function.kind() {
                    "identifier" => {
                        // Simple function call: foo()
                        if let Ok(name) = function.utf8_text(source) {
                            if !name.is_empty() {
                                result.relations.push(AnalysisRelation {
                                    source_qualified_name: caller_qn.to_string(),
                                    target_qualified_name: format!("{module_context}::{name}"),
                                    kind: EdgeKind::Calls,
                                });
                            }
                        }
                    }
                    "member_expression" => {
                        // Method call: obj.method() or this.method()
                        let object = function.child_by_field_name("object");
                        let property = function.child_by_field_name("property");
                        if let (Some(obj), Some(prop)) = (object, property) {
                            if let (Ok(obj_text), Ok(prop_text)) =
                                (obj.utf8_text(source), prop.utf8_text(source))
                            {
                                if obj_text == "this" {
                                    // this.method() — resolve to class method if in class context
                                    if let Some(cls_qn) = class_context {
                                        result.relations.push(AnalysisRelation {
                                            source_qualified_name: caller_qn.to_string(),
                                            target_qualified_name: format!("{cls_qn}::{prop_text}"),
                                            kind: EdgeKind::Calls,
                                        });
                                    }
                                } else {
                                    // obj.method() — best effort with receiver text
                                    result.relations.push(AnalysisRelation {
                                        source_qualified_name: caller_qn.to_string(),
                                        target_qualified_name: format!("{obj_text}::{prop_text}"),
                                        kind: EdgeKind::Calls,
                                    });
                                }
                            }
                        }
                    }
                    _ => {
                        // Other call forms (IIFE, computed, etc.) — skip.
                    }
                }
            }
        }

        // Recurse into children to find nested call expressions.
        visit_ts_call_expressions(
            child,
            source,
            caller_qn,
            module_context,
            class_context,
            result,
        );
    }
}

/// Extract an import statement and emit a Depends relation.
fn extract_import(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    module_context: &str,
    relations: &mut Vec<AnalysisRelation>,
) {
    // import_statement has a "source" field which is a string node.
    let Some(source_node) = node.child_by_field_name("source") else {
        return;
    };

    let Ok(import_path) = source_node.utf8_text(source) else {
        return;
    };

    // Strip quotes from the import path
    let import_path = import_path.trim_matches(|c| c == '\'' || c == '"');

    if import_path.is_empty() {
        return;
    }

    // Only track relative imports (starting with ./ or ../)
    // External package imports are not resolvable within our graph
    if import_path.starts_with("./") || import_path.starts_with("../") {
        // Convert the relative path to a qualified-name-like form.
        // ./lib/api -> module context determined by orchestrator
        // For now, emit raw import path as target -- the orchestrator
        // will resolve it when it builds the full qualified name map.
        relations.push(AnalysisRelation {
            source_qualified_name: module_context.to_string(),
            target_qualified_name: import_path.to_string(),
            kind: EdgeKind::Depends,
        });
    }
}

/// Emit module items for directories and files in a TypeScript package.
pub(crate) fn emit_ts_module_items(
    source_root: &Path,
    package_name: &str,
    source_files: &[PathBuf],
) -> Vec<AnalysisItem> {
    let mut items = Vec::new();
    let mut emitted_modules: HashSet<String> = HashSet::new();

    for file in source_files {
        let rel = match file.strip_prefix(source_root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Emit directory modules.
        let mut current_qn = package_name.to_string();
        for component in rel.parent().iter().flat_map(|p| p.components()) {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str().unwrap_or("");
                let parent_qn = current_qn.clone();
                current_qn = format!("{current_qn}::{name_str}");
                if emitted_modules.insert(current_qn.clone()) {
                    items.push(AnalysisItem {
                        qualified_name: current_qn.clone(),
                        kind: NodeKind::Component,
                        sub_kind: "module".to_string(),
                        parent_qualified_name: Some(parent_qn),
                        source_ref: file.parent().unwrap_or(source_root).display().to_string(),
                        language: "typescript".to_string(),
                        metadata: None,
                        tags: vec![],
                    });
                }
            }
        }

        // Emit file-level item.
        let file_stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Skip index/main files — they represent their parent directory.
        if file_stem == "index" || file_stem == "main" {
            continue;
        }

        let file_qn = format!("{current_qn}::{file_stem}");
        let (kind, sub_kind, lang) = if ext == "svelte" {
            (NodeKind::Unit, "component", "svelte")
        } else {
            (NodeKind::Component, "module", "typescript")
        };

        if emitted_modules.insert(file_qn.clone()) {
            items.push(AnalysisItem {
                qualified_name: file_qn,
                kind,
                sub_kind: sub_kind.to_string(),
                parent_qualified_name: Some(current_qn),
                source_ref: file.display().to_string(),
                language: lang.to_string(),
                metadata: None,
                tags: vec![],
            });
        }
    }

    items
}

/// Map a file path (from source_ref) to its module qualified name.
fn file_to_module_qn(source_root: &Path, source_ref: &str, package_name: &str) -> Option<String> {
    let file_path_str = source_ref
        .rsplit_once(':')
        .map(|(p, _)| p)
        .unwrap_or(source_ref);
    let file_path = Path::new(file_path_str);
    let rel = file_path.strip_prefix(source_root).ok()?;

    let stem = rel.file_stem().and_then(|s| s.to_str())?;
    let mut qn = package_name.to_string();

    for component in rel.parent().iter().flat_map(|p| p.components()) {
        if let std::path::Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                qn = format!("{qn}::{name_str}");
            }
        }
    }

    if stem != "index" && stem != "main" {
        qn = format!("{qn}::{stem}");
    }

    Some(qn)
}

/// Resolve a relative TypeScript import path to a qualified name.
fn resolve_ts_import(import_path: &str, package_name: &str) -> Option<String> {
    let clean = import_path
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx")
        .trim_end_matches(".svelte")
        .trim_end_matches(".js");

    if clean.is_empty() {
        return None;
    }

    Some(format!("{package_name}::{}", clean.replace('/', "::")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_ts_source(package_name: &str, source: &str) -> ParseResult {
        let mut file = NamedTempFile::with_suffix(".ts").unwrap();
        write!(file, "{}", source).unwrap();
        let analyzer = TypeScriptAnalyzer::new();
        analyzer.analyze_crate(package_name, &[file.path()])
    }

    // --- Export extraction tests ---

    #[test]
    fn extracts_exported_function() {
        let result = parse_ts_source("my-app", "export function fetchData(): void {}");
        let fns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            fns.iter().any(|f| f.qualified_name == "my-app::fetchData"),
            "should extract exported function, got: {:?}",
            fns
        );
    }

    #[test]
    fn extracts_exported_class() {
        let result = parse_ts_source("my-app", "export class UserService {}");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert!(
            classes
                .iter()
                .any(|c| c.qualified_name == "my-app::UserService"),
            "should extract exported class, got: {:?}",
            classes
        );
    }

    #[test]
    fn extracts_exported_interface() {
        let result = parse_ts_source("my-app", "export interface ApiNode { id: string; }");
        let interfaces: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "interface")
            .collect();
        assert!(
            interfaces
                .iter()
                .any(|i| i.qualified_name == "my-app::ApiNode"),
            "should extract exported interface, got: {:?}",
            interfaces
        );
    }

    #[test]
    fn extracts_exported_type_alias() {
        let result = parse_ts_source("my-app", "export type NodeKind = 'system' | 'service';");
        let types: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "type-alias")
            .collect();
        assert!(
            types.iter().any(|t| t.qualified_name == "my-app::NodeKind"),
            "should extract exported type alias, got: {:?}",
            types
        );
    }

    #[test]
    fn extracts_export_default_function() {
        let result = parse_ts_source("my-app", "export default function main() {}");
        let fns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            fns.iter().any(|f| f.qualified_name == "my-app::main"),
            "should extract default exported function, got: {:?}",
            fns
        );
    }

    #[test]
    fn extracts_non_exported_declarations() {
        let result = parse_ts_source(
            "my-app",
            r#"
function privateHelper() {}
class InternalClass {}
export function publicFn() {}
"#,
        );
        let names: Vec<_> = result
            .items
            .iter()
            .map(|i| i.qualified_name.as_str())
            .collect();
        assert!(
            names.contains(&"my-app::privateHelper"),
            "should extract non-exported function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"my-app::InternalClass"),
            "should extract non-exported class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"my-app::publicFn"),
            "should extract exported function, got: {:?}",
            names
        );
    }

    #[test]
    fn exported_items_have_exported_tag() {
        let result = parse_ts_source(
            "my-app",
            r#"
function internal() {}
export function external() {}
"#,
        );
        let internal = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my-app::internal")
            .expect("internal function should be extracted");
        assert!(
            !internal.tags.contains(&"exported".to_string()),
            "non-exported item should not have 'exported' tag"
        );
        let external = result
            .items
            .iter()
            .find(|i| i.qualified_name == "my-app::external")
            .expect("external function should be extracted");
        assert!(
            external.tags.contains(&"exported".to_string()),
            "exported item should have 'exported' tag"
        );
    }

    #[test]
    fn empty_file_produces_no_items() {
        let result = parse_ts_source("my-app", "");
        assert!(result.items.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn language_is_typescript() {
        let result = parse_ts_source("my-app", "export function hello() {}");
        assert_eq!(result.items[0].language, "typescript");
    }

    #[test]
    fn source_ref_contains_line_number() {
        let result = parse_ts_source("my-app", "export function hello() {}");
        assert!(
            result.items[0].source_ref.contains(':'),
            "source_ref should contain file:line, got: {}",
            result.items[0].source_ref
        );
    }

    #[test]
    fn parent_qualified_name_is_package() {
        let result = parse_ts_source("my-app", "export function hello() {}");
        assert_eq!(
            result.items[0].parent_qualified_name,
            Some("my-app".to_string()),
        );
    }

    // --- Import edge and Svelte component tests ---

    #[test]
    fn extracts_relative_import_as_depends_edge() {
        let result = parse_ts_source("my-app", "import { fetchData } from './lib/api';");
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            !depends.is_empty(),
            "relative import should generate Depends relation"
        );
        assert_eq!(depends[0].source_qualified_name, "my-app");
        assert_eq!(depends[0].target_qualified_name, "./lib/api");
    }

    #[test]
    fn ignores_external_package_imports() {
        let result = parse_ts_source("my-app", "import { writable } from 'svelte/store';");
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            depends.is_empty(),
            "external package imports should not generate edges"
        );
    }

    #[test]
    fn extracts_from_svelte_script_block() {
        let mut file = NamedTempFile::with_suffix(".svelte").unwrap();
        write!(
            file,
            r#"<script lang="ts">
export function handleClick(): void {{}}
</script>

<button on:click={{handleClick}}>Click me</button>"#,
        )
        .unwrap();
        let analyzer = TypeScriptAnalyzer::new();
        let result = analyzer.analyze_crate("my-app", &[file.path()]);
        let fns: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            fns.iter()
                .any(|f| f.qualified_name == "my-app::handleClick"),
            "should extract function from Svelte script block, got: {:?}",
            fns
        );
    }

    #[test]
    fn svelte_source_ref_includes_line_offset() {
        let mut file = NamedTempFile::with_suffix(".svelte").unwrap();
        write!(
            file,
            "<p>hello</p>\n<script lang=\"ts\">\nexport function test(): void {{}}\n</script>",
        )
        .unwrap();
        let analyzer = TypeScriptAnalyzer::new();
        let result = analyzer.analyze_crate("my-app", &[file.path()]);
        // The function is on line 3 of the .svelte file (1-indexed)
        // line_offset=1 (script on line 1 zero-indexed) + row 1 within script + 1 = line 3
        let item = &result.items[0];
        let line_num: usize = item.source_ref.rsplit(':').next().unwrap().parse().unwrap();
        assert!(
            line_num >= 3,
            "source_ref line should account for offset, got: {}",
            item.source_ref
        );
    }

    #[test]
    fn multiple_imports_generate_multiple_edges() {
        let result = parse_ts_source(
            "my-app",
            r#"
import { type ApiNode } from './lib/types';
import { fetchNodes } from './lib/api';
"#,
        );
        let depends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert_eq!(depends.len(), 2, "should generate 2 Depends relations");
    }

    // --- Class member extraction tests ---

    #[test]
    fn extracts_class_methods() {
        let result = parse_ts_source(
            "my-app",
            r#"
export class UserService {
    greet(): void {}
    static create(): UserService { return new UserService(); }
}
"#,
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert!(
            methods
                .iter()
                .any(|m| m.qualified_name == "my-app::UserService::greet"),
            "should extract greet method, got: {:?}",
            methods
        );
        assert!(
            methods
                .iter()
                .any(|m| m.qualified_name == "my-app::UserService::create"),
            "should extract static create method, got: {:?}",
            methods
        );
        // Methods should be parented under the class.
        for m in &methods {
            assert_eq!(
                m.parent_qualified_name,
                Some("my-app::UserService".to_string()),
                "method parent should be the class"
            );
        }
    }

    #[test]
    fn extracts_class_properties() {
        let result = parse_ts_source(
            "my-app",
            r#"
export class Config {
    name: string;
    count = 0;
}
"#,
        );
        let props: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "property")
            .collect();
        assert!(
            props
                .iter()
                .any(|p| p.qualified_name == "my-app::Config::name"),
            "should extract name property, got: {:?}",
            props
        );
        assert!(
            props
                .iter()
                .any(|p| p.qualified_name == "my-app::Config::count"),
            "should extract count property, got: {:?}",
            props
        );
    }

    #[test]
    fn extracts_class_constructor() {
        let result = parse_ts_source(
            "my-app",
            r#"
export class Node {
    constructor(public id: string) {}
}
"#,
        );
        let ctors: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "constructor")
            .collect();
        assert!(
            ctors
                .iter()
                .any(|c| c.qualified_name == "my-app::Node::constructor"),
            "should extract constructor, got: {:?}",
            ctors
        );
        assert_eq!(
            ctors[0].parent_qualified_name,
            Some("my-app::Node".to_string()),
        );
    }

    // --- Heritage clause tests ---

    #[test]
    fn emits_extends_edge() {
        let result = parse_ts_source(
            "my-app",
            r#"
class Base {}
export class Derived extends Base {}
"#,
        );
        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        assert!(
            !extends.is_empty(),
            "should emit Extends edge for class inheritance"
        );
        assert_eq!(extends[0].source_qualified_name, "my-app::Derived");
        assert_eq!(extends[0].target_qualified_name, "Base");
    }

    #[test]
    fn emits_implements_edge() {
        let result = parse_ts_source(
            "my-app",
            r#"
interface Serializable { serialize(): string; }
export class Config implements Serializable {
    serialize(): string { return ""; }
}
"#,
        );
        let implements: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert!(
            !implements.is_empty(),
            "should emit Implements edge for class-interface relationship"
        );
        assert_eq!(implements[0].source_qualified_name, "my-app::Config");
        assert_eq!(implements[0].target_qualified_name, "Serializable");
    }

    #[test]
    fn emits_extends_and_implements_together() {
        let result = parse_ts_source(
            "my-app",
            r#"
class Base {}
interface Loggable { log(): void; }
interface Serializable { serialize(): string; }
export class App extends Base implements Loggable, Serializable {
    log(): void {}
    serialize(): string { return ""; }
}
"#,
        );
        let extends: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Extends)
            .collect();
        let implements: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Implements)
            .collect();
        assert_eq!(
            extends.len(),
            1,
            "should emit 1 Extends edge, got: {:?}",
            extends
        );
        assert_eq!(
            implements.len(),
            2,
            "should emit 2 Implements edges, got: {:?}",
            implements
        );
    }

    // --- Interface member extraction tests ---

    #[test]
    fn extracts_interface_methods_and_properties() {
        let result = parse_ts_source(
            "my-app",
            r#"
export interface INode {
    id: string;
    getName(): string;
}
"#,
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method" && i.qualified_name.starts_with("my-app::INode"))
            .collect();
        let props: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "property" && i.qualified_name.starts_with("my-app::INode"))
            .collect();
        assert!(
            methods
                .iter()
                .any(|m| m.qualified_name == "my-app::INode::getName"),
            "should extract interface method, got: {:?}",
            methods
        );
        assert!(
            props
                .iter()
                .any(|p| p.qualified_name == "my-app::INode::id"),
            "should extract interface property, got: {:?}",
            props
        );
        // Members should be parented under the interface.
        for m in methods.iter().chain(props.iter()) {
            assert_eq!(m.parent_qualified_name, Some("my-app::INode".to_string()),);
        }
    }

    // --- Enum member extraction tests ---

    #[test]
    fn extracts_enum_members() {
        let result = parse_ts_source(
            "my-app",
            r#"
export enum Color {
    Red,
    Green = "green",
    Blue,
}
"#,
        );
        let variants: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "variant")
            .collect();
        assert_eq!(
            variants.len(),
            3,
            "should extract 3 enum variants, got: {:?}",
            variants
        );
        assert!(variants
            .iter()
            .any(|v| v.qualified_name == "my-app::Color::Red"));
        assert!(variants
            .iter()
            .any(|v| v.qualified_name == "my-app::Color::Green"));
        assert!(variants
            .iter()
            .any(|v| v.qualified_name == "my-app::Color::Blue"));
        // All variants should be parented under the enum.
        for v in &variants {
            assert_eq!(v.parent_qualified_name, Some("my-app::Color".to_string()),);
        }
    }

    #[test]
    fn extracts_exported_enum() {
        let result = parse_ts_source("my-app", "export enum Direction { Up, Down, Left, Right }");
        let enums: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "enum")
            .collect();
        assert!(
            enums
                .iter()
                .any(|e| e.qualified_name == "my-app::Direction"),
            "should extract exported enum, got: {:?}",
            enums
        );
        let variants: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "variant")
            .collect();
        assert_eq!(variants.len(), 4, "should extract 4 enum variants");
    }

    // --- Non-exported item extraction tests ---

    #[test]
    fn extracts_non_exported_class_with_members() {
        let result = parse_ts_source(
            "my-app",
            r#"
class InternalHelper {
    value: number;
    compute(): number { return this.value * 2; }
}
"#,
        );
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my-app::InternalHelper" && i.sub_kind == "class"),
            "should extract non-exported class"
        );
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my-app::InternalHelper::value"),
            "should extract property of non-exported class"
        );
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my-app::InternalHelper::compute"),
            "should extract method of non-exported class"
        );
    }

    #[test]
    fn extracts_non_exported_enum() {
        let result = parse_ts_source(
            "my-app",
            r#"
enum Status {
    Active,
    Inactive,
}
"#,
        );
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my-app::Status" && i.sub_kind == "enum"),
            "should extract non-exported enum"
        );
        let variants: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "variant")
            .collect();
        assert_eq!(
            variants.len(),
            2,
            "should extract 2 variants from non-exported enum"
        );
    }

    #[test]
    fn extracts_non_exported_interface() {
        let result = parse_ts_source(
            "my-app",
            r#"
interface InternalConfig {
    debug: boolean;
    getLevel(): number;
}
"#,
        );
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my-app::InternalConfig" && i.sub_kind == "interface"),
            "should extract non-exported interface"
        );
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my-app::InternalConfig::debug"),
            "should extract property of non-exported interface"
        );
        assert!(
            result
                .items
                .iter()
                .any(|i| i.qualified_name == "my-app::InternalConfig::getLevel"),
            "should extract method of non-exported interface"
        );
    }

    // --- Test file detection tests ---

    #[test]
    fn test_file_detection() {
        assert!(is_typescript_test_file(Path::new("src/utils.test.ts")));
        assert!(is_typescript_test_file(Path::new("src/utils.spec.ts")));
        assert!(is_typescript_test_file(Path::new("src/__tests__/utils.ts")));
        assert!(!is_typescript_test_file(Path::new("src/utils.ts")));
        assert!(!is_typescript_test_file(Path::new("src/testing.ts")));
    }

    // --- M28: Call graph analysis tests ---

    #[test]
    fn extracts_function_call_edge() {
        let result = parse_ts_source(
            "my-app",
            r#"
function helper(): void {}
function main(): void {
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
            "should extract Calls edge for function call"
        );
        assert!(
            calls
                .iter()
                .any(|c| c.source_qualified_name == "my-app::main"
                    && c.target_qualified_name == "my-app::helper"),
            "should have main -> helper Calls edge, got: {:?}",
            calls
        );
    }

    #[test]
    fn extracts_method_call_edge() {
        let result = parse_ts_source(
            "my-app",
            r#"
class Service {
    start(): void {}
    run(): void {
        this.start();
    }
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
                .any(|c| c.source_qualified_name == "my-app::Service::run"
                    && c.target_qualified_name == "my-app::Service::start"),
            "should have Service::run -> Service::start Calls edge, got: {:?}",
            calls
        );
    }

    #[test]
    fn does_not_emit_calls_for_imports() {
        let result = parse_ts_source("my-app", "import { fetchData } from './lib/api';");
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            calls.is_empty(),
            "import statements should not generate Calls edges, got: {:?}",
            calls
        );
    }

    #[test]
    fn extracts_nested_call_in_control_flow() {
        let result = parse_ts_source(
            "my-app",
            r#"
function doWork(): void {}
function main(): void {
    if (true) {
        doWork();
    }
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
                .any(|c| c.source_qualified_name == "my-app::main"
                    && c.target_qualified_name == "my-app::doWork"),
            "should find calls inside if blocks, got: {:?}",
            calls
        );
    }
}
