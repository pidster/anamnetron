//! Python language analyzer using tree-sitter-python.
//!
//! Extracts structural elements (classes, functions, methods) and import
//! relationships from Python source files.

use std::path::Path;

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
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_python_file(&mut parser, &source, file, package_name, &mut result);
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

fn parse_python_file(
    parser: &mut tree_sitter::Parser,
    source: &str,
    file: &Path,
    package_name: &str,
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
                extract_function(&child, source, package_name, None, &source_ref_base, result);
            }
            "class_definition" => {
                extract_class(&child, source, package_name, &source_ref_base, result);
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
                                result,
                            );
                        }
                        "class_definition" => {
                            extract_class(&inner, source, package_name, &source_ref_base, result);
                        }
                        _ => {}
                    }
                }
            }
            "import_statement" => {
                extract_import(&child, source, package_name, result);
            }
            "import_from_statement" => {
                extract_import_from(&child, source, package_name, result);
            }
            _ => {}
        }
    }
}

fn extract_function(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    class_name: Option<&str>,
    source_ref_base: &str,
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

    result.items.push(AnalysisItem {
        qualified_name: qn,
        kind: NodeKind::Unit,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: parent_qn,
        source_ref: format!("{source_ref_base}:{line}"),
        language: "python".to_string(),
    });
}

fn extract_class(
    node: &tree_sitter::Node,
    source: &str,
    package_name: &str,
    source_ref_base: &str,
    result: &mut ParseResult,
) {
    let name = match node.child_by_field_name("name") {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => return,
        },
        None => return,
    };

    let line = node.start_position().row + 1;
    result.items.push(AnalysisItem {
        qualified_name: format!("{package_name}::{name}"),
        kind: NodeKind::Unit,
        sub_kind: "class".to_string(),
        parent_qualified_name: Some(package_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "python".to_string(),
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
    result: &mut ParseResult,
) {
    if let Some(module_name) = node.child_by_field_name("module_name") {
        if let Ok(name) = module_name.utf8_text(source.as_bytes()) {
            result.relations.push(AnalysisRelation {
                source_qualified_name: package_name.to_string(),
                target_qualified_name: name.replace('.', "::"),
                kind: EdgeKind::Depends,
            });
        }
    }
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
}
