//! Language-specific analysis drivers.

pub mod rust;

use std::path::Path;

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

/// Result of parsing a set of source files for a single crate.
#[derive(Debug, Default)]
pub struct ParseResult {
    /// Extracted code elements.
    pub items: Vec<AnalysisItem>,
    /// Extracted relationships between elements.
    pub relations: Vec<AnalysisRelation>,
    /// Warnings from parsing (non-fatal).
    pub warnings: Vec<AnalysisWarning>,
}

/// A language-specific source code analyzer.
pub trait LanguageAnalyzer {
    /// Parse a set of source files for a crate and return extracted items and relations.
    ///
    /// `crate_name` is the Rust crate name (e.g., "svt_core").
    /// `files` are the `.rs` source files to parse.
    fn analyze_crate(&self, crate_name: &str, files: &[&Path]) -> ParseResult;
}
