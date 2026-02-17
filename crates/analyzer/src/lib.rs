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

use std::path::Path;

use svt_core::model::{NodeKind, SnapshotKind};
use svt_core::store::GraphStore;

use crate::discovery::discover_project;
use crate::languages::rust::RustAnalyzer;
use crate::languages::LanguageAnalyzer;
use crate::mapping::map_to_graph;
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

/// Analyze a Rust project and populate an analysis snapshot in the store.
///
/// Discovers crates via `cargo metadata`, parses source files with tree-sitter,
/// maps qualified names to canonical paths, and batch-inserts into the store.
///
/// Returns a summary of what was analyzed and any warnings encountered.
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
    // 1. Discover project layout
    let layout = discover_project(project_root)?;

    // 2. Parse each crate
    let analyzer = RustAnalyzer::new();
    let mut all_items = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;

    for crate_info in &layout.crates {
        // Emit crate-level item (tree-sitter doesn't emit these)
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

        let parse_result = analyzer.analyze_crate(&crate_info.name.replace('-', "_"), &file_refs);
        all_items.extend(parse_result.items);
        all_relations.extend(parse_result.relations);
        all_warnings.extend(parse_result.warnings);
    }

    // 3. Map to graph nodes and edges
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // 4. Create snapshot and insert
    let version = store.create_snapshot(SnapshotKind::Analysis, commit_ref)?;
    store.add_nodes_batch(version, &nodes)?;
    store.add_edges_batch(version, &edges)?;

    Ok(AnalysisSummary {
        version,
        crates_analyzed: layout.crates.len(),
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
