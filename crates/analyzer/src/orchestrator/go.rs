//! Go language orchestrator.
//!
//! Discovers Go modules via `go.mod` and delegates parsing to
//! [`GoAnalyzer`](crate::languages::go::GoAnalyzer).

use std::path::Path;

use svt_core::model::NodeKind;

use crate::discovery::discover_go_packages;
use crate::languages::go::GoAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Go projects.
#[derive(Debug)]
pub struct GoOrchestrator {
    analyzer: GoAnalyzer,
}

impl GoOrchestrator {
    /// Create a new `GoOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: GoAnalyzer::new(),
        }
    }
}

impl Default for GoOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageOrchestrator for GoOrchestrator {
    fn language_id(&self) -> &str {
        "go"
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_go_packages(root)
            .unwrap_or_default()
            .into_iter()
            .map(|pkg| LanguageUnit {
                name: pkg.name.clone(),
                language: "go".to_string(),
                root: pkg.root.clone(),
                source_root: pkg.root.clone(),
                source_files: pkg.source_files.clone(),
                top_level_kind: NodeKind::Service,
                top_level_sub_kind: "module".to_string(),
                source_ref: pkg.root.join("go.mod").display().to_string(),
                parent_qualified_name: None,
            })
            .collect()
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.analyzer.analyze_crate(&unit.name, &file_refs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_orchestrator_language_id() {
        let orch = GoOrchestrator::new();
        assert_eq!(orch.language_id(), "go");
    }
}
