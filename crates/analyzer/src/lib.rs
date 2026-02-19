//! `svt-analyzer` -- Tree-sitter based code analysis and structure discovery.
//!
//! This crate scans source code using tree-sitter grammars to extract
//! architectural elements (modules, types, functions, dependencies) and
//! populate the core graph model.

#![warn(missing_docs)]

pub mod discovery;
pub mod languages;
pub mod mapping;
pub mod types;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use svt_core::model::{NodeKind, SnapshotKind};
use svt_core::store::GraphStore;

use crate::discovery::{discover_project, discover_ts_packages};
use crate::languages::rust::RustAnalyzer;
use crate::languages::typescript::TypeScriptAnalyzer;
use crate::languages::LanguageAnalyzer;
use crate::mapping::map_to_graph;
use crate::types::{AnalysisItem, AnalysisSummary, TsPackageInfo};

/// Errors during project analysis.
#[derive(Debug, thiserror::Error)]
pub enum AnalyzerError {
    /// Project discovery failed.
    #[error("discovery error: {0}")]
    Discovery(#[from] crate::discovery::DiscoveryError),
    /// Graph store error.
    #[error("store error: {0}")]
    Store(#[from] svt_core::store::StoreError),
}

/// Analyze a project and populate an analysis snapshot in the store.
///
/// Discovers Rust crates via `cargo metadata` and TypeScript packages via
/// `package.json`, parses source files with tree-sitter, maps qualified names
/// to canonical paths, and batch-inserts into the store.
///
/// Returns a summary of what was analyzed and any warnings encountered.
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
    let mut all_items = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;

    // Phase 1: Rust analysis
    let layout = discover_project(project_root)?;
    let rust_analyzer = RustAnalyzer::new();

    for crate_info in &layout.crates {
        all_items.push(AnalysisItem {
            qualified_name: crate_info.name.replace('-', "_"),
            kind: NodeKind::Service,
            sub_kind: "crate".to_string(),
            parent_qualified_name: None,
            source_ref: crate_info.entry_point.display().to_string(),
            language: "rust".to_string(),
        });

        let file_refs: Vec<&Path> = crate_info
            .source_files
            .iter()
            .map(|p| p.as_path())
            .collect();
        files_analyzed += file_refs.len();

        let parse_result =
            rust_analyzer.analyze_crate(&crate_info.name.replace('-', "_"), &file_refs);
        all_items.extend(parse_result.items);
        all_relations.extend(parse_result.relations);
        all_warnings.extend(parse_result.warnings);
    }

    // Phase 2: TypeScript/Svelte analysis
    let ts_packages = discover_ts_packages(project_root).unwrap_or_default();
    let ts_analyzer = TypeScriptAnalyzer::new();
    let mut ts_packages_analyzed = 0;

    for package in &ts_packages {
        // Emit package-level item
        all_items.push(AnalysisItem {
            qualified_name: package.name.clone(),
            kind: NodeKind::Service,
            sub_kind: "package".to_string(),
            parent_qualified_name: None,
            source_ref: package.root.join("package.json").display().to_string(),
            language: "typescript".to_string(),
        });

        // Emit directory and file-level module items
        emit_ts_module_items(
            &package.source_root,
            &package.name,
            &package.source_files,
            &mut all_items,
        );

        let file_refs: Vec<&Path> = package.source_files.iter().map(|p| p.as_path()).collect();
        files_analyzed += file_refs.len();

        let parse_result = ts_analyzer.analyze_crate(&package.name, &file_refs);

        // Reparent items to their file-level module qualified names
        for mut item in parse_result.items {
            if let Some(file_module_qn) =
                file_to_module_qn(&package.source_root, &item.source_ref, &package.name)
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
            all_items.push(item);
        }

        // Resolve relative import paths to qualified names
        for mut rel in parse_result.relations {
            if rel.target_qualified_name.starts_with("./")
                || rel.target_qualified_name.starts_with("../")
            {
                if let Some(resolved) = resolve_ts_import(&rel.target_qualified_name, package) {
                    rel.target_qualified_name = resolved;
                    all_relations.push(rel);
                }
            } else {
                all_relations.push(rel);
            }
        }

        all_warnings.extend(parse_result.warnings);
        ts_packages_analyzed += 1;
    }

    // Phase 3: Map to graph nodes and edges
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // Phase 4: Create snapshot and insert
    let version = store.create_snapshot(SnapshotKind::Analysis, commit_ref)?;
    store.add_nodes_batch(version, &nodes)?;
    store.add_edges_batch(version, &edges)?;

    Ok(AnalysisSummary {
        version,
        crates_analyzed: layout.crates.len(),
        ts_packages_analyzed,
        files_analyzed,
        nodes_created: nodes.len(),
        edges_created: edges.len(),
        warnings: all_warnings,
    })
}

/// Emit module items for directories and files in a TypeScript package.
fn emit_ts_module_items(
    source_root: &Path,
    package_name: &str,
    source_files: &[PathBuf],
    items: &mut Vec<AnalysisItem>,
) {
    let mut emitted_modules: HashSet<String> = HashSet::new();

    for file in source_files {
        let rel = match file.strip_prefix(source_root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Emit directory modules
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

        // Emit file-level item
        let file_stem = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Skip index/main files — they represent their parent directory
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
fn resolve_ts_import(import_path: &str, package: &TsPackageInfo) -> Option<String> {
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

    Some(format!("{}::{}", package.name, clean.replace('/', "::")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use svt_core::store::CozoStore;

    #[test]
    fn analyze_project_creates_analysis_snapshot() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let mut store = CozoStore::new_in_memory().unwrap();
        let summary = analyze_project(&mut store, &project_root, None).unwrap();

        assert!(summary.version > 0);
        assert!(summary.crates_analyzed >= 4);
        assert!(summary.nodes_created > 0);
        assert!(summary.edges_created > 0);
    }

    #[test]
    fn analyze_project_with_commit_ref() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let mut store = CozoStore::new_in_memory().unwrap();
        let summary = analyze_project(&mut store, &project_root, Some("abc123")).unwrap();

        assert!(summary.version > 0);
    }
}
