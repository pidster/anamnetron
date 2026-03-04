//! Python language analyzer using tree-sitter-python.
//!
//! Extracts structural elements (classes, functions, methods) and import
//! relationships from Python source files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use svt_core::analysis::{LanguageDescriptor, LanguageParser as CoreLanguageParser};
use svt_core::model::{EdgeKind, NodeKind};

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// Python source code analyzer using tree-sitter-python.
#[derive(Debug)]
pub struct PythonAnalyzer {
    _private: (),
}

impl PythonAnalyzer {
    /// Create a new `PythonAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for PythonAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonAnalyzer {
    /// Language descriptor for Python packages.
    #[must_use]
    pub fn descriptor() -> LanguageDescriptor {
        LanguageDescriptor {
            language_id: "python".to_string(),
            manifest_files: vec!["pyproject.toml".to_string(), "setup.py".to_string()],
            source_extensions: vec![".py".to_string()],
            skip_directories: vec![
                "venv".to_string(),
                ".venv".to_string(),
                "__pycache__".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
            ],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
        }
    }

    /// Create a boxed language parser for Python.
    #[must_use]
    pub fn parser() -> Box<dyn CoreLanguageParser> {
        Box::new(PythonAnalyzer::new())
    }
}

impl CoreLanguageParser for PythonAnalyzer {
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult {
        self.analyze_crate(unit_name, files)
    }

    fn emit_structural_items(
        &self,
        source_root: &Path,
        unit_name: &str,
        source_files: &[PathBuf],
    ) -> Vec<AnalysisItem> {
        emit_python_module_items(source_root, unit_name, source_files)
    }

    fn post_process(&self, source_root: &Path, unit_name: &str, result: &mut ParseResult) {
        // Reparent items to their file-level module QNs.
        for item in &mut result.items {
            if let Some(file_module_qn) =
                python_file_to_module_qn(source_root, &item.source_ref, unit_name)
            {
                let item_name = item
                    .qualified_name
                    .rsplit("::")
                    .next()
                    .unwrap_or("")
                    .to_string();

                // For methods, preserve the class::method structure.
                if item.sub_kind == "method" {
                    if let Some(ref parent_qn) = item.parent_qualified_name {
                        let class_name = parent_qn.rsplit("::").next().unwrap_or("").to_string();
                        item.qualified_name =
                            format!("{file_module_qn}::{class_name}::{item_name}");
                        item.parent_qualified_name =
                            Some(format!("{file_module_qn}::{class_name}"));
                    }
                } else {
                    item.qualified_name = format!("{file_module_qn}::{item_name}");
                    item.parent_qualified_name = Some(file_module_qn);
                }
            }
        }

        // Resolve relative imports (dot-prefixed).
        for rel in &mut result.relations {
            if rel.target_qualified_name.starts_with('.') {
                // Extract the source file's module context from the embedded path.
                // Format: ".module_path@/abs/file/path.py" or "..module_path@/abs/file/path.py"
                if let Some((dots_and_module, file_ref)) = rel.target_qualified_name.split_once('@')
                {
                    let source_module_qn =
                        python_file_to_module_qn(source_root, file_ref, unit_name)
                            .unwrap_or_else(|| unit_name.to_string());
                    if let Some(resolved) =
                        resolve_python_relative_import(dots_and_module, &source_module_qn)
                    {
                        rel.target_qualified_name = resolved;
                    }
                }
            }
        }
    }
}

impl LanguageAnalyzer for PythonAnalyzer {
    fn language_id(&self) -> &str {
        "python"
    }

    fn analyze_crate(&self, package_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-python grammar".to_string(),
            });
            return result;
        }

        for file in files {
            let is_test_file = is_python_test_file(file);
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_python_file(
                        &mut parser,
                        &source,
                        file,
                        package_name,
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

/// Check whether a file path refers to a Python test file.
fn is_python_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    name.starts_with("test_") || name.ends_with("_test.py") || name == "conftest.py"
}

fn parse_python_file(
    parser: &mut tree_sitter::Parser,
    source: &str,
    file: &Path,
    package_name: &str,
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

    for child in root.children(&mut root.walk()) {
        match child.kind() {
            "function_definition" => {
                extract_function(
                    &child,
                    source,
                    package_name,
                    None,
                    &source_ref_base,
                    is_test_file,
                    false,
                    result,
                );
            }
            "class_definition" => {
                extract_class(
                    &child,
                    source,
                    package_name,
                    &source_ref_base,
                    is_test_file,
                    result,
                );
            }
            "decorated_definition" => {
                for inner in child.children(&mut child.walk()) {
                    match inner.kind() {
                        "function_definition" => {
                            extract_function(
                                &inner,
                                source,
                                package_name,
                                None,
                                &source_ref_base,
                                is_test_file,
                                false,
                                result,
                            );
                        }
                        "class_definition" => {
                            extract_class(
                                &inner,
                                source,
                                package_name,
                                &source_ref_base,
                                is_test_file,
                                result,
                            );
                        }
                        _ => {}
                    }
                }
            }
            "import_statement" => {
                extract_import(&child, source, package_name, result);
            }
            "import_from_statement" => {
                extract_import_from(&child, source, package_name, &source_ref_base, result);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn extract_function(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    class_name: Option<&str>,
    source_ref_base: &str,
    is_test_file: bool,
    is_test_class: bool,
    result: &mut ParseResult,
) {
    let name = match node.child_by_field_name("name") {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => return,
        },
        None => return,
    };

    // Skip private/dunder methods for noise reduction (keep __init__)
    if name.starts_with('_') && name != "__init__" {
        return;
    }

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;
    let (qn, sub_kind, parent_qn) = if let Some(cls) = class_name {
        (
            format!("{package_name}::{cls}::{name}"),
            "method",
            Some(format!("{package_name}::{cls}")),
        )
    } else {
        (
            format!("{package_name}::{name}"),
            "function",
            Some(package_name.to_string()),
        )
    };

    let mut tags = Vec::new();
    if is_test_file || is_test_class || name.starts_with("test_") {
        tags.push("test".to_string());
    }

    result.items.push(AnalysisItem {
        qualified_name: qn.clone(),
        kind: NodeKind::Unit,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: parent_qn,
        source_ref: format!("{source_ref_base}:{line}"),
        language: "python".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Walk the function body for call expressions.
    if let Some(body) = node.child_by_field_name("body") {
        let class_qn = class_name.map(|cls| format!("{package_name}::{cls}"));
        visit_py_call_expressions(body, source, &qn, package_name, class_qn.as_deref(), result);
    }
}

fn extract_class(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    source_ref_base: &str,
    is_test_file: bool,
    result: &mut ParseResult,
) {
    let name = match node.child_by_field_name("name") {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => return,
        },
        None => return,
    };

    let is_test_class = is_test_file || name.starts_with("Test");

    let mut tags = Vec::new();
    if is_test_class {
        tags.push("test".to_string());
    }

    let line = node.start_position().row + 1;
    let loc = node.end_position().row - node.start_position().row + 1;
    result.items.push(AnalysisItem {
        qualified_name: format!("{package_name}::{name}"),
        kind: NodeKind::Unit,
        sub_kind: "class".to_string(),
        parent_qualified_name: Some(package_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "python".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
        tags,
    });

    // Extract methods from class body
    if let Some(body) = node.child_by_field_name("body") {
        for child in body.children(&mut body.walk()) {
            match child.kind() {
                "function_definition" => {
                    extract_function(
                        &child,
                        source,
                        package_name,
                        Some(&name),
                        source_ref_base,
                        is_test_file,
                        is_test_class,
                        result,
                    );
                }
                "decorated_definition" => {
                    for inner in child.children(&mut child.walk()) {
                        if inner.kind() == "function_definition" {
                            extract_function(
                                &inner,
                                source,
                                package_name,
                                Some(&name),
                                source_ref_base,
                                is_test_file,
                                is_test_class,
                                result,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Recursively walk AST nodes to find call expressions and emit `Calls` edges.
///
/// For each `call` node found, extracts the callee and emits an
/// `AnalysisRelation` with `EdgeKind::Calls` from the containing function
/// to the target function/method.
fn visit_py_call_expressions(
    node: tree_sitter::Node<'_>,
    source: &str,
    caller_qn: &str,
    package_name: &str,
    class_context: Option<&str>,
    result: &mut ParseResult,
) {
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        if child.kind() == "call" {
            // The first named child of a `call` node is the callee expression.
            if let Some(callee) = child.named_child(0) {
                match callee.kind() {
                    "identifier" => {
                        // Simple function call: foo()
                        if let Ok(name) = callee.utf8_text(source.as_bytes()) {
                            if !name.is_empty() {
                                result.relations.push(AnalysisRelation {
                                    source_qualified_name: caller_qn.to_string(),
                                    target_qualified_name: format!("{package_name}::{name}"),
                                    kind: EdgeKind::Calls,
                                });
                            }
                        }
                    }
                    "attribute" => {
                        // Method call: obj.method() or self.method()
                        let object = callee.child_by_field_name("object");
                        let attribute = callee.child_by_field_name("attribute");
                        if let (Some(obj), Some(attr)) = (object, attribute) {
                            if let (Ok(obj_text), Ok(attr_text)) = (
                                obj.utf8_text(source.as_bytes()),
                                attr.utf8_text(source.as_bytes()),
                            ) {
                                if obj_text == "self" {
                                    // self.method() — resolve to class method if in class context
                                    if let Some(cls_qn) = class_context {
                                        result.relations.push(AnalysisRelation {
                                            source_qualified_name: caller_qn.to_string(),
                                            target_qualified_name: format!("{cls_qn}::{attr_text}"),
                                            kind: EdgeKind::Calls,
                                        });
                                    }
                                } else {
                                    // obj.method() — best effort with receiver text
                                    result.relations.push(AnalysisRelation {
                                        source_qualified_name: caller_qn.to_string(),
                                        target_qualified_name: format!("{obj_text}::{attr_text}"),
                                        kind: EdgeKind::Calls,
                                    });
                                }
                            }
                        }
                    }
                    _ => {
                        // Other call forms — skip.
                    }
                }
            }
        }

        // Recurse into children to find nested call expressions.
        visit_py_call_expressions(
            child,
            source,
            caller_qn,
            package_name,
            class_context,
            result,
        );
    }
}

fn extract_import(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    result: &mut ParseResult,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "dotted_name" {
            if let Ok(name) = child.utf8_text(source.as_bytes()) {
                result.relations.push(AnalysisRelation {
                    source_qualified_name: package_name.to_string(),
                    target_qualified_name: name.replace('.', "::"),
                    kind: EdgeKind::Depends,
                });
            }
        }
    }
}

fn extract_import_from(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    source_ref_base: &str,
    result: &mut ParseResult,
) {
    // Check for relative imports (tree-sitter uses "relative_import" child)
    let mut has_relative = false;
    let mut dot_count = 0;
    let mut module_path = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "relative_import" {
            has_relative = true;
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                match inner.kind() {
                    "import_prefix" => {
                        if let Ok(text) = inner.utf8_text(source.as_bytes()) {
                            dot_count = text.chars().filter(|c| *c == '.').count();
                        }
                    }
                    "dotted_name" => {
                        if let Ok(text) = inner.utf8_text(source.as_bytes()) {
                            module_path = text.replace('.', "::");
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if has_relative {
        let dots: String = ".".repeat(dot_count);
        if module_path.is_empty() {
            // `from . import foo, bar` — emit one relation per imported name.
            // Each imported name is a module within the current package.
            let mut name_cursor = node.walk();
            for child in node.children(&mut name_cursor) {
                if child.kind() == "dotted_name" {
                    if let Ok(name) = child.utf8_text(source.as_bytes()) {
                        let target = format!("{dots}{name}@{source_ref_base}");
                        result.relations.push(AnalysisRelation {
                            source_qualified_name: package_name.to_string(),
                            target_qualified_name: target,
                            kind: EdgeKind::Depends,
                        });
                    }
                }
            }
        } else {
            // `from .module import foo` — depend on the module itself.
            let target = format!("{dots}{module_path}@{source_ref_base}");
            result.relations.push(AnalysisRelation {
                source_qualified_name: package_name.to_string(),
                target_qualified_name: target,
                kind: EdgeKind::Depends,
            });
        }
    } else if let Some(module_name) = node.child_by_field_name("module_name") {
        if let Ok(name) = module_name.utf8_text(source.as_bytes()) {
            result.relations.push(AnalysisRelation {
                source_qualified_name: package_name.to_string(),
                target_qualified_name: name.replace('.', "::"),
                kind: EdgeKind::Depends,
            });
        }
    }
}

/// Emit directory- and file-based module hierarchy nodes for Python packages.
///
/// `__init__.py` files represent their parent directory package and do not get
/// a separate file-level node. All other `.py` files get a `Component/module` node.
fn emit_python_module_items(
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

        // Emit directory modules.
        let mut current_qn = unit_name.to_string();
        for component in rel.parent().iter().flat_map(|p| p.components()) {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str().unwrap_or("");
                let parent_qn = current_qn.clone();
                current_qn = format!("{current_qn}::{name_str}");
                if emitted.insert(current_qn.clone()) {
                    items.push(AnalysisItem {
                        qualified_name: current_qn.clone(),
                        kind: NodeKind::Component,
                        sub_kind: "module".to_string(),
                        parent_qualified_name: Some(parent_qn),
                        source_ref: file.parent().unwrap_or(source_root).display().to_string(),
                        language: "python".to_string(),
                        metadata: None,
                        tags: vec![],
                    });
                }
            }
        }

        // Emit file-level module (skip __init__.py — it represents the directory package).
        let file_stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if file_stem == "__init__" {
            continue;
        }

        let file_qn = format!("{current_qn}::{file_stem}");
        if emitted.insert(file_qn.clone()) {
            items.push(AnalysisItem {
                qualified_name: file_qn,
                kind: NodeKind::Component,
                sub_kind: "module".to_string(),
                parent_qualified_name: Some(current_qn),
                source_ref: file.display().to_string(),
                language: "python".to_string(),
                metadata: None,
                tags: vec![],
            });
        }
    }

    items
}

/// Map a source_ref file path to its module qualified name.
///
/// For `__init__.py`, returns the directory/package QN (not `pkg::__init__`).
/// For other files, returns `pkg::filename` (without extension).
fn python_file_to_module_qn(
    source_root: &Path,
    source_ref: &str,
    package_name: &str,
) -> Option<String> {
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

    // __init__.py represents the directory package itself
    if stem != "__init__" {
        qn = format!("{qn}::{stem}");
    }

    Some(qn)
}

/// Resolve a Python relative import to a qualified name.
///
/// `dots_and_module` is the dot-prefixed import path (e.g., "..utils", ".foo").
/// `source_module_qn` is the current file's module QN (e.g., "pkg::sub::mod").
///
/// N dots means navigate up N-1 levels from the current module.
fn resolve_python_relative_import(dots_and_module: &str, source_module_qn: &str) -> Option<String> {
    let dot_count = dots_and_module.chars().take_while(|c| *c == '.').count();
    let module_suffix = &dots_and_module[dot_count..];

    if dot_count == 0 {
        return None;
    }

    // Navigate up: 1 dot = current package (go up 1 level from file module),
    // 2 dots = parent package, etc.
    let parts: Vec<&str> = source_module_qn.split("::").collect();
    let levels_up = dot_count;
    if levels_up >= parts.len() {
        return None; // Can't navigate above root
    }

    let base_parts = &parts[..parts.len() - levels_up];
    let mut resolved = base_parts.join("::");

    if !module_suffix.is_empty() {
        let suffix = module_suffix.replace('.', "::");
        resolved = format!("{resolved}::{suffix}");
    }

    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn parse_py_source(source: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.py");
        std::fs::write(&file, source).unwrap();
        let analyzer = PythonAnalyzer::new();
        let file_path = PathBuf::from(&file);
        analyzer.analyze_crate("mypackage", &[file_path.as_path()])
    }

    fn parse_py_source_file(source: &str, filename: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join(filename);
        std::fs::write(&file, source).unwrap();
        let analyzer = PythonAnalyzer::new();
        let file_path = PathBuf::from(&file);
        analyzer.analyze_crate("mypackage", &[file_path.as_path()])
    }

    #[test]
    fn extracts_top_level_function() {
        let result = parse_py_source("def hello():\n    pass\n");
        let funcs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].qualified_name.contains("hello"));
        assert_eq!(funcs[0].kind, NodeKind::Unit);
        assert_eq!(funcs[0].language, "python");
    }

    #[test]
    fn extracts_class_definition() {
        let result = parse_py_source("class MyService:\n    pass\n");
        let classes: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "class")
            .collect();
        assert_eq!(classes.len(), 1);
        assert!(classes[0].qualified_name.contains("MyService"));
        assert_eq!(classes[0].kind, NodeKind::Unit);
    }

    #[test]
    fn extracts_class_methods() {
        let result = parse_py_source(
            "class MyService:\n    def start(self):\n        pass\n    def stop(self):\n        pass\n",
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 2);
    }

    #[test]
    fn extracts_import_relations() {
        let result = parse_py_source("import os\nfrom pathlib import Path\n");
        let imports: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            imports.len() >= 2,
            "should have import dependencies for os and pathlib"
        );
    }

    #[test]
    fn extracts_decorated_function() {
        let result = parse_py_source("@staticmethod\ndef helper():\n    pass\n");
        let funcs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert_eq!(funcs.len(), 1);
    }

    #[test]
    fn handles_empty_file() {
        let result = parse_py_source("");
        assert!(result.items.is_empty());
    }

    #[test]
    fn language_id_is_python() {
        let analyzer = PythonAnalyzer::new();
        assert_eq!(analyzer.language_id(), "python");
    }

    // --- M25: Test detection tests ---

    #[test]
    fn test_file_items_tagged_as_test() {
        let result = parse_py_source_file(
            "def test_add():\n    pass\n\ndef helper():\n    pass\n",
            "test_math.py",
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
    fn test_file_suffix_pattern_detected() {
        let result = parse_py_source_file("def test_add():\n    pass\n", "math_test.py");
        assert!(result
            .items
            .iter()
            .all(|i| i.tags.contains(&"test".to_string())));
    }

    #[test]
    fn conftest_file_items_tagged_as_test() {
        let result = parse_py_source_file("def fixture_db():\n    pass\n", "conftest.py");
        assert!(result
            .items
            .iter()
            .all(|i| i.tags.contains(&"test".to_string())));
    }

    #[test]
    fn test_function_name_tagged_in_non_test_file() {
        let result = parse_py_source("def test_add():\n    pass\n\ndef helper():\n    pass\n");
        let test_items: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.tags.contains(&"test".to_string()))
            .collect();
        assert_eq!(test_items.len(), 1, "only test_add should be tagged");
        assert!(test_items[0].qualified_name.contains("test_add"));
    }

    #[test]
    fn test_class_and_methods_tagged() {
        let result = parse_py_source(
            "class TestMath:\n    def test_add(self):\n        pass\n    def helper(self):\n        pass\n",
        );
        let class = result.items.iter().find(|i| i.sub_kind == "class").unwrap();
        assert!(
            class.tags.contains(&"test".to_string()),
            "Test* class should be tagged"
        );

        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert!(
            methods.iter().all(|m| m.tags.contains(&"test".to_string())),
            "all methods in Test* class should be tagged"
        );
    }

    #[test]
    fn non_test_items_have_no_test_tag() {
        let result = parse_py_source("def hello():\n    pass\n\nclass Config:\n    pass\n");
        assert!(
            result.items.iter().all(|i| i.tags.is_empty()),
            "non-test items should have no tags"
        );
    }

    // --- M26: Module hierarchy tests ---

    #[test]
    fn emits_package_hierarchy_with_init_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("__init__.py"), "").unwrap();
        std::fs::write(root.join("utils.py"), "").unwrap();
        std::fs::write(root.join("sub/__init__.py"), "").unwrap();
        std::fs::write(root.join("sub/models.py"), "").unwrap();

        let files = vec![
            root.join("__init__.py"),
            root.join("utils.py"),
            root.join("sub/__init__.py"),
            root.join("sub/models.py"),
        ];

        let items = emit_python_module_items(root, "mypkg", &files);
        let qns: Vec<_> = items.iter().map(|i| i.qualified_name.as_str()).collect();

        // Directory modules
        assert!(
            qns.contains(&"mypkg::sub"),
            "should emit sub directory module"
        );
        // File-level modules (not __init__)
        assert!(qns.contains(&"mypkg::utils"), "should emit utils module");
        assert!(
            qns.contains(&"mypkg::sub::models"),
            "should emit sub::models module"
        );
        // __init__.py should NOT produce a separate node
        assert!(
            !qns.iter().any(|q| q.contains("__init__")),
            "__init__ should not produce a module node"
        );
    }

    #[test]
    fn deduplicates_directory_modules() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        std::fs::write(root.join("pkg/a.py"), "").unwrap();
        std::fs::write(root.join("pkg/b.py"), "").unwrap();

        let files = vec![root.join("pkg/a.py"), root.join("pkg/b.py")];
        let items = emit_python_module_items(root, "mypkg", &files);

        let pkg_modules: Vec<_> = items
            .iter()
            .filter(|i| i.qualified_name == "mypkg::pkg")
            .collect();
        assert_eq!(pkg_modules.len(), 1, "should deduplicate pkg module");
    }

    #[test]
    fn post_process_reparents_items_to_file_module() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let file = root.join("sub/utils.py");
        std::fs::write(&file, "def helper():\n    pass\n").unwrap();

        let analyzer = PythonAnalyzer::new();
        let mut result = analyzer.analyze_crate("mypkg", &[file.as_path()]);

        let func = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function")
            .unwrap();
        assert_eq!(func.qualified_name, "mypkg::helper");

        analyzer.post_process(root, "mypkg", &mut result);

        let func = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function")
            .unwrap();
        assert_eq!(func.qualified_name, "mypkg::sub::utils::helper");
        assert_eq!(
            func.parent_qualified_name,
            Some("mypkg::sub::utils".to_string())
        );
    }

    #[test]
    fn post_process_reparents_init_items_to_package() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let file = root.join("sub/__init__.py");
        std::fs::write(&file, "def setup():\n    pass\n").unwrap();

        let analyzer = PythonAnalyzer::new();
        let mut result = analyzer.analyze_crate("mypkg", &[file.as_path()]);
        analyzer.post_process(root, "mypkg", &mut result);

        let func = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function")
            .unwrap();
        // __init__.py items belong to the package, not __init__ sub-module
        assert_eq!(func.qualified_name, "mypkg::sub::setup");
        assert_eq!(func.parent_qualified_name, Some("mypkg::sub".to_string()));
    }

    #[test]
    fn post_process_reparents_method_preserving_class() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        let file = root.join("pkg/service.py");
        std::fs::write(&file, "class Server:\n    def start(self):\n        pass\n").unwrap();

        let analyzer = PythonAnalyzer::new();
        let mut result = analyzer.analyze_crate("mypkg", &[file.as_path()]);
        analyzer.post_process(root, "mypkg", &mut result);

        let method = result
            .items
            .iter()
            .find(|i| i.sub_kind == "method")
            .unwrap();
        assert_eq!(method.qualified_name, "mypkg::pkg::service::Server::start");
        assert_eq!(
            method.parent_qualified_name,
            Some("mypkg::pkg::service::Server".to_string())
        );
    }

    #[test]
    fn python_file_to_module_qn_handles_init() {
        let root = Path::new("/project/src");
        let result = python_file_to_module_qn(root, "/project/src/sub/__init__.py:1", "mypkg");
        assert_eq!(result, Some("mypkg::sub".to_string()));
    }

    #[test]
    fn python_file_to_module_qn_handles_regular_file() {
        let root = Path::new("/project/src");
        let result = python_file_to_module_qn(root, "/project/src/sub/utils.py:5", "mypkg");
        assert_eq!(result, Some("mypkg::sub::utils".to_string()));
    }

    #[test]
    fn python_file_to_module_qn_handles_root_file() {
        let root = Path::new("/project/src");
        let result = python_file_to_module_qn(root, "/project/src/main.py:1", "mypkg");
        assert_eq!(result, Some("mypkg::main".to_string()));
    }

    // --- Relative import resolution tests ---

    #[test]
    fn resolves_single_dot_relative_import() {
        let result = resolve_python_relative_import(".utils", "mypkg::sub::mod");
        assert_eq!(result, Some("mypkg::sub::utils".to_string()));
    }

    #[test]
    fn resolves_double_dot_relative_import() {
        let result = resolve_python_relative_import("..config", "mypkg::sub::mod");
        assert_eq!(result, Some("mypkg::config".to_string()));
    }

    #[test]
    fn resolves_bare_dot_import() {
        let result = resolve_python_relative_import(".", "mypkg::sub::mod");
        assert_eq!(result, Some("mypkg::sub".to_string()));
    }

    #[test]
    fn returns_none_for_too_many_dots() {
        let result = resolve_python_relative_import("....foo", "mypkg::sub");
        assert_eq!(result, None);
    }

    #[test]
    fn returns_none_for_non_relative_import() {
        let result = resolve_python_relative_import("os", "mypkg::sub");
        assert_eq!(result, None);
    }

    #[test]
    fn extracts_relative_import_with_file_ref() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let file = root.join("sub/mod.py");
        std::fs::write(&file, "from . import utils\n").unwrap();

        let analyzer = PythonAnalyzer::new();
        let result = analyzer.analyze_crate("mypkg", &[file.as_path()]);

        let rel = result
            .relations
            .iter()
            .find(|r| r.kind == EdgeKind::Depends);
        assert!(rel.is_some(), "should extract relative import");
        let target = &rel.unwrap().target_qualified_name;
        assert!(target.starts_with('.'), "should preserve dots");
        assert!(target.contains('@'), "should embed file path");
    }

    #[test]
    fn post_process_resolves_relative_imports() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let file = root.join("sub/mod.py");
        std::fs::write(&file, "from . import utils\n").unwrap();

        let analyzer = PythonAnalyzer::new();
        let mut result = analyzer.analyze_crate("mypkg", &[file.as_path()]);
        analyzer.post_process(root, "mypkg", &mut result);

        let rel = result
            .relations
            .iter()
            .find(|r| r.kind == EdgeKind::Depends);
        assert!(rel.is_some());
        assert_eq!(
            rel.unwrap().target_qualified_name,
            "mypkg::sub::utils",
            "relative import should be resolved"
        );
    }

    // --- M28: Call graph analysis tests ---

    #[test]
    fn extracts_function_call_edge() {
        let result = parse_py_source(
            r#"def helper():
    pass

def main():
    helper()
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
                .any(|c| c.source_qualified_name == "mypackage::main"
                    && c.target_qualified_name == "mypackage::helper"),
            "should have main -> helper Calls edge, got: {:?}",
            calls
        );
    }

    #[test]
    fn extracts_method_call_edge() {
        let result = parse_py_source(
            r#"class Service:
    def start(self):
        pass
    def run(self):
        self.start()
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
                .any(|c| c.source_qualified_name == "mypackage::Service::run"
                    && c.target_qualified_name == "mypackage::Service::start"),
            "should have Service::run -> Service::start Calls edge, got: {:?}",
            calls
        );
    }

    #[test]
    fn does_not_emit_calls_for_imports() {
        let result = parse_py_source("import os\nfrom pathlib import Path\n");
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
        let result = parse_py_source(
            r#"def helper():
    pass

def main():
    if True:
        helper()
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
                .any(|c| c.source_qualified_name == "mypackage::main"
                    && c.target_qualified_name == "mypackage::helper"),
            "should find calls inside if blocks, got: {:?}",
            calls
        );
    }
}
