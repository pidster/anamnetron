//! Language orchestrators that bundle discovery, analysis, and post-processing.
//!
//! Each language implements [`LanguageOrchestrator`] to participate in the
//! analysis pipeline. The [`OrchestratorRegistry`] collects all orchestrators
//! and drives the pipeline loop in [`crate::analyze_project`].

pub mod descriptor;
pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;

use std::path::Path;

use svt_core::model::NodeKind;

use crate::languages::ParseResult;
use crate::types::AnalysisItem;

/// A discovered package, module, or crate for any language.
///
/// Produced by [`LanguageOrchestrator::discover`] and consumed by the
/// analysis pipeline to emit top-level nodes and drive parsing.
#[derive(Debug, Clone)]
pub struct LanguageUnit {
    /// Package/crate/module name (used as the qualified name for the top-level node).
    pub name: String,
    /// Language identifier (e.g., "rust", "go").
    pub language: String,
    /// Root directory of the unit.
    pub root: std::path::PathBuf,
    /// Source root directory (e.g., `root/src/`).
    pub source_root: std::path::PathBuf,
    /// All source files to analyze.
    pub source_files: Vec<std::path::PathBuf>,
    /// [`NodeKind`] for the top-level item.
    pub top_level_kind: NodeKind,
    /// Sub-kind for the top-level item (e.g., "crate", "package", "module").
    pub top_level_sub_kind: String,
    /// Source reference for the top-level item (e.g., path to entry point).
    pub source_ref: String,
    /// Parent qualified name for the top-level item (e.g., workspace name for crates).
    pub parent_qualified_name: Option<String>,
}

/// Orchestrates discovery, analysis, and post-processing for a language.
///
/// Each language implements this trait to participate in the analysis pipeline.
/// The pipeline calls methods in order:
/// 1. [`extra_items`](LanguageOrchestrator::extra_items) — project-level items (e.g., workspace root)
/// 2. [`discover`](LanguageOrchestrator::discover) — find packages/crates/modules
/// 3. For each discovered unit:
///    a. Emit a top-level node from [`LanguageUnit`] fields (done by the pipeline)
///    b. [`emit_structural_items`](LanguageOrchestrator::emit_structural_items) — additional structure
///    c. [`analyze`](LanguageOrchestrator::analyze) — parse source files
///    d. [`post_process`](LanguageOrchestrator::post_process) — language-specific fixups
pub trait LanguageOrchestrator: Send + Sync {
    /// Unique language identifier.
    fn language_id(&self) -> &str;

    /// Discover packages/crates/modules in the project root.
    fn discover(&self, root: &Path) -> Vec<LanguageUnit>;

    /// Analyze source files for a single unit.
    fn analyze(&self, unit: &LanguageUnit) -> ParseResult;

    /// Emit project-level items not tied to any single unit (e.g., workspace root).
    ///
    /// Default: no extra items.
    fn extra_items(&self, _root: &Path) -> Vec<AnalysisItem> {
        vec![]
    }

    /// Emit additional structural items (e.g., directory/file modules for TypeScript).
    ///
    /// Default: no additional items.
    fn emit_structural_items(&self, _unit: &LanguageUnit) -> Vec<AnalysisItem> {
        vec![]
    }

    /// Post-process analysis results (e.g., reparent items, resolve imports).
    ///
    /// Default: no post-processing.
    fn post_process(&self, _unit: &LanguageUnit, _result: &mut ParseResult) {}
}

/// Registry of language orchestrators for the analysis pipeline.
pub struct OrchestratorRegistry {
    orchestrators: Vec<Box<dyn LanguageOrchestrator>>,
}

impl OrchestratorRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            orchestrators: Vec::new(),
        }
    }

    /// Create a registry with all built-in orchestrators pre-registered.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(rust::RustOrchestrator::new()));
        registry.register(Box::new(typescript::TypeScriptOrchestrator::new()));
        registry.register(Box::new(go::orchestrator()));
        registry.register(Box::new(python::orchestrator()));
        registry
    }

    /// Register an orchestrator.
    pub fn register(&mut self, orchestrator: Box<dyn LanguageOrchestrator>) {
        self.orchestrators.push(orchestrator);
    }

    /// Get all registered orchestrators.
    #[must_use]
    pub fn orchestrators(&self) -> &[Box<dyn LanguageOrchestrator>] {
        &self.orchestrators
    }
}

impl Default for OrchestratorRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn language_unit_has_required_fields() {
        let unit = LanguageUnit {
            name: "test-pkg".to_string(),
            language: "test".to_string(),
            root: PathBuf::from("/tmp"),
            source_root: PathBuf::from("/tmp/src"),
            source_files: vec![PathBuf::from("/tmp/src/main.rs")],
            top_level_kind: svt_core::model::NodeKind::Service,
            top_level_sub_kind: "crate".to_string(),
            source_ref: "/tmp/src/main.rs".to_string(),
            parent_qualified_name: None,
        };
        assert_eq!(unit.name, "test-pkg");
        assert_eq!(unit.language, "test");
        assert_eq!(unit.source_files.len(), 1);
    }

    #[test]
    fn orchestrator_registry_with_defaults_has_all_languages() {
        let registry = OrchestratorRegistry::with_defaults();
        let mut ids: Vec<&str> = registry
            .orchestrators()
            .iter()
            .map(|o| o.language_id())
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["go", "python", "rust", "typescript"]);
    }

    #[test]
    fn orchestrator_registry_register_adds_orchestrator() {
        let mut registry = OrchestratorRegistry::new();
        assert!(registry.orchestrators().is_empty());
        registry.register(Box::new(rust::RustOrchestrator::new()));
        assert_eq!(registry.orchestrators().len(), 1);
    }
}
