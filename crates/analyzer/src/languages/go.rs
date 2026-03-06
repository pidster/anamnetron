//! Go language analyzer using tree-sitter-go.
//!
//! Extracts structural elements (packages, structs, interfaces, functions,
//! methods) and import relationships from Go source files.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

    fn emit_structural_items(
        &self,
        source_root: &Path,
        unit_name: &str,
        source_files: &[PathBuf],
    ) -> Vec<AnalysisItem> {
        emit_go_module_items(source_root, unit_name, source_files)
    }

    fn post_process(&self, source_root: &Path, unit_name: &str, result: &mut ParseResult) {
        // Reparent items to their directory-level (package-level) module QNs.
        for item in &mut result.items {
            if let Some(dir_qn) = go_dir_to_module_qn(source_root, &item.source_ref, unit_name) {
                let item_name = item
                    .qualified_name
                    .rsplit("::")
                    .next()
                    .unwrap_or("")
                    .to_string();
                // For methods, preserve the receiver::method structure.
                // Check if the item has a parent that isn't the unit_name itself.
                if item.sub_kind == "method" {
                    if let Some(ref parent_qn) = item.parent_qualified_name {
                        // Extract the receiver type name from the old parent QN.
                        let recv = parent_qn.rsplit("::").next().unwrap_or("").to_string();
                        item.qualified_name = format!("{dir_qn}::{recv}::{item_name}");
                        item.parent_qualified_name = Some(format!("{dir_qn}::{recv}"));
                    }
                } else {
                    item.qualified_name = format!("{dir_qn}::{item_name}");
                    item.parent_qualified_name = Some(dir_qn);
                }
            }
        }
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
            let is_test_file = is_go_test_file(file);
            match std::fs::read_to_string(file) {
                Ok(source) => {
                    parse_go_file(
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

/// Check whether a file path refers to a Go test file.
fn is_go_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|f| f.to_str())
        .is_some_and(|name| name.ends_with("_test.go"))
}

/// Compute test tags for a Go item based on its name and file context.
fn go_test_tags(name: &str, is_test_file: bool) -> Vec<String> {
    if is_test_file {
        return vec!["test".to_string()];
    }
    if name.starts_with("Test")
        || name.starts_with("Benchmark")
        || name.starts_with("Example")
        || name.starts_with("Fuzz")
    {
        return vec!["test".to_string()];
    }
    vec![]
}

fn parse_go_file(
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

    // First pass: collect import aliases for package resolution in call expressions.
    let mut import_aliases: HashMap<String, String> = HashMap::new();
    for child in root.children(&mut root.walk()) {
        if child.kind() == "import_declaration" {
            collect_go_import_aliases(&child, source, &mut import_aliases);
        }
    }

    for child in root.children(&mut root.walk()) {
        match child.kind() {
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source.as_bytes()) {
                        let line = child.start_position().row + 1;
                        let loc = child.end_position().row - child.start_position().row + 1;
                        let func_qn = format!("{module_name}::{name}");
                        result.items.push(AnalysisItem {
                            qualified_name: func_qn.clone(),
                            kind: NodeKind::Unit,
                            sub_kind: "function".to_string(),
                            parent_qualified_name: Some(module_name.to_string()),
                            source_ref: format!("{source_ref_base}:{line}"),
                            language: "go".to_string(),
                            metadata: Some(serde_json::json!({"loc": loc})),
                            tags: go_test_tags(name, is_test_file),
                        });

                        // Walk the function body for call expressions.
                        if let Some(body) = child.child_by_field_name("body") {
                            visit_go_call_expressions(
                                body,
                                source,
                                &func_qn,
                                module_name,
                                &import_aliases,
                                result,
                            );
                        }
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
                            qualified_name: qn.clone(),
                            kind: NodeKind::Unit,
                            sub_kind: "method".to_string(),
                            parent_qualified_name: receiver_type
                                .map(|r| format!("{module_name}::{r}"))
                                .or_else(|| Some(module_name.to_string())),
                            source_ref: format!("{source_ref_base}:{line}"),
                            language: "go".to_string(),
                            metadata: Some(serde_json::json!({"loc": loc})),
                            tags: go_test_tags(name, is_test_file),
                        });

                        // Walk the method body for call expressions.
                        if let Some(body) = child.child_by_field_name("body") {
                            visit_go_call_expressions(
                                body,
                                source,
                                &qn,
                                module_name,
                                &import_aliases,
                                result,
                            );
                        }
                    }
                }
            }
            "type_declaration" => {
                for spec in child.children(&mut child.walk()) {
                    if spec.kind() == "type_spec" {
                        extract_type_spec(
                            &spec,
                            source,
                            module_name,
                            &source_ref_base,
                            is_test_file,
                            result,
                        );
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
    is_test_file: bool,
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

    let tags = if is_test_file {
        vec!["test".to_string()]
    } else {
        vec![]
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
        tags,
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

/// Collect import aliases from a Go import declaration.
///
/// Maps the local alias (or last path segment) to the full import path.
/// For example, `import "fmt"` maps `"fmt"` -> `"fmt"`, and
/// `import http "net/http"` maps `"http"` -> `"net/http"`.
fn collect_go_import_aliases(
    import_decl: &tree_sitter::Node,
    source: &str,
    aliases: &mut HashMap<String, String>,
) {
    let mut cursor = import_decl.walk();
    for child in import_decl.children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                collect_single_import_alias(&child, source, aliases);
            }
            "import_spec_list" => {
                let mut inner_cursor = child.walk();
                for spec in child.children(&mut inner_cursor) {
                    if spec.kind() == "import_spec" {
                        collect_single_import_alias(&spec, source, aliases);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Collect a single import alias from an `import_spec` node.
fn collect_single_import_alias(
    spec: &tree_sitter::Node,
    source: &str,
    aliases: &mut HashMap<String, String>,
) {
    if let Some(path_node) = spec.child_by_field_name("path") {
        if let Ok(path) = path_node.utf8_text(source.as_bytes()) {
            let import_path = path.trim_matches('"');
            // Check for explicit alias (e.g., `http "net/http"`)
            let alias = spec
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .map(|s| s.to_string());
            let local_name = alias.unwrap_or_else(|| {
                // Default alias is the last path segment.
                import_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(import_path)
                    .to_string()
            });
            if local_name != "_" && local_name != "." {
                aliases.insert(local_name, import_path.to_string());
            }
        }
    }
}

/// Recursively walk AST nodes to find call expressions and emit `Calls` edges.
///
/// For each `call_expression` found, extracts the callee and emits an
/// `AnalysisRelation` with `EdgeKind::Calls` from the containing function
/// to the target function/method.
fn visit_go_call_expressions(
    node: tree_sitter::Node<'_>,
    source: &str,
    caller_qn: &str,
    module_name: &str,
    import_aliases: &HashMap<String, String>,
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
                        if let Ok(name) = function.utf8_text(source.as_bytes()) {
                            if !name.is_empty() {
                                result.relations.push(AnalysisRelation {
                                    source_qualified_name: caller_qn.to_string(),
                                    target_qualified_name: format!("{module_name}::{name}"),
                                    kind: EdgeKind::Calls,
                                });
                            }
                        }
                    }
                    "selector_expression" => {
                        // Method/package call: obj.Method() or pkg.Func()
                        let operand = function.child_by_field_name("operand");
                        let field = function.child_by_field_name("field");
                        if let (Some(op), Some(fld)) = (operand, field) {
                            if let (Ok(op_text), Ok(fld_text)) = (
                                op.utf8_text(source.as_bytes()),
                                fld.utf8_text(source.as_bytes()),
                            ) {
                                if let Some(import_path) = import_aliases.get(op_text) {
                                    // Package call: pkg.Func() -> import_path::Func
                                    result.relations.push(AnalysisRelation {
                                        source_qualified_name: caller_qn.to_string(),
                                        target_qualified_name: format!("{import_path}::{fld_text}"),
                                        kind: EdgeKind::Calls,
                                    });
                                } else {
                                    // Method call on a local variable: obj.Method()
                                    result.relations.push(AnalysisRelation {
                                        source_qualified_name: caller_qn.to_string(),
                                        target_qualified_name: format!("{op_text}::{fld_text}"),
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
        visit_go_call_expressions(
            child,
            source,
            caller_qn,
            module_name,
            import_aliases,
            result,
        );
    }
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

/// Emit directory-based module hierarchy nodes for Go packages.
///
/// In Go, all files in a directory share the same package, so we emit
/// `Component/module` nodes only for directories — not individual files.
fn emit_go_module_items(
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
                        language: "go".to_string(),
                        metadata: None,
                        tags: vec![],
                    });
                }
            }
        }
    }

    items
}

/// Map a source_ref file path to its directory-level module qualified name.
///
/// In Go, all files in the same directory belong to the same package, so
/// the module QN is based on the directory path, not the file stem.
fn go_dir_to_module_qn(source_root: &Path, source_ref: &str, unit_name: &str) -> Option<String> {
    let file_path_str = source_ref
        .rsplit_once(':')
        .map(|(p, _)| p)
        .unwrap_or(source_ref);
    let file_path = Path::new(file_path_str);
    let rel = file_path.strip_prefix(source_root).ok()?;

    let mut qn = unit_name.to_string();
    for component in rel.parent().iter().flat_map(|p| p.components()) {
        if let std::path::Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                qn = format!("{qn}::{name_str}");
            }
        }
    }

    Some(qn)
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

    fn parse_go_source_file(source: &str, filename: &str) -> ParseResult {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join(filename);
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

    // --- M25: Test detection tests ---

    #[test]
    fn test_file_items_tagged_as_test() {
        let result = parse_go_source_file(
            "package main\n\nfunc TestAdd(t *testing.T) {}\nfunc helper() {}\n",
            "math_test.go",
        );
        let items: Vec<_> = result.items.iter().collect();
        assert!(
            items.iter().all(|i| i.tags.contains(&"test".to_string())),
            "all items in _test.go files should be tagged as test"
        );
    }

    #[test]
    fn test_function_name_tagged_in_non_test_file() {
        let result = parse_go_source(
            "package main\n\nfunc TestAdd() {}\nfunc BenchmarkSort() {}\nfunc ExamplePrint() {}\nfunc FuzzParse() {}\nfunc helper() {}\n",
        );
        let test_items: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.tags.contains(&"test".to_string()))
            .collect();
        assert_eq!(
            test_items.len(),
            4,
            "Test*, Benchmark*, Example*, Fuzz* should be tagged"
        );
        let non_test: Vec<_> = result.items.iter().filter(|i| i.tags.is_empty()).collect();
        assert_eq!(non_test.len(), 1, "helper should not be tagged");
    }

    #[test]
    fn non_test_items_have_no_test_tag() {
        let result = parse_go_source("package main\n\nfunc Hello() {}\ntype Config struct{}\n");
        assert!(
            result.items.iter().all(|i| i.tags.is_empty()),
            "non-test items should have no tags"
        );
    }

    #[test]
    fn test_file_struct_tagged_as_test() {
        let result = parse_go_source_file(
            "package main\n\ntype testHelper struct{}\n",
            "helper_test.go",
        );
        let structs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "struct")
            .collect();
        assert_eq!(structs.len(), 1);
        assert!(structs[0].tags.contains(&"test".to_string()));
    }

    // --- M26: Module hierarchy tests ---

    #[test]
    fn emits_directory_based_module_hierarchy() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create nested directory structure
        std::fs::create_dir_all(root.join("cmd/server")).unwrap();
        std::fs::create_dir_all(root.join("pkg/handler")).unwrap();
        std::fs::write(root.join("main.go"), "package main").unwrap();
        std::fs::write(root.join("cmd/server/main.go"), "package main").unwrap();
        std::fs::write(root.join("pkg/handler/handler.go"), "package handler").unwrap();

        let files = vec![
            root.join("main.go"),
            root.join("cmd/server/main.go"),
            root.join("pkg/handler/handler.go"),
        ];

        let items = emit_go_module_items(root, "myapp", &files);

        let module_qns: Vec<_> = items.iter().map(|i| i.qualified_name.as_str()).collect();
        assert!(module_qns.contains(&"myapp::cmd"), "should emit cmd module");
        assert!(
            module_qns.contains(&"myapp::cmd::server"),
            "should emit cmd::server module"
        );
        assert!(module_qns.contains(&"myapp::pkg"), "should emit pkg module");
        assert!(
            module_qns.contains(&"myapp::pkg::handler"),
            "should emit pkg::handler module"
        );

        // All should be Component/module
        assert!(items
            .iter()
            .all(|i| i.kind == NodeKind::Component && i.sub_kind == "module"));
    }

    #[test]
    fn deduplicates_directory_modules() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        std::fs::write(root.join("pkg/a.go"), "package pkg").unwrap();
        std::fs::write(root.join("pkg/b.go"), "package pkg").unwrap();

        let files = vec![root.join("pkg/a.go"), root.join("pkg/b.go")];
        let items = emit_go_module_items(root, "myapp", &files);

        let pkg_modules: Vec<_> = items
            .iter()
            .filter(|i| i.qualified_name == "myapp::pkg")
            .collect();
        assert_eq!(pkg_modules.len(), 1, "should deduplicate pkg module");
    }

    #[test]
    fn emits_no_modules_for_root_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::write(root.join("main.go"), "package main").unwrap();

        let files = vec![root.join("main.go")];
        let items = emit_go_module_items(root, "myapp", &files);

        assert!(
            items.is_empty(),
            "root-level files produce no directory modules"
        );
    }

    #[test]
    fn post_process_reparents_items_to_directory_module() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("pkg/handler")).unwrap();
        let file = root.join("pkg/handler/handler.go");
        std::fs::write(&file, "package handler\n\nfunc Handle() {}\n").unwrap();

        let analyzer = GoAnalyzer::new();
        let mut result = analyzer.analyze_crate("myapp", &[file.as_path()]);

        // Before post_process, items are flat under unit_name
        let func = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function")
            .unwrap();
        assert_eq!(func.qualified_name, "myapp::Handle");

        analyzer.post_process(root, "myapp", &mut result);

        // After post_process, items are under directory module
        let func = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function")
            .unwrap();
        assert_eq!(func.qualified_name, "myapp::pkg::handler::Handle");
        assert_eq!(
            func.parent_qualified_name,
            Some("myapp::pkg::handler".to_string())
        );
    }

    #[test]
    fn post_process_reparents_method_preserving_receiver() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("pkg")).unwrap();
        let file = root.join("pkg/server.go");
        std::fs::write(
            &file,
            "package pkg\n\ntype Server struct{}\n\nfunc (s *Server) Start() {}\n",
        )
        .unwrap();

        let analyzer = GoAnalyzer::new();
        let mut result = analyzer.analyze_crate("myapp", &[file.as_path()]);
        analyzer.post_process(root, "myapp", &mut result);

        let method = result
            .items
            .iter()
            .find(|i| i.sub_kind == "method")
            .unwrap();
        assert_eq!(method.qualified_name, "myapp::pkg::Server::Start");
        assert_eq!(
            method.parent_qualified_name,
            Some("myapp::pkg::Server".to_string())
        );
    }

    #[test]
    fn post_process_handles_root_level_files() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let file = root.join("main.go");
        std::fs::write(&file, "package main\n\nfunc main() {}\n").unwrap();

        let analyzer = GoAnalyzer::new();
        let mut result = analyzer.analyze_crate("myapp", &[file.as_path()]);
        analyzer.post_process(root, "myapp", &mut result);

        let func = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function")
            .unwrap();
        // Root-level files stay under unit_name
        assert_eq!(func.qualified_name, "myapp::main");
        assert_eq!(func.parent_qualified_name, Some("myapp".to_string()));
    }

    #[test]
    fn go_dir_to_module_qn_extracts_directory_path() {
        let root = Path::new("/project/src");
        let result = go_dir_to_module_qn(root, "/project/src/pkg/handler/handler.go:10", "myapp");
        assert_eq!(result, Some("myapp::pkg::handler".to_string()));
    }

    #[test]
    fn go_dir_to_module_qn_returns_unit_name_for_root_files() {
        let root = Path::new("/project/src");
        let result = go_dir_to_module_qn(root, "/project/src/main.go:1", "myapp");
        assert_eq!(result, Some("myapp".to_string()));
    }

    #[test]
    fn go_dir_to_module_qn_returns_none_for_unrelated_path() {
        let root = Path::new("/project/src");
        let result = go_dir_to_module_qn(root, "/other/path/file.go:1", "myapp");
        assert_eq!(result, None);
    }

    // --- M28: Call graph analysis tests ---

    #[test]
    fn extracts_function_call_edge() {
        let result = parse_go_source(
            r#"package main

func helper() {}

func main() {
    helper()
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
                .any(|c| c.source_qualified_name == "myapp::main"
                    && c.target_qualified_name == "myapp::helper"),
            "should have main -> helper Calls edge, got: {:?}",
            calls
        );
    }

    #[test]
    fn extracts_method_call_edge() {
        let result = parse_go_source(
            r#"package main

import "fmt"

type Server struct{}

func (s *Server) Start() {
    fmt.Println("starting")
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
                .any(|c| c.source_qualified_name == "myapp::Server::Start"
                    && c.target_qualified_name == "fmt::Println"),
            "should have Server::Start -> fmt::Println Calls edge, got: {:?}",
            calls
        );
    }

    #[test]
    fn does_not_emit_calls_for_imports() {
        let result = parse_go_source(
            r#"package main

import "fmt"
"#,
        );
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
        let result = parse_go_source(
            r#"package main

func helper() {}

func main() {
    if true {
        helper()
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
                .any(|c| c.source_qualified_name == "myapp::main"
                    && c.target_qualified_name == "myapp::helper"),
            "should find calls inside if blocks, got: {:?}",
            calls
        );
    }

    // --- Additional coverage tests ---

    #[test]
    fn package_only_file_produces_no_items() {
        let result = parse_go_source("package main\n");
        assert!(
            result.items.is_empty(),
            "package-only file should have no items"
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn file_with_only_imports_produces_only_relations() {
        let result = parse_go_source(
            r#"package main

import (
    "fmt"
    "os"
)
"#,
        );
        assert!(
            result.items.is_empty(),
            "import-only file should have no items"
        );
        let imports: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert_eq!(imports.len(), 2, "should have 2 import dependencies");
    }

    #[test]
    fn extracts_type_alias() {
        // Go type definition (not Go type alias with `=`, which tree-sitter handles differently)
        let result = parse_go_source("package main\n\ntype ID int\n");
        let aliases: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "type_alias")
            .collect();
        assert_eq!(aliases.len(), 1);
        assert!(aliases[0].qualified_name.contains("ID"));
    }

    #[test]
    fn extracts_interface_with_multiple_methods() {
        let result = parse_go_source(
            r#"package main

type Service interface {
    Start() error
    Stop() error
    Status() string
}
"#,
        );
        let ifaces: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "interface")
            .collect();
        assert_eq!(ifaces.len(), 1);
        assert!(ifaces[0].qualified_name.contains("Service"));
    }

    #[test]
    fn extracts_struct_with_embedded_type() {
        let result = parse_go_source(
            r#"package main

type Base struct {
    ID int
}

type Extended struct {
    Base
    Name string
}
"#,
        );
        let structs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "struct")
            .collect();
        assert_eq!(structs.len(), 2, "should extract both Base and Extended");
    }

    #[test]
    fn extracts_multiple_functions() {
        let result = parse_go_source(
            r#"package main

func foo() {}
func bar() {}
func baz() {}
"#,
        );
        let funcs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert_eq!(funcs.len(), 3);
    }

    #[test]
    fn method_with_value_receiver() {
        let result = parse_go_source(
            "package main\n\ntype Server struct{}\n\nfunc (s Server) Name() string { return \"\" }\n",
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 1);
        assert!(
            methods[0].qualified_name.contains("Server::Name"),
            "should extract method with value receiver, got: {}",
            methods[0].qualified_name
        );
    }

    #[test]
    fn grouped_import_declaration() {
        let result = parse_go_source(
            r#"package main

import (
    "fmt"
    "net/http"
    "os"
)
"#,
        );
        let imports: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert_eq!(imports.len(), 3, "should extract 3 grouped imports");
    }

    #[test]
    fn aliased_import_used_in_call() {
        let result = parse_go_source(
            r#"package main

import h "net/http"

func main() {
    h.ListenAndServe(":8080", nil)
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
                .any(|c| c.target_qualified_name == "net/http::ListenAndServe"),
            "aliased import call should resolve to full path, got: {:?}",
            calls
        );
    }

    #[test]
    fn multiple_methods_on_same_receiver() {
        let result = parse_go_source(
            r#"package main

type DB struct{}

func (d *DB) Open() {}
func (d *DB) Close() {}
func (d *DB) Query() {}
"#,
        );
        let methods: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "method")
            .collect();
        assert_eq!(methods.len(), 3, "should extract 3 methods on DB");
        assert!(methods.iter().all(|m| m.qualified_name.contains("DB")));
    }

    #[test]
    fn nested_call_in_for_loop() {
        let result = parse_go_source(
            r#"package main

func process() {}

func main() {
    for i := 0; i < 10; i++ {
        process()
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
                .any(|c| c.target_qualified_name == "myapp::process"),
            "should find calls inside for loops"
        );
    }

    #[test]
    fn function_metadata_includes_loc() {
        let result = parse_go_source(
            r#"package main

func multi() {
    x := 1
    y := 2
    _ = x + y
}
"#,
        );
        let func = result
            .items
            .iter()
            .find(|i| i.sub_kind == "function")
            .expect("should have a function");
        let loc = func.metadata.as_ref().unwrap()["loc"].as_u64().unwrap();
        assert!(
            loc >= 4,
            "multi-line function should have loc >= 4, got {loc}"
        );
    }

    #[test]
    fn descriptor_has_correct_fields() {
        let desc = GoAnalyzer::descriptor();
        assert_eq!(desc.language_id, "go");
        assert!(desc.manifest_files.contains(&"go.mod".to_string()));
        assert!(desc.source_extensions.contains(&".go".to_string()));
        assert!(desc.skip_directories.contains(&"vendor".to_string()));
        assert_eq!(desc.top_level_kind, NodeKind::Service);
        assert_eq!(desc.top_level_sub_kind, "module");
    }

    #[test]
    fn default_trait_creates_analyzer() {
        let analyzer = GoAnalyzer::default();
        assert_eq!(analyzer.language_id(), "go");
    }

    #[test]
    fn parser_returns_boxed_language_parser() {
        let _parser = GoAnalyzer::parser();
        // Just verify it constructs without panic
    }

    #[test]
    fn single_import_outside_group() {
        let result = parse_go_source("package main\n\nimport \"fmt\"\n");
        let imports: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].target_qualified_name, "fmt");
    }

    #[test]
    fn syntax_error_still_extracts_valid_items() {
        let result = parse_go_source(
            r#"package main

func valid() {}

func broken( {
"#,
        );
        // tree-sitter is error-tolerant; it should still find the valid function
        let funcs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "function")
            .collect();
        assert!(
            !funcs.is_empty(),
            "should extract valid items despite syntax errors"
        );
    }

    #[test]
    fn multiple_type_declarations_in_group() {
        let result = parse_go_source(
            r#"package main

type (
    Request struct {
        URL string
    }
    Response struct {
        Code int
    }
)
"#,
        );
        let structs: Vec<_> = result
            .items
            .iter()
            .filter(|i| i.sub_kind == "struct")
            .collect();
        assert_eq!(
            structs.len(),
            2,
            "should extract both grouped type declarations"
        );
    }

    #[test]
    fn local_variable_method_call() {
        let result = parse_go_source(
            r#"package main

func main() {
    s := Server{}
    s.Start()
}
"#,
        );
        let calls: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Calls)
            .collect();
        assert!(
            calls.iter().any(|c| c.target_qualified_name == "s::Start"),
            "local variable method call should use variable name as operand, got: {:?}",
            calls
        );
    }
}
