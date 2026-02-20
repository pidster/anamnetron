//! Rust language orchestrator.
//!
//! Discovers Rust crates via `cargo metadata` and delegates parsing to
//! [`RustAnalyzer`](crate::languages::rust::RustAnalyzer). Handles workspace
//! detection and qualified name mapping.

use std::path::Path;

use svt_core::model::NodeKind;

use crate::discovery::discover_project;
use crate::languages::rust::RustAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};
use crate::types::AnalysisItem;

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Rust projects.
#[derive(Debug)]
pub struct RustOrchestrator {
    analyzer: RustAnalyzer,
}

impl RustOrchestrator {
    /// Create a new `RustOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: RustAnalyzer::new(),
        }
    }
}

impl Default for RustOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageOrchestrator for RustOrchestrator {
    fn language_id(&self) -> &str {
        "rust"
    }

    fn extra_items(&self, root: &Path) -> Vec<AnalysisItem> {
        let layout = match discover_project(root) {
            Ok(l) => l,
            Err(_) => return vec![],
        };
        let mut items = Vec::new();
        if let Some(ref ws_name) = layout.workspace_name {
            items.push(AnalysisItem {
                qualified_name: ws_name.replace('-', "_"),
                kind: NodeKind::System,
                sub_kind: "workspace".to_string(),
                parent_qualified_name: None,
                source_ref: layout.workspace_root.display().to_string(),
                language: "rust".to_string(),
            });
        }
        items
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        let layout = match discover_project(root) {
            Ok(l) => l,
            Err(_) => return vec![],
        };

        layout
            .crates
            .iter()
            .map(|c| {
                let qn = workspace_qualified_name(&c.name, layout.workspace_name.as_deref());
                LanguageUnit {
                    name: qn,
                    language: "rust".to_string(),
                    root: c.root.clone(),
                    source_root: c.root.join("src"),
                    source_files: c.source_files.clone(),
                    top_level_kind: NodeKind::Service,
                    top_level_sub_kind: "crate".to_string(),
                    source_ref: c.entry_point.display().to_string(),
                    parent_qualified_name: layout
                        .workspace_name
                        .as_ref()
                        .map(|ws| ws.replace('-', "_")),
                }
            })
            .collect()
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.analyzer.analyze_crate(&unit.name, &file_refs)
    }
}

/// Convert a package name to a qualified name, splitting workspace prefix.
///
/// With workspace `"svt"`, `"svt-core"` becomes `"svt::core"`.
/// Without a workspace prefix, `"svt-core"` becomes `"svt_core"`.
fn workspace_qualified_name(package_name: &str, workspace_name: Option<&str>) -> String {
    if let Some(ws) = workspace_name {
        let prefix = format!("{ws}-");
        if let Some(suffix) = package_name.strip_prefix(&prefix) {
            return format!("{}::{}", ws.replace('-', "_"), suffix.replace('-', "_"));
        }
    }
    package_name.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_orchestrator_language_id() {
        let orch = RustOrchestrator::new();
        assert_eq!(orch.language_id(), "rust");
    }

    #[test]
    fn rust_orchestrator_discovers_workspace_crates() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let orch = RustOrchestrator::new();
        let units = orch.discover(&project_root);
        assert!(
            units.len() >= 4,
            "should discover at least 4 crates, got {}",
            units.len()
        );
        assert!(units.iter().all(|u| u.language == "rust"));
        assert!(units.iter().all(|u| u.top_level_sub_kind == "crate"));
    }

    #[test]
    fn rust_orchestrator_emits_workspace_root_as_extra_item() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let orch = RustOrchestrator::new();
        let extra = orch.extra_items(&project_root);
        assert!(!extra.is_empty(), "should emit workspace root item");
        assert_eq!(extra[0].sub_kind, "workspace");
    }

    #[test]
    fn workspace_qualified_name_splits_prefix() {
        assert_eq!(
            workspace_qualified_name("svt-core", Some("svt")),
            "svt::core"
        );
        assert_eq!(
            workspace_qualified_name("svt-analyzer", Some("svt")),
            "svt::analyzer"
        );
    }

    #[test]
    fn workspace_qualified_name_no_prefix_falls_back() {
        assert_eq!(workspace_qualified_name("svt-core", None), "svt_core");
    }

    #[test]
    fn workspace_qualified_name_non_matching_prefix() {
        assert_eq!(
            workspace_qualified_name("other-crate", Some("svt")),
            "other_crate"
        );
    }
}
