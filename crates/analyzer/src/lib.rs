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
pub mod hashing;
pub mod languages;
pub mod mapping;
pub mod metrics;
pub mod orchestrator;
pub mod types;

use std::collections::HashMap;
use std::path::Path;

use svt_core::model::{SnapshotKind, Version};
use svt_core::store::GraphStore;
use tracing::{debug, info, info_span};

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
    project_id: &str,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
    let registry = OrchestratorRegistry::with_defaults();
    analyze_project_with_registry(store, project_id, project_root, commit_ref, registry)
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
    project_id: &str,
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
        let lang = orchestrator.language_id();
        let _lang_span = info_span!("analyze_language", language = lang).entered();

        // Phase 1: project-level extra items (e.g., workspace root).
        all_items.extend(orchestrator.extra_items(project_root));

        // Phase 2: discover units.
        let units = orchestrator.discover(project_root);
        info!(language = lang, units = units.len(), "discovered units");

        for unit in &units {
            let _unit_span =
                info_span!("analyze_unit", unit = %unit.name, files = unit.source_files.len())
                    .entered();

            // Phase 3: emit top-level item from LanguageUnit fields.
            all_items.push(AnalysisItem {
                qualified_name: unit.name.clone(),
                kind: unit.top_level_kind,
                sub_kind: unit.top_level_sub_kind.clone(),
                parent_qualified_name: unit.parent_qualified_name.clone(),
                source_ref: unit.source_ref.clone(),
                language: unit.language.clone(),
                metadata: None,
                tags: vec![],
            });

            // Phase 4: emit structural items.
            all_items.extend(orchestrator.emit_structural_items(unit));

            // Phase 5: analyze source files.
            files_analyzed += unit.source_files.len();
            debug!(unit = %unit.name, "analyzing source files");
            let mut result = orchestrator.analyze(unit);
            debug!(
                unit = %unit.name,
                items = result.items.len(),
                relations = result.relations.len(),
                "analysis complete"
            );

            // Phase 6: post-process.
            debug!(unit = %unit.name, "post-processing");
            orchestrator.post_process(unit, &mut result);
            debug!(
                unit = %unit.name,
                items = result.items.len(),
                relations = result.relations.len(),
                "post-processing complete"
            );

            all_items.extend(result.items);
            all_relations.extend(result.relations);
            all_warnings.extend(result.warnings);
        }

        *units_per_language
            .entry(orchestrator.language_id().to_string())
            .or_insert(0) += units.len();
    }

    // Phase 6.5: Metric enrichment via rust-code-analysis.
    {
        let _metrics_span = info_span!("enrich_metrics").entered();
        crate::metrics::enrich_metrics(&mut all_items, project_root);
        info!(items = all_items.len(), "metric enrichment complete");
    }

    info!(
        items = all_items.len(),
        relations = all_relations.len(),
        "mapping to graph"
    );
    // Map to graph nodes and edges.
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // Aggregate method call resolution stats from warnings.
    let (method_calls_resolved, method_calls_unresolved) =
        aggregate_method_call_stats(&all_warnings);

    // Create snapshot and insert.
    let version = store.create_snapshot(project_id, SnapshotKind::Analysis, commit_ref)?;
    store.add_nodes_batch(version, &nodes)?;
    store.add_edges_batch(version, &edges)?;

    Ok(AnalysisSummary {
        version,
        crates_analyzed: *units_per_language.get("rust").unwrap_or(&0),
        ts_packages_analyzed: *units_per_language.get("typescript").unwrap_or(&0),
        go_packages_analyzed: *units_per_language.get("go").unwrap_or(&0),
        python_packages_analyzed: *units_per_language.get("python").unwrap_or(&0),
        java_packages_analyzed: *units_per_language.get("java").unwrap_or(&0),
        files_analyzed,
        nodes_created: nodes.len(),
        edges_created: edges.len(),
        warnings: all_warnings,
        incremental: false,
        units_skipped: 0,
        units_reanalyzed: 0,
        nodes_copied: 0,
        edges_copied: 0,
        method_calls_resolved,
        method_calls_unresolved,
    })
}

/// Analyze a project incrementally, reusing results from a previous version.
///
/// Convenience wrapper around [`analyze_project_incremental_with_registry`]
/// that uses the default orchestrator registry.
pub fn analyze_project_incremental(
    store: &mut impl GraphStore,
    project_id: &str,
    project_root: &Path,
    commit_ref: Option<&str>,
    previous_version: Option<Version>,
) -> Result<AnalysisSummary, AnalyzerError> {
    let registry = OrchestratorRegistry::with_defaults();
    analyze_project_incremental_with_registry(
        store,
        project_id,
        project_root,
        commit_ref,
        previous_version,
        registry,
    )
}

/// Analyze a project incrementally using a custom [`OrchestratorRegistry`].
///
/// When `previous_version` is `Some` and that version has a file manifest,
/// only language units with changed files are re-analyzed. Unchanged units
/// have their nodes and edges copied from the previous version.
///
/// Falls back to full analysis when there is no previous version or no
/// file manifest, but still stores a manifest for future incremental runs.
pub fn analyze_project_incremental_with_registry(
    store: &mut impl GraphStore,
    project_id: &str,
    project_root: &Path,
    commit_ref: Option<&str>,
    previous_version: Option<Version>,
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

    // Phase 1: Discover all units across all orchestrators.
    let mut discovered: Vec<(
        &dyn crate::orchestrator::LanguageOrchestrator,
        Vec<crate::orchestrator::LanguageUnit>,
    )> = Vec::new();
    for orchestrator in registry.orchestrators() {
        let units = orchestrator.discover(project_root);
        discovered.push((orchestrator.as_ref(), units));
    }

    // Collect (language_id, &unit) pairs for manifest building.
    let all_units: Vec<(&str, &crate::orchestrator::LanguageUnit)> = discovered
        .iter()
        .flat_map(|(orch, units)| units.iter().map(move |u| (orch.language_id(), u)))
        .collect();

    // Phase 2: Build current file manifest.
    let (current_manifest, hash_warnings) =
        crate::hashing::build_manifest(project_root, &all_units);

    // Phase 3: Determine which units need re-analysis.
    let previous_manifest = match previous_version {
        Some(pv) => store.get_file_manifest(pv)?,
        None => Vec::new(),
    };

    let can_do_incremental = previous_version.is_some() && !previous_manifest.is_empty();
    let changed_unit_names = if can_do_incremental {
        crate::hashing::changed_units(&current_manifest, &previous_manifest)
    } else {
        // All units are "changed" for full analysis
        all_units
            .iter()
            .map(|(_, u)| u.name.clone())
            .collect::<std::collections::HashSet<String>>()
    };

    // Phase 4: Create new snapshot version.
    let version = store.create_snapshot(project_id, SnapshotKind::Analysis, commit_ref)?;

    // Phase 5: Copy all nodes and edges from previous version (if incremental).
    let mut nodes_copied = 0;
    let mut edges_copied = 0;
    if can_do_incremental {
        if let Some(pv) = previous_version {
            nodes_copied = store.copy_nodes(pv, version)?;
            edges_copied = store.copy_edges(pv, version)?;
        }
    }

    // Phase 6: Run analysis pipeline (only changed units get full analysis).
    let mut all_items: Vec<AnalysisItem> = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;
    let mut units_per_language: HashMap<String, usize> = HashMap::new();
    let mut units_skipped = 0;
    let mut units_reanalyzed = 0;

    all_warnings.extend(
        hash_warnings
            .into_iter()
            .map(|msg| crate::types::AnalysisWarning {
                source_ref: String::new(),
                message: msg,
            }),
    );

    for (orchestrator, units) in &discovered {
        let lang = orchestrator.language_id();
        let _lang_span = info_span!("analyze_language", language = lang).entered();

        // Project-level extra items (always emitted).
        all_items.extend(orchestrator.extra_items(project_root));

        for unit in units {
            // Always emit the top-level item and structural items.
            all_items.push(AnalysisItem {
                qualified_name: unit.name.clone(),
                kind: unit.top_level_kind,
                sub_kind: unit.top_level_sub_kind.clone(),
                parent_qualified_name: unit.parent_qualified_name.clone(),
                source_ref: unit.source_ref.clone(),
                language: unit.language.clone(),
                metadata: None,
                tags: vec![],
            });
            all_items.extend(orchestrator.emit_structural_items(unit));

            if changed_unit_names.contains(&unit.name) {
                let _unit_span =
                    info_span!("analyze_unit", unit = %unit.name, files = unit.source_files.len())
                        .entered();
                // Changed unit: full analysis.
                files_analyzed += unit.source_files.len();
                debug!(unit = %unit.name, "analyzing source files");
                let mut result = orchestrator.analyze(unit);
                debug!(
                    unit = %unit.name,
                    items = result.items.len(),
                    relations = result.relations.len(),
                    "analysis complete"
                );
                debug!(unit = %unit.name, "post-processing");
                orchestrator.post_process(unit, &mut result);
                debug!(
                    unit = %unit.name,
                    items = result.items.len(),
                    relations = result.relations.len(),
                    "post-processing complete"
                );
                all_items.extend(result.items);
                all_relations.extend(result.relations);
                all_warnings.extend(result.warnings);
                units_reanalyzed += 1;
            } else {
                debug!(unit = %unit.name, "unchanged, skipping");
                // Unchanged unit: skip analysis (nodes/edges already copied).
                units_skipped += 1;
            }
        }

        *units_per_language
            .entry(orchestrator.language_id().to_string())
            .or_insert(0) += units.len();
    }

    // Phase 6.5: Metric enrichment via rust-code-analysis.
    {
        let _metrics_span = info_span!("enrich_metrics").entered();
        crate::metrics::enrich_metrics(&mut all_items, project_root);
        info!(items = all_items.len(), "metric enrichment complete");
    }

    // Phase 7: Map to graph and upsert (overwrites copied data for changed units).
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // Aggregate method call resolution stats from warnings.
    let (method_calls_resolved, method_calls_unresolved) =
        aggregate_method_call_stats(&all_warnings);

    store.add_nodes_batch(version, &nodes)?;
    store.add_edges_batch(version, &edges)?;

    // Phase 8: Store file manifest for future incremental runs.
    store.add_file_manifest(version, &current_manifest)?;

    Ok(AnalysisSummary {
        version,
        crates_analyzed: *units_per_language.get("rust").unwrap_or(&0),
        ts_packages_analyzed: *units_per_language.get("typescript").unwrap_or(&0),
        go_packages_analyzed: *units_per_language.get("go").unwrap_or(&0),
        python_packages_analyzed: *units_per_language.get("python").unwrap_or(&0),
        java_packages_analyzed: *units_per_language.get("java").unwrap_or(&0),
        files_analyzed,
        nodes_created: nodes.len(),
        edges_created: edges.len(),
        warnings: all_warnings,
        incremental: can_do_incremental,
        units_skipped,
        units_reanalyzed,
        nodes_copied,
        edges_copied,
        method_calls_resolved,
        method_calls_unresolved,
    })
}

/// Aggregate method call resolution stats from analysis warnings.
///
/// Parses warning messages with the format:
/// `"N method call(s): M resolved, K could not be resolved without type information"`
fn aggregate_method_call_stats(warnings: &[crate::types::AnalysisWarning]) -> (usize, usize) {
    let mut resolved = 0;
    let mut unresolved = 0;

    for w in warnings {
        if w.message.contains("method call(s):") {
            // Extract "M resolved" count
            if let Some(r_pos) = w.message.find(" resolved") {
                let before = &w.message[..r_pos];
                if let Some(space) = before.rfind(' ') {
                    if let Ok(n) = before[space + 1..].parse::<usize>() {
                        resolved += n;
                    }
                } else if let Ok(n) = before.parse::<usize>() {
                    resolved += n;
                }
            }
            // Extract "K could not be resolved" count
            if let Some(u_pos) = w.message.find(" could not be resolved") {
                let before = &w.message[..u_pos];
                if let Some(comma) = before.rfind(", ") {
                    if let Ok(n) = before[comma + 2..].trim().parse::<usize>() {
                        unresolved += n;
                    }
                }
            }
        }
    }

    (resolved, unresolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use svt_core::model::DEFAULT_PROJECT_ID;
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
        let summary = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root, None).unwrap();

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
        let summary = analyze_project(
            &mut store,
            DEFAULT_PROJECT_ID,
            &project_root,
            Some("abc123"),
        )
        .unwrap();

        assert!(summary.version > 0);
    }

    #[test]
    fn incremental_analysis_falls_back_when_no_previous() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let mut store = CozoStore::new_in_memory().unwrap();
        let summary =
            analyze_project_incremental(&mut store, DEFAULT_PROJECT_ID, &project_root, None, None)
                .unwrap();

        // Should do full analysis (no previous version)
        assert!(!summary.incremental);
        assert!(summary.nodes_created > 0);
        assert_eq!(summary.nodes_copied, 0);
        assert_eq!(summary.units_skipped, 0);
    }

    #[test]
    fn incremental_analysis_stores_file_manifest() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let mut store = CozoStore::new_in_memory().unwrap();
        let summary =
            analyze_project_incremental(&mut store, DEFAULT_PROJECT_ID, &project_root, None, None)
                .unwrap();

        let manifest = store.get_file_manifest(summary.version).unwrap();
        assert!(
            !manifest.is_empty(),
            "manifest should be stored after analysis"
        );
        assert!(
            manifest.iter().any(|e| e.language == "rust"),
            "manifest should contain rust entries"
        );
    }

    #[test]
    fn analyze_project_rejects_nonexistent_root() {
        let project_root = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let mut store = CozoStore::new_in_memory().unwrap();
        let err = analyze_project(&mut store, DEFAULT_PROJECT_ID, &project_root, None).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("does not exist"),
            "error should mention missing root, got: {msg}"
        );
    }

    #[test]
    fn analyzer_error_discovery_variant_displays_correctly() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test error");
        let discovery_err = crate::discovery::DiscoveryError::Io(io_err);
        let err = AnalyzerError::Discovery(discovery_err);
        let msg = format!("{err}");
        assert!(
            msg.contains("discovery error"),
            "AnalyzerError::Discovery should display 'discovery error', got: {msg}"
        );
        assert!(
            msg.contains("test error"),
            "should contain inner error message, got: {msg}"
        );
    }

    #[test]
    fn aggregate_method_call_stats_with_method_call_warnings() {
        let warnings = vec![
            crate::types::AnalysisWarning {
                source_ref: "test.rs".to_string(),
                message: "10 method call(s): 7 resolved, 3 could not be resolved without type information".to_string(),
            },
            crate::types::AnalysisWarning {
                source_ref: "other.rs".to_string(),
                message: "5 method call(s): 2 resolved, 3 could not be resolved without type information".to_string(),
            },
        ];
        let (resolved, unresolved) = aggregate_method_call_stats(&warnings);
        assert_eq!(resolved, 9, "should sum resolved counts across warnings");
        assert_eq!(
            unresolved, 6,
            "should sum unresolved counts across warnings"
        );
    }

    #[test]
    fn aggregate_method_call_stats_with_no_method_call_warnings() {
        let warnings = vec![crate::types::AnalysisWarning {
            source_ref: "test.rs".to_string(),
            message: "some other warning".to_string(),
        }];
        let (resolved, unresolved) = aggregate_method_call_stats(&warnings);
        assert_eq!(resolved, 0);
        assert_eq!(unresolved, 0);
    }

    #[test]
    fn aggregate_method_call_stats_with_empty_warnings() {
        let (resolved, unresolved) = aggregate_method_call_stats(&[]);
        assert_eq!(resolved, 0);
        assert_eq!(unresolved, 0);
    }

    #[test]
    fn aggregate_method_call_stats_with_mixed_warnings() {
        let warnings = vec![
            crate::types::AnalysisWarning {
                source_ref: "a.rs".to_string(),
                message: "unrelated warning about something".to_string(),
            },
            crate::types::AnalysisWarning {
                source_ref: "b.rs".to_string(),
                message:
                    "8 method call(s): 5 resolved, 3 could not be resolved without type information"
                        .to_string(),
            },
            crate::types::AnalysisWarning {
                source_ref: "c.rs".to_string(),
                message: "another unrelated warning".to_string(),
            },
        ];
        let (resolved, unresolved) = aggregate_method_call_stats(&warnings);
        assert_eq!(resolved, 5, "should only count from method call warnings");
        assert_eq!(unresolved, 3);
    }

    #[test]
    fn aggregate_method_call_stats_resolved_only() {
        let warnings = vec![crate::types::AnalysisWarning {
            source_ref: "test.rs".to_string(),
            message: "4 method call(s): 4 resolved".to_string(),
        }];
        let (resolved, unresolved) = aggregate_method_call_stats(&warnings);
        assert_eq!(resolved, 4);
        assert_eq!(
            unresolved, 0,
            "no 'could not be resolved' means 0 unresolved"
        );
    }

    #[test]
    fn incremental_analysis_rejects_nonexistent_root() {
        let project_root = PathBuf::from("/nonexistent/path");
        let mut store = CozoStore::new_in_memory().unwrap();
        let err =
            analyze_project_incremental(&mut store, DEFAULT_PROJECT_ID, &project_root, None, None)
                .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("does not exist"),
            "incremental should reject nonexistent root, got: {msg}"
        );
    }

    #[test]
    fn incremental_analysis_skips_unchanged_units() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let mut store = CozoStore::new_in_memory().unwrap();

        // First run: full analysis (stores manifest)
        let first =
            analyze_project_incremental(&mut store, DEFAULT_PROJECT_ID, &project_root, None, None)
                .unwrap();
        assert!(!first.incremental);

        // Second run: incremental (no files changed)
        let second = analyze_project_incremental(
            &mut store,
            DEFAULT_PROJECT_ID,
            &project_root,
            None,
            Some(first.version),
        )
        .unwrap();

        assert!(second.incremental, "second run should be incremental");
        assert!(
            second.units_skipped > 0,
            "some units should be skipped (nothing changed)"
        );
        assert!(
            second.nodes_copied > 0,
            "nodes should be copied from previous"
        );
        assert!(
            second.edges_copied > 0,
            "edges should be copied from previous"
        );
        assert!(
            second.nodes_created > 0,
            "structural nodes should still be created"
        );
    }
}
