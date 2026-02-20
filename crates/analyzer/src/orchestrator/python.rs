//! Python language orchestrator.
//!
//! Discovers Python packages via `pyproject.toml` or `setup.py` and delegates
//! parsing to [`PythonAnalyzer`](crate::languages::python::PythonAnalyzer).

use std::path::Path;

use svt_core::model::NodeKind;

use crate::discovery::discover_python_packages;
use crate::languages::python::PythonAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Python projects.
#[derive(Debug)]
pub struct PythonOrchestrator {
    analyzer: PythonAnalyzer,
}

impl PythonOrchestrator {
    /// Create a new `PythonOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: PythonAnalyzer::new(),
        }
    }
}

impl Default for PythonOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageOrchestrator for PythonOrchestrator {
    fn language_id(&self) -> &str {
        "python"
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_python_packages(root)
            .unwrap_or_default()
            .into_iter()
            .map(|pkg| LanguageUnit {
                name: pkg.name.clone(),
                language: "python".to_string(),
                root: pkg.root.clone(),
                source_root: pkg.source_root.clone(),
                source_files: pkg.source_files.clone(),
                top_level_kind: NodeKind::Service,
                top_level_sub_kind: "package".to_string(),
                source_ref: pkg.root.display().to_string(),
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
    fn python_orchestrator_language_id() {
        let orch = PythonOrchestrator::new();
        assert_eq!(orch.language_id(), "python");
    }
}
