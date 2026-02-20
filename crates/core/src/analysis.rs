//! Analysis pipeline types shared between svt-core and svt-analyzer.
//!
//! These types are the interchange format between language parsers (which
//! produce them) and the mapping/insertion pipeline (which consumes them).
//! They live in svt-core so that plugin authors can implement
//! [`LanguageParser`] without depending on svt-analyzer.

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
}
