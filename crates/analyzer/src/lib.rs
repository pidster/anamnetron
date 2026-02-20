//! `svt-analyzer` -- Tree-sitter based code analysis and structure discovery.
//!
//! This crate scans source code using tree-sitter grammars to extract
//! architectural elements (modules, types, functions, dependencies) and
//! populate the core graph model.
//!
//! Analysis is driven by the [`orchestrator::OrchestratorRegistry`], which
//! collects per-language [`orchestrator::LanguageOrchestrator`] implementations
//! and runs a uniform discover-analyse-postprocess pipeline for each language.

#![warn(missing_docs)]

pub mod discovery;
pub mod languages;
pub mod mapping;
pub mod orchestrator;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

use svt_core::model::SnapshotKind;
use svt_core::store::GraphStore;

use crate::mapping::map_to_graph;
use crate::orchestrator::OrchestratorRegistry;
use crate::types::{AnalysisItem, AnalysisSummary};

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
/// Convenience wrapper around [`analyze_project_with_registry`] that uses
/// the default orchestrator registry (built-in languages only).
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
    let registry = OrchestratorRegistry::with_defaults();
    analyze_project_with_registry(store, project_root, commit_ref, registry)
}

/// Analyze a project using a custom [`OrchestratorRegistry`].
///
/// This is the main entry point for the analysis pipeline. It iterates over
/// all registered language orchestrators, running a uniform
/// discover-analyse-postprocess pipeline for each language. Results are mapped
/// to graph nodes and edges, then batch-inserted into the store.
///
/// Use this when you need to register additional orchestrators (e.g., from
/// plugins) beyond the built-in defaults.
pub fn analyze_project_with_registry(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
    registry: OrchestratorRegistry,
) -> Result<AnalysisSummary, AnalyzerError> {
    if !project_root.is_dir() {
        return Err(AnalyzerError::Discovery(
            crate::discovery::DiscoveryError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("project root does not exist: {}", project_root.display()),
            )),
        ));
    }

    let mut all_items: Vec<AnalysisItem> = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;
    let mut units_per_language: HashMap<String, usize> = HashMap::new();

    for orchestrator in registry.orchestrators() {
        // Phase 1: project-level extra items (e.g., workspace root).
        all_items.extend(orchestrator.extra_items(project_root));

        // Phase 2: discover units.
        let units = orchestrator.discover(project_root);

        for unit in &units {
            // Phase 3: emit top-level item from LanguageUnit fields.
            all_items.push(AnalysisItem {
                qualified_name: unit.name.clone(),
                kind: unit.top_level_kind,
                sub_kind: unit.top_level_sub_kind.clone(),
                parent_qualified_name: unit.parent_qualified_name.clone(),
                source_ref: unit.source_ref.clone(),
                language: unit.language.clone(),
            });

            // Phase 4: emit structural items.
            all_items.extend(orchestrator.emit_structural_items(unit));

            // Phase 5: analyze source files.
            files_analyzed += unit.source_files.len();
            let mut result = orchestrator.analyze(unit);

            // Phase 6: post-process.
            orchestrator.post_process(unit, &mut result);

            all_items.extend(result.items);
            all_relations.extend(result.relations);
            all_warnings.extend(result.warnings);
        }

        *units_per_language
            .entry(orchestrator.language_id().to_string())
            .or_insert(0) += units.len();
    }

    // Map to graph nodes and edges.
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // Create snapshot and insert.
    let version = store.create_snapshot(SnapshotKind::Analysis, commit_ref)?;
    store.add_nodes_batch(version, &nodes)?;
    store.add_edges_batch(version, &edges)?;

    Ok(AnalysisSummary {
        version,
        crates_analyzed: *units_per_language.get("rust").unwrap_or(&0),
        ts_packages_analyzed: *units_per_language.get("typescript").unwrap_or(&0),
        go_packages_analyzed: *units_per_language.get("go").unwrap_or(&0),
        python_packages_analyzed: *units_per_language.get("python").unwrap_or(&0),
        files_analyzed,
        nodes_created: nodes.len(),
        edges_created: edges.len(),
        warnings: all_warnings,
    })
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
