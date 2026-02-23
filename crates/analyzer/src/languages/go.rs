//! Go language analyzer using tree-sitter-go.
//!
//! Extracts structural elements (packages, structs, interfaces, functions,
//! methods) and import relationships from Go source files.

use std::path::Path;

use svt_core::analysis::{LanguageDescriptor, LanguageParser as CoreLanguageParser};
use svt_core::model::{EdgeKind, NodeKind};

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

use super::{LanguageAnalyzer, ParseResult};

/// Go source code analyzer using tree-sitter-go.
#[derive(Debug)]
pub struct GoAnalyzer {
    _private: (),
}

impl GoAnalyzer {
    /// Create a new `GoAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for GoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl GoAnalyzer {
    /// Language descriptor for Go modules.
    #[must_use]
    pub fn descriptor() -> LanguageDescriptor {
        LanguageDescriptor {
            language_id: "go".to_string(),
            manifest_files: vec!["go.mod".to_string()],
            source_extensions: vec![".go".to_string()],
            skip_directories: vec![
                "vendor".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "dist".to_string(),
            ],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        }
    }

    /// Create a boxed language parser for Go.
    #[must_use]
    pub fn parser() -> Box<dyn CoreLanguageParser> {
        Box::new(GoAnalyzer::new())
    }
}

impl CoreLanguageParser for GoAnalyzer {
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult {
        self.analyze_crate(unit_name, files)
    }
}

impl LanguageAnalyzer for GoAnalyzer {
    fn language_id(&self) -> &str {
        "go"
    }

    fn analyze_crate(&self, module_name: &str, files: &[&Path]) -> ParseResult {
        let mut result = ParseResult::default();

        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .is_err()
        {
            result.warnings.push(AnalysisWarning {
                source_ref: String::new(),
                message: "failed to load tree-sitter-go grammar".to_string(),
            });
            return result;
        }

        for file in files {
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_go_file(&mut parser, &source, file, module_name, &mut result);
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

fn parse_go_file(
    parser: &mut tree_sitter::Parser,
    source: &str,
    file: &Path,
    module_name: &str,
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
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        let line = child.start_position().row + 1;
                        let loc = child.end_position().row - child.start_position().row + 1;
                        result.items.push(AnalysisItem {
                            qualified_name: format!("{module_name}::{name}"),
                            kind: NodeKind::Unit,
                            sub_kind: "function".to_string(),
                            parent_qualified_name: Some(module_name.to_string()),
                            source_ref: format!("{source_ref_base}:{line}"),
                            language: "go".to_string(),
                            metadata: Some(serde_json::json!({"loc": loc})),
                        });
                    }
                }
            }
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        let receiver_type = extract_receiver_type(&child, source);
                        let line = child.start_position().row + 1;
                        let loc = child.end_position().row - child.start_position().row + 1;
                        let qn = if let Some(ref recv) = receiver_type {
                            format!("{module_name}::{recv}::{name}")
                        } else {
                            format!("{module_name}::{name}")
                        };
                        result.items.push(AnalysisItem {
                            qualified_name: qn,
                            kind: NodeKind::Unit,
                            sub_kind: "method".to_string(),
                            parent_qualified_name: receiver_type
                                .map(|r| format!("{module_name}::{r}"))
                                .or_else(|| Some(module_name.to_string())),
                            source_ref: format!("{source_ref_base}:{line}"),
                            language: "go".to_string(),
                            metadata: Some(serde_json::json!({"loc": loc})),
                        });
                    }
                }
            }
            "type_declaration" => {
                for spec in child.children(&mut child.walk()) {
                    if spec.kind() == "type_spec" {
                        extract_type_spec(&spec, source, module_name, &source_ref_base, result);
                    }
                }
            }
            "import_declaration" => {
                extract_imports(&child, source, module_name, result);
            }
            _ => {}
        }
    }
}

fn extract_type_spec(
    spec: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    source_ref_base: &str,
    result: &mut ParseResult,
) {
    let name = match spec.child_by_field_name("name") {
        Some(n) => match n.utf8_text(source.as_bytes()) {
            Ok(s) => s.to_string(),
            Err(_) => return,
        },
        None => return,
    };

    let type_node = spec.child_by_field_name("type");
    let sub_kind = match type_node.map(|n| n.kind()) {
        Some("struct_type") => "struct",
        Some("interface_type") => "interface",
        _ => "type_alias",
    };

    let line = spec.start_position().row + 1;
    let loc = spec.end_position().row - spec.start_position().row + 1;
    result.items.push(AnalysisItem {
        qualified_name: format!("{module_name}::{name}"),
        kind: NodeKind::Unit,
        sub_kind: sub_kind.to_string(),
        parent_qualified_name: Some(module_name.to_string()),
        source_ref: format!("{source_ref_base}:{line}"),
        language: "go".to_string(),
        metadata: Some(serde_json::json!({"loc": loc})),
    });
}

fn extract_receiver_type(method: &tree_sitter::Node, source: &str) -> Option<String> {
    let params = method.child_by_field_name("receiver")?;
    let mut cursor = params.walk();
    for child in params.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            if let Some(type_node) = child.child_by_field_name("type") {
                let text = type_node.utf8_text(source.as_bytes()).ok()?;
                return Some(text.trim_start_matches('*').to_string());
            }
        }
    }
    None
}

fn extract_imports(
    import_decl: &tree_sitter::Node,
    source: &str,
    module_name: &str,
    result: &mut ParseResult,
) {
    let mut cursor = import_decl.walk();
    for child in import_decl.children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                if let Some(path_node) = child.child_by_field_name("path") {
                    if let Ok(path) = path_node.utf8_text(source.as_bytes()) {
                        let import_path = path.trim_matches('"');
                        result.relations.push(AnalysisRelation {
                            source_qualified_name: module_name.to_string(),
                            target_qualified_name: import_path.to_string(),
                            kind: EdgeKind::Depends,
                        });
                    }
                }
            }
            "import_spec_list" => {
                let mut inner_cursor = child.walk();
                for spec in child.children(&mut inner_cursor) {
                    if spec.kind() == "import_spec" {
                        if let Some(path_node) = spec.child_by_field_name("path") {
                            if let Ok(path) = path_node.utf8_text(source.as_bytes()) {
                                let import_path = path.trim_matches('"');
                                result.relations.push(AnalysisRelation {
                                    source_qualified_name: module_name.to_string(),
                                    target_qualified_name: import_path.to_string(),
                                    kind: EdgeKind::Depends,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn parse_go_source(source: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.go");
        std::fs::write(&file, source).unwrap();
        let analyzer = GoAnalyzer::new();
        let file_path = PathBuf::from(&file);
        analyzer.analyze_crate("myapp", &[file_path.as_path()])
    }

    #[test]
    fn extracts_package_level_function() {
        let result = parse_go_source("package main\n\nfunc Hello() {}\n");
        let funcs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert_eq!(funcs.len(), 1);
        assert!(funcs[0].qualified_name.contains("Hello"));
        assert_eq!(funcs[0].kind, NodeKind::Unit);
        assert_eq!(funcs[0].language, "go");
    }

    #[test]
    fn extracts_struct_type() {
        let result = parse_go_source("package main\n\ntype Server struct {\n\tPort int\n}\n");
        let structs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "struct")
            .collect();
        assert_eq!(structs.len(), 1);
        assert!(structs[0].qualified_name.contains("Server"));
        assert_eq!(structs[0].kind, NodeKind::Unit);
    }

    #[test]
    fn extracts_interface_type() {
        let result =
            parse_go_source("package main\n\ntype Handler interface {\n\tHandle() error\n}\n");
        let ifaces: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "interface")
            .collect();
        assert_eq!(ifaces.len(), 1);
        assert!(ifaces[0].qualified_name.contains("Handler"));
    }

    #[test]
    fn extracts_method_declaration() {
        let result = parse_go_source(
            "package main\n\ntype Server struct{}\n\nfunc (s *Server) Start() {}\n",
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 1);
        assert!(
            methods[0].qualified_name.contains("Server")
                && methods[0].qualified_name.contains("Start")
        );
    }

    #[test]
    fn extracts_import_relations() {
        let result = parse_go_source(
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hello\")\n}\n",
        );
        let imports: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            !imports.is_empty(),
            "should have at least one import dependency"
        );
    }

    #[test]
    fn handles_empty_file_gracefully() {
        let result = parse_go_source("");
        assert!(result.items.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn language_id_is_go() {
        let analyzer = GoAnalyzer::new();
        assert_eq!(analyzer.language_id(), "go");
    }
}
