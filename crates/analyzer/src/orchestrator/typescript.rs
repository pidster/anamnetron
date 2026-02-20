//! TypeScript/Svelte language orchestrator.
//!
//! Discovers TypeScript packages via `package.json`, emits directory and
//! file-level module items, reparents parsed items to their file modules,
//! and resolves relative import paths.

use std::collections::HashSet;
use std::path::Path;

use svt_core::model::NodeKind;

use crate::discovery::discover_ts_packages;
use crate::languages::typescript::TypeScriptAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};
use crate::types::AnalysisItem;

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for TypeScript/Svelte projects.
#[derive(Debug)]
pub struct TypeScriptOrchestrator {
    analyzer: TypeScriptAnalyzer,
}

impl TypeScriptOrchestrator {
    /// Create a new `TypeScriptOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: TypeScriptAnalyzer::new(),
        }
    }
}

impl Default for TypeScriptOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageOrchestrator for TypeScriptOrchestrator {
    fn language_id(&self) -> &str {
        "typescript"
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_ts_packages(root)
            .unwrap_or_default()
            .into_iter()
            .map(|pkg| LanguageUnit {
                name: pkg.name.clone(),
                language: "typescript".to_string(),
                root: pkg.root.clone(),
                source_root: pkg.source_root.clone(),
                source_files: pkg.source_files.clone(),
                top_level_kind: NodeKind::Service,
                top_level_sub_kind: "package".to_string(),
                source_ref: pkg.root.join("package.json").display().to_string(),
                parent_qualified_name: None,
            })
            .collect()
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.analyzer.analyze_crate(&unit.name, &file_refs)
    }

    fn emit_structural_items(&self, unit: &LanguageUnit) -> Vec<AnalysisItem> {
        emit_ts_module_items(&unit.source_root, &unit.name, &unit.source_files)
    }

    fn post_process(&self, unit: &LanguageUnit, result: &mut ParseResult) {
        // Reparent items to their file-level module qualified names.
        for item in &mut result.items {
            if let Some(file_module_qn) =
                file_to_module_qn(&unit.source_root, &item.source_ref, &unit.name)
            {
                let item_name = item
                    .qualified_name
                    .rsplit("::")
                    .next()
                    .unwrap_or("")
                    .to_string();
                item.qualified_name = format!("{file_module_qn}::{item_name}");
                item.parent_qualified_name = Some(file_module_qn);
            }
        }

        // Resolve relative import paths to qualified names.
        result.relations.retain_mut(|rel| {
            if rel.target_qualified_name.starts_with("./")
                || rel.target_qualified_name.starts_with("../")
            {
                if let Some(resolved) = resolve_ts_import(&rel.target_qualified_name, &unit.name) {
                    rel.target_qualified_name = resolved;
                    true
                } else {
                    false
                }
            } else {
                true
            }
        });
    }
}

/// Emit module items for directories and files in a TypeScript package.
fn emit_ts_module_items(
    source_root: &Path,
    package_name: &str,
    source_files: &[std::path::PathBuf],
) -> Vec<AnalysisItem> {
    let mut items = Vec::new();
    let mut emitted_modules: HashSet<String> = HashSet::new();

    for file in source_files {
        let rel = match file.strip_prefix(source_root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Emit directory modules.
        let mut current_qn = package_name.to_string();
        for component in rel.parent().iter().flat_map(|p| p.components()) {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_str().unwrap_or("");
                let parent_qn = current_qn.clone();
                current_qn = format!("{current_qn}::{name_str}");
                if emitted_modules.insert(current_qn.clone()) {
                    items.push(AnalysisItem {
                        qualified_name: current_qn.clone(),
                        kind: NodeKind::Component,
                        sub_kind: "module".to_string(),
                        parent_qualified_name: Some(parent_qn),
                        source_ref: file.parent().unwrap_or(source_root).display().to_string(),
                        language: "typescript".to_string(),
                    });
                }
            }
        }

        // Emit file-level item.
        let file_stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Skip index/main files — they represent their parent directory.
        if file_stem == "index" || file_stem == "main" {
            continue;
        }

        let file_qn = format!("{current_qn}::{file_stem}");
        let (kind, sub_kind, lang) = if ext == "svelte" {
            (NodeKind::Unit, "component", "svelte")
        } else {
            (NodeKind::Component, "module", "typescript")
        };

        if emitted_modules.insert(file_qn.clone()) {
            items.push(AnalysisItem {
                qualified_name: file_qn,
                kind,
                sub_kind: sub_kind.to_string(),
                parent_qualified_name: Some(current_qn),
                source_ref: file.display().to_string(),
                language: lang.to_string(),
            });
        }
    }

    items
}

/// Map a file path (from source_ref) to its module qualified name.
fn file_to_module_qn(source_root: &Path, source_ref: &str, package_name: &str) -> Option<String> {
    let file_path_str = source_ref
        .rsplit_once(':')
        .map(|(p, _)| p)
        .unwrap_or(source_ref);
    let file_path = Path::new(file_path_str);
    let rel = file_path.strip_prefix(source_root).ok()?;

    let stem = rel.file_stem().and_then(|s| s.to_str())?;
    let mut qn = package_name.to_string();

    for component in rel.parent().iter().flat_map(|p| p.components()) {
        if let std::path::Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                qn = format!("{qn}::{name_str}");
            }
        }
    }

    if stem != "index" && stem != "main" {
        qn = format!("{qn}::{stem}");
    }

    Some(qn)
}

/// Resolve a relative TypeScript import path to a qualified name.
fn resolve_ts_import(import_path: &str, package_name: &str) -> Option<String> {
    let clean = import_path
        .trim_start_matches("./")
        .trim_start_matches("../")
        .trim_end_matches(".ts")
        .trim_end_matches(".tsx")
        .trim_end_matches(".svelte")
        .trim_end_matches(".js");

    if clean.is_empty() {
        return None;
    }

    Some(format!("{package_name}::{}", clean.replace('/', "::")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_orchestrator_language_id() {
        let orch = TypeScriptOrchestrator::new();
        assert_eq!(orch.language_id(), "typescript");
    }

    #[test]
    fn typescript_orchestrator_discovers_packages() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let orch = TypeScriptOrchestrator::new();
        let units = orch.discover(&project_root);
        assert!(!units.is_empty(), "should discover at least 1 TS package");
        assert!(units.iter().all(|u| u.language == "typescript"));
    }

    #[test]
    fn typescript_orchestrator_emits_structural_items() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let orch = TypeScriptOrchestrator::new();
        let units = orch.discover(&project_root);
        assert!(!units.is_empty());
        let items = orch.emit_structural_items(&units[0]);
        assert!(
            !items.is_empty(),
            "should emit structural module items for TS package"
        );
    }
}
