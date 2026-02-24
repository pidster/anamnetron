//! Rust language orchestrator.
//!
//! Discovers Rust crates via `cargo metadata` and delegates parsing to
//! [`RustAnalyzer`](crate::languages::rust::RustAnalyzer). Handles workspace
//! detection and qualified name mapping.

use std::path::Path;
use std::sync::Mutex;

use svt_core::model::{EdgeKind, NodeKind};

use crate::discovery::discover_project;
use crate::languages::rust::RustAnalyzer;
use crate::languages::ParseResult;
use crate::types::{AnalysisItem, AnalysisRelation};

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Rust projects.
#[derive(Debug)]
pub struct RustOrchestrator {
    analyzer: RustAnalyzer,
    /// Cached crate-level dependency pairs (source_qn, target_qn).
    ///
    /// Populated during [`discover()`](LanguageOrchestrator::discover) from
    /// `Cargo.toml` workspace-internal dependencies, then emitted as
    /// `Depends` relations during [`post_process()`](LanguageOrchestrator::post_process).
    crate_deps: Mutex<Vec<(String, String)>>,
}

impl RustOrchestrator {
    /// Create a new `RustOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: RustAnalyzer::new(),
            crate_deps: Mutex::new(Vec::new()),
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
                metadata: None,
                tags: vec![],
            });
        }
        items
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        let layout = match discover_project(root) {
            Ok(l) => l,
            Err(_) => return vec![],
        };

        // Cache crate-level dependency pairs for post_process().
        let mut dep_pairs = Vec::new();
        for c in &layout.crates {
            let source_qn = workspace_qualified_name(&c.name, layout.workspace_name.as_deref());
            for dep_name in &c.workspace_dependencies {
                let target_qn =
                    workspace_qualified_name(dep_name, layout.workspace_name.as_deref());
                dep_pairs.push((source_qn.clone(), target_qn));
            }
        }
        if let Ok(mut deps) = self.crate_deps.lock() {
            *deps = dep_pairs;
        }

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
        self.analyzer
            .analyze_crate_with_root(&unit.name, &file_refs, &unit.source_root)
    }

    fn post_process(&self, unit: &LanguageUnit, result: &mut ParseResult) {
        // Emit crate-level Depends edges for this unit's workspace dependencies.
        if let Ok(deps) = self.crate_deps.lock() {
            for (source_qn, target_qn) in deps.iter() {
                if source_qn == &unit.name {
                    result.relations.push(AnalysisRelation {
                        source_qualified_name: source_qn.clone(),
                        target_qualified_name: target_qn.clone(),
                        kind: EdgeKind::Depends,
                    });
                }
            }
        }
    }
}

/// Convert a package name to a qualified name, splitting workspace prefix.
///
/// With workspace `"svt"`, `"svt-core"` becomes `"svt::core"`.
/// Without a workspace prefix, `"svt-core"` becomes `"svt_core"`.
fn workspace_qualified_name(package_name: &str, workspace_name: Option<&str>) -> String {
    if let Some(ws) = workspace_name {
        // When the crate name exactly matches the workspace name, suffix with "::app"
        // to avoid colliding with the workspace system node's qualified name.
        if package_name == ws {
            return format!("{}::app", ws.replace('-', "_"));
        }
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

    #[test]
    fn workspace_qualified_name_same_as_workspace_avoids_collision() {
        assert_eq!(
            workspace_qualified_name("aeon", Some("aeon")),
            "aeon::app",
            "crate name matching workspace should get '::app' suffix to avoid collision"
        );
    }

    #[test]
    fn rust_orchestrator_emits_crate_dependency_edges() {
        use svt_core::model::EdgeKind;

        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let orch = RustOrchestrator::new();
        let units = orch.discover(&project_root);

        // Find the analyzer unit and run post_process to get dependency edges.
        let analyzer_unit = units
            .iter()
            .find(|u| u.name == "svt::analyzer")
            .expect("should find svt::analyzer unit");

        let mut result = ParseResult::default();
        orch.post_process(analyzer_unit, &mut result);

        let deps: Vec<_> = result
            .relations
            .iter()
            .filter(|r| r.kind == EdgeKind::Depends)
            .collect();
        assert!(
            deps.iter()
                .any(|d| d.source_qualified_name == "svt::analyzer"
                    && d.target_qualified_name == "svt::core"),
            "should emit Depends edge from svt::analyzer to svt::core, got: {:?}",
            deps
        );
    }
}
