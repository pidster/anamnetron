//! TypeScript language analyzer using tree-sitter-typescript.
//!
//! Extracts exported structural elements (classes, functions, interfaces,
//! type aliases) and import relationships from TypeScript source files.
//! Also handles Svelte files by extracting their `<script>` blocks first.

use std::path::Path;

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

impl LanguageAnalyzer for TypeScriptAnalyzer {
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

/// Parse TypeScript source and extract exported items and import relations.
fn parse_typescript_source(
    parser: &mut tree_sitter::Parser,
    package_name: &str,
    file_path: &Path,
    source: &str,
    line_offset: usize,
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
                    &mut result.items,
                );
            }
            "import_statement" => {
                extract_import(child, source_bytes, &module_context, &mut result.relations);
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
    items: &mut Vec<AnalysisItem>,
) {
    // export_statement can contain: function_declaration, class_declaration,
    // interface_declaration, type_alias_declaration, or lexical_declaration.
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i) else {
            continue;
        };

        let (kind, sub_kind) = match child.kind() {
            "function_declaration" => (NodeKind::Unit, "function"),
            "class_declaration" => (NodeKind::Unit, "class"),
            "interface_declaration" => (NodeKind::Unit, "interface"),
            "type_alias_declaration" => (NodeKind::Unit, "type-alias"),
            _ => continue,
        };

        let Some(name) = child
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
        else {
            continue;
        };

        let line = child.start_position().row + 1 + line_offset;
        let source_ref = format!("{}:{line}", file_path.display());

        items.push(AnalysisItem {
            qualified_name: format!("{module_context}::{name}"),
            kind,
            sub_kind: sub_kind.to_string(),
            parent_qualified_name: Some(module_context.to_string()),
            source_ref,
            language: "typescript".to_string(),
        });
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

    // --- Export extraction tests (Task 4) ---

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
    fn ignores_non_exported_declarations() {
        let result = parse_ts_source(
            "my-app",
            r#"
function privateHelper() {}
const localVar = 42;
class InternalClass {}
export function publicFn() {}
"#,
        );
        assert_eq!(
            result.items.len(),
            1,
            "should only extract the exported function, got: {:?}",
            result.items
        );
        assert_eq!(result.items[0].qualified_name, "my-app::publicFn");
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

    // --- Import edge and Svelte component tests (Task 5) ---

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
}
