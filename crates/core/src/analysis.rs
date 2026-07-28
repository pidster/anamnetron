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
    /// Classification tags (e.g., "test", "benchmark"). Orthogonal to kind/sub_kind.
    pub tags: Vec<String>,
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

/// Per-shape method-call resolution telemetry.
///
/// Design note (inline design artifact per `.claude/rules/design-first.md`):
/// this counts how many syntactic method calls (`receiver.method()`) a parser
/// could resolve to a concrete target, broken down by receiver shape. It lives
/// on [`ParseResult`] because that is the natural home for parse telemetry — it
/// is produced alongside the items/relations of the same parse and travels with
/// them to the aggregation point. The field is additive and `Default`-derived,
/// so it does not break existing plugin constructions; the change is
/// small-enough (a telemetry struct, not a new mechanism) that an inline design
/// comment is the correct artifact rather than an ADR or design doc.
///
/// # Invariants
///
/// Every method call is counted in **exactly one** bucket, so the partition
/// invariant holds: `resolved() + unresolved() == total()`. Simple function
/// calls (`foo()`) are deliberately **not** counted here — they resolve
/// best-effort almost always and would inflate the resolution ratio
/// dishonestly. The ratio derived from these counts is method-calls-only.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MethodCallStats {
    /// `self.method()` resolved via the enclosing `impl` type.
    pub self_resolved: usize,
    /// `x.method()` resolved via a local-variable type mapping.
    pub local_var_resolved: usize,
    /// Unresolved: receiver is a field access (e.g. `self.field.method()`).
    pub unresolved_field_access: usize,
    /// Unresolved: receiver is a chained call (e.g. `foo().method()`, `.await`).
    pub unresolved_chained: usize,
    /// Unresolved: any other receiver shape.
    pub unresolved_other: usize,
}

impl MethodCallStats {
    /// Total number of method calls that were resolved to a concrete target.
    #[must_use]
    pub fn resolved(&self) -> usize {
        self.self_resolved + self.local_var_resolved
    }

    /// Total number of method calls that could not be resolved.
    #[must_use]
    pub fn unresolved(&self) -> usize {
        self.unresolved_field_access + self.unresolved_chained + self.unresolved_other
    }

    /// Total number of method calls observed (`resolved() + unresolved()`).
    #[must_use]
    pub fn total(&self) -> usize {
        self.resolved() + self.unresolved()
    }

    /// Accumulate another set of counts into this one (bucket-wise).
    pub fn merge(&mut self, other: &Self) {
        self.self_resolved += other.self_resolved;
        self.local_var_resolved += other.local_var_resolved;
        self.unresolved_field_access += other.unresolved_field_access;
        self.unresolved_chained += other.unresolved_chained;
        self.unresolved_other += other.unresolved_other;
    }
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
    /// Per-shape method-call resolution telemetry (see [`MethodCallStats`]).
    ///
    /// Additive telemetry field: parsers that do not resolve method calls leave
    /// this at its `Default` (all-zero) value.
    pub method_call_stats: MethodCallStats,
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

    /// Return names of well-known container/wrapper types for this language.
    ///
    /// When the analyzer encounters `impl WellKnown<InnerType>`, it resolves
    /// methods to `InnerType` rather than creating a phantom node for the
    /// well-known type.
    fn well_known_container_types(&self) -> &[&str] {
        &[]
    }
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
                tags: vec![],
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
            ..Default::default()
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
        assert_eq!(result.method_call_stats, MethodCallStats::default());
    }

    #[test]
    fn method_call_stats_derives_totals_from_buckets() {
        let stats = MethodCallStats {
            self_resolved: 2,
            local_var_resolved: 3,
            unresolved_field_access: 4,
            unresolved_chained: 5,
            unresolved_other: 6,
        };
        assert_eq!(stats.resolved(), 5, "resolved = self + local_var");
        assert_eq!(
            stats.unresolved(),
            15,
            "unresolved = field + chained + other"
        );
        assert_eq!(
            stats.total(),
            20,
            "partition invariant: total = resolved + unresolved"
        );
        assert_eq!(stats.resolved() + stats.unresolved(), stats.total());
    }

    #[test]
    fn method_call_stats_default_is_all_zero() {
        let stats = MethodCallStats::default();
        assert_eq!(stats.resolved(), 0);
        assert_eq!(stats.unresolved(), 0);
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn method_call_stats_merge_is_bucketwise_sum() {
        let mut a = MethodCallStats {
            self_resolved: 1,
            local_var_resolved: 1,
            unresolved_field_access: 1,
            unresolved_chained: 1,
            unresolved_other: 1,
        };
        let b = MethodCallStats {
            self_resolved: 4,
            local_var_resolved: 3,
            unresolved_field_access: 2,
            unresolved_chained: 1,
            unresolved_other: 0,
        };
        a.merge(&b);
        assert_eq!(a.self_resolved, 5);
        assert_eq!(a.local_var_resolved, 4);
        assert_eq!(a.unresolved_field_access, 3);
        assert_eq!(a.unresolved_chained, 2);
        assert_eq!(a.unresolved_other, 1);
        assert_eq!(a.total(), 15);
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
                    tags: vec![],
                }],
                relations: vec![],
                warnings: vec![],
                ..Default::default()
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
