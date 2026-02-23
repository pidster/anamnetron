//! Analysis pipeline types shared between svt-core and svt-analyzer.
//!
//! These types are the interchange format between language parsers (which
//! produce them) and the mapping/insertion pipeline (which consumes them).
//! They live in svt-core so that plugin authors can implement
//! [`LanguageParser`] without depending on svt-analyzer.

use std::path::{Path, PathBuf};

use crate::model::{EdgeKind, NodeKind};

/// A code element extracted by static analysis.
#[derive(Debug, Clone)]
pub struct AnalysisItem {
    /// Language-specific qualified name (e.g., "svt_core::model::Node").
    pub qualified_name: String,
    /// Abstraction level.
    pub kind: NodeKind,
    /// Language-specific type (e.g., "crate", "module", "struct", "function").
    pub sub_kind: String,
    /// Qualified name of the containment parent, if any.
    pub parent_qualified_name: Option<String>,
    /// Source file and line reference (e.g., "crates/core/src/model/mod.rs:42").
    pub source_ref: String,
    /// Source language.
    pub language: String,
    /// Extensible metadata from analysis (e.g., LOC, metrics).
    pub metadata: Option<serde_json::Value>,
}

/// A relationship between code elements.
#[derive(Debug, Clone)]
pub struct AnalysisRelation {
    /// Qualified name of the source element.
    pub source_qualified_name: String,
    /// Qualified name of the target element.
    pub target_qualified_name: String,
    /// Relationship type.
    pub kind: EdgeKind,
}

/// A non-fatal warning from analysis.
#[derive(Debug, Clone)]
pub struct AnalysisWarning {
    /// Source file and line where the issue was found.
    pub source_ref: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Result of parsing source files for a single language unit.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    /// Extracted code elements.
    pub items: Vec<AnalysisItem>,
    /// Extracted relationships between elements.
    pub relations: Vec<AnalysisRelation>,
    /// Warnings from parsing (non-fatal).
    pub warnings: Vec<AnalysisWarning>,
}

/// Describes how to discover project units for a language.
///
/// The host uses this to walk the project directory, find manifest files,
/// derive package names, and collect source files — without the plugin
/// needing to implement any discovery logic.
#[derive(Debug, Clone)]
pub struct LanguageDescriptor {
    /// Unique language identifier (e.g., "rust", "go", "java").
    pub language_id: String,
    /// Manifest filenames that indicate a project unit
    /// (e.g., `["go.mod"]`, `["package.json"]`, `["pyproject.toml", "setup.py"]`).
    pub manifest_files: Vec<String>,
    /// Source file extensions to collect (e.g., `[".go"]`, `[".py"]`).
    pub source_extensions: Vec<String>,
    /// Directories to skip during walking (e.g., `["vendor", "node_modules"]`).
    pub skip_directories: Vec<String>,
    /// The [`NodeKind`] for top-level units (typically `NodeKind::Service`).
    pub top_level_kind: NodeKind,
    /// Sub-kind label for top-level units (e.g., "module", "package", "crate").
    pub top_level_sub_kind: String,
}

/// Trait for parsing source files into analysis items and relations.
///
/// Plugin authors implement this to add support for a new language.
/// The host handles discovery, file walking, and orchestration —
/// the parser only needs to extract structure from source code.
pub trait LanguageParser: Send + Sync {
    /// Parse source files for a single project unit.
    ///
    /// `unit_name` is the package/module name derived from the manifest.
    /// `files` are all source files collected by the host based on the descriptor.
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult;

    /// Emit additional structural items beyond what parsing finds.
    ///
    /// For example, TypeScript emits directory-based module nodes.
    /// Default: no additional items.
    fn emit_structural_items(
        &self,
        _source_root: &Path,
        _unit_name: &str,
        _source_files: &[PathBuf],
    ) -> Vec<AnalysisItem> {
        vec![]
    }

    /// Post-process parse results (e.g., reparenting items, resolving imports).
    ///
    /// Default: no post-processing.
    fn post_process(&self, _source_root: &Path, _unit_name: &str, _result: &mut ParseResult) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, NodeKind};

    #[test]
    fn parse_result_collects_items_relations_warnings() {
        let result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "my_crate::Foo".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "struct".to_string(),
                parent_qualified_name: Some("my_crate".to_string()),
                source_ref: "src/lib.rs:10".to_string(),
                language: "rust".to_string(),
                metadata: None,
            }],
            relations: vec![AnalysisRelation {
                source_qualified_name: "my_crate::Foo".to_string(),
                target_qualified_name: "my_crate::Bar".to_string(),
                kind: EdgeKind::Depends,
            }],
            warnings: vec![AnalysisWarning {
                source_ref: "src/lib.rs:20".to_string(),
                message: "unresolved import".to_string(),
            }],
        };
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qualified_name, "my_crate::Foo");
        assert_eq!(result.relations.len(), 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn parse_result_default_is_empty() {
        let result = ParseResult::default();
        assert!(result.items.is_empty());
        assert!(result.relations.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn language_descriptor_fields_accessible() {
        let desc = LanguageDescriptor {
            language_id: "java".to_string(),
            manifest_files: vec!["pom.xml".to_string()],
            source_extensions: vec![".java".to_string()],
            skip_directories: vec!["target".to_string(), ".git".to_string()],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        };
        assert_eq!(desc.language_id, "java");
        assert_eq!(desc.manifest_files, vec!["pom.xml"]);
        assert_eq!(desc.source_extensions, vec![".java"]);
    }

    /// A mock parser for testing the LanguageParser trait.
    struct MockParser;

    impl LanguageParser for MockParser {
        fn parse(&self, unit_name: &str, _files: &[&Path]) -> ParseResult {
            ParseResult {
                items: vec![AnalysisItem {
                    qualified_name: format!("{unit_name}::Main"),
                    kind: NodeKind::Unit,
                    sub_kind: "class".to_string(),
                    parent_qualified_name: Some(unit_name.to_string()),
                    source_ref: "src/Main.java:1".to_string(),
                    language: "java".to_string(),
                    metadata: None,
                }],
                relations: vec![],
                warnings: vec![],
            }
        }
    }

    #[test]
    fn mock_parser_returns_items() {
        let parser = MockParser;
        let result = parser.parse("my-app", &[]);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qualified_name, "my-app::Main");
    }

    #[test]
    fn language_parser_default_hooks_are_noops() {
        let parser = MockParser;
        let root = Path::new("/tmp");
        assert!(parser.emit_structural_items(root, "pkg", &[]).is_empty());
        let mut result = ParseResult::default();
        parser.post_process(root, "pkg", &mut result);
        assert!(result.items.is_empty());
    }
}
