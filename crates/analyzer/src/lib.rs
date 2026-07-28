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
pub mod type_flow;
pub mod type_metadata;
pub mod types;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

/// A single source directory to analyze, with optional project-level exclusions.
///
/// Multiple sources are combined into **one** analysis snapshot (see the
/// project config design, Decision 5).
#[derive(Debug, Clone)]
pub struct AnalysisSource {
    /// Directory to analyze.
    pub root: PathBuf,
    /// Directories to exclude, relative to [`root`](AnalysisSource::root).
    ///
    /// Each entry is matched as a leading path-component prefix of a file's
    /// path relative to `root`. `"vendor"` and `"vendor/"` both exclude
    /// `<root>/vendor/**`; `"a/b"` excludes `<root>/a/b/**`. These are layered
    /// on top of each language's built-in skip list, not a replacement for it.
    pub exclude: Vec<String>,
}

impl AnalysisSource {
    /// Create a source with no exclusions.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            exclude: Vec::new(),
        }
    }

    /// Create a source with the given exclusions.
    #[must_use]
    pub fn with_excludes(root: impl Into<PathBuf>, exclude: Vec<String>) -> Self {
        Self {
            root: root.into(),
            exclude,
        }
    }
}

/// Split an exclude pattern into normalized path components.
///
/// Returns `None` for entries that are empty after normalization.
fn exclude_components(pattern: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = pattern
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

/// Return true if `path` lies under one of the `exclude` patterns, which are
/// interpreted relative to `root`.
fn is_excluded(path: &Path, root: &Path, exclude: &[String]) -> bool {
    if exclude.is_empty() {
        return false;
    }
    let relative = match path.strip_prefix(root) {
        Ok(r) => r,
        // Paths outside the source root are not covered by its excludes.
        Err(_) => return false,
    };
    let components: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    exclude
        .iter()
        .filter_map(|p| exclude_components(p))
        .any(|pattern| {
            components.len() >= pattern.len() && components[..pattern.len()] == pattern[..]
        })
}

/// Apply a source's excludes to a discovered unit.
///
/// Returns `None` when the unit itself is excluded, or when every one of its
/// source files was excluded (an emptied unit would otherwise produce a
/// top-level node for code that was explicitly opted out of analysis).
fn apply_excludes(
    mut unit: crate::orchestrator::LanguageUnit,
    root: &Path,
    exclude: &[String],
) -> Option<crate::orchestrator::LanguageUnit> {
    if exclude.is_empty() {
        return Some(unit);
    }
    if is_excluded(&unit.root, root, exclude) || is_excluded(&unit.source_root, root, exclude) {
        return None;
    }
    let had_files = !unit.source_files.is_empty();
    unit.source_files.retain(|f| !is_excluded(f, root, exclude));
    if had_files && unit.source_files.is_empty() {
        return None;
    }
    Some(unit)
}

/// Discover language units across every source, applying per-source excludes.
///
/// Units are deduplicated by (unit name, unit root) so that overlapping or
/// nested source entries do not analyze the same unit twice.
fn discover_all<'a>(
    registry: &'a OrchestratorRegistry,
    sources: &[AnalysisSource],
) -> Vec<(
    &'a dyn crate::orchestrator::LanguageOrchestrator,
    Vec<crate::orchestrator::LanguageUnit>,
)> {
    let mut discovered = Vec::new();
    for orchestrator in registry.orchestrators() {
        let mut seen: std::collections::HashSet<(String, PathBuf)> =
            std::collections::HashSet::new();
        let mut units = Vec::new();
        for source in sources {
            for unit in orchestrator.discover(&source.root) {
                let Some(unit) = apply_excludes(unit, &source.root, &source.exclude) else {
                    continue;
                };
                if seen.insert((unit.name.clone(), unit.root.clone())) {
                    units.push(unit);
                }
            }
        }
        info!(
            language = orchestrator.language_id(),
            units = units.len(),
            "discovered units"
        );
        discovered.push((orchestrator.as_ref(), units));
    }
    discovered
}

/// Verify every source root exists and is a directory.
fn check_sources(sources: &[AnalysisSource]) -> Result<(), AnalyzerError> {
    for source in sources {
        if !source.root.is_dir() {
            return Err(AnalyzerError::Discovery(
                crate::discovery::DiscoveryError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("project root does not exist: {}", source.root.display()),
                )),
            ));
        }
    }
    Ok(())
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
    analyze_sources_with_registry(
        store,
        project_id,
        project_root,
        &[AnalysisSource::new(project_root)],
        commit_ref,
        registry,
    )
}

/// Analyze one or more source directories into a **single** analysis snapshot.
///
/// Every source is discovered (honouring its `exclude` list), and all resulting
/// items and relations are combined before mapping to the graph, so a project
/// configured with several `sources` entries yields one coherent snapshot with
/// cross-source edges intact.
///
/// `project_root` is the project directory that anchors metric enrichment; each
/// source root is normally a subdirectory of it.
pub fn analyze_sources_with_registry(
    store: &mut impl GraphStore,
    project_id: &str,
    project_root: &Path,
    sources: &[AnalysisSource],
    commit_ref: Option<&str>,
    registry: OrchestratorRegistry,
) -> Result<AnalysisSummary, AnalyzerError> {
    check_sources(sources)?;

    let mut all_items: Vec<AnalysisItem> = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;
    let mut units_per_language: HashMap<String, usize> = HashMap::new();
    let mut method_call_stats = svt_core::analysis::MethodCallStats::default();

    let discovered = discover_all(&registry, sources);

    for (orchestrator, units) in &discovered {
        let lang = orchestrator.language_id();
        let _lang_span = info_span!("analyze_language", language = lang).entered();

        // Phase 1: project-level extra items (e.g., workspace root), per source.
        // Overlapping sources can yield the same item, so deduplicate by name.
        let mut seen_extra: std::collections::HashSet<String> = std::collections::HashSet::new();
        for source in sources {
            for item in orchestrator.extra_items(&source.root) {
                if seen_extra.insert(item.qualified_name.clone()) {
                    all_items.push(item);
                }
            }
        }

        for unit in units {
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

            method_call_stats.merge(&result.method_call_stats);
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

    // Phase 6.6: Type-flow analysis — infer transforms and data_flow edges.
    {
        let _flow_span = info_span!("type_flow_analysis").entered();
        let combined = svt_core::analysis::ParseResult {
            items: all_items.clone(),
            relations: all_relations.clone(),
            warnings: vec![],
            ..Default::default()
        };
        let type_flow = crate::type_flow::TypeFlowAnalysis::from_parse_results(&[combined]);
        let flow_relations = type_flow.analyze();
        debug!(
            flow_edges = flow_relations.len(),
            signatures = type_flow.signature_count(),
            "type-flow analysis complete"
        );
        all_relations.extend(flow_relations);
    }

    info!(
        items = all_items.len(),
        relations = all_relations.len(),
        "mapping to graph"
    );
    // Map to graph nodes and edges.
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // Method-call resolution stats, carried as typed per-shape counters.
    let method_calls_resolved = method_call_stats.resolved();
    let method_calls_unresolved = method_call_stats.unresolved();

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
        method_call_stats,
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
    analyze_sources_incremental_with_registry(
        store,
        project_id,
        project_root,
        &[AnalysisSource::new(project_root)],
        commit_ref,
        previous_version,
        registry,
    )
}

/// Incrementally analyze one or more source directories into a single snapshot.
///
/// The multi-source counterpart to
/// [`analyze_project_incremental_with_registry`]. Excludes are applied during
/// discovery, before the file manifest is built, so excluded files never enter
/// the manifest and therefore never register as spurious changes.
///
/// The manifest is anchored at `project_root` (not per source), so paths remain
/// stable across runs even if the configured source list changes.
pub fn analyze_sources_incremental_with_registry(
    store: &mut impl GraphStore,
    project_id: &str,
    project_root: &Path,
    sources: &[AnalysisSource],
    commit_ref: Option<&str>,
    previous_version: Option<Version>,
    registry: OrchestratorRegistry,
) -> Result<AnalysisSummary, AnalyzerError> {
    check_sources(sources)?;

    // Phase 1: Discover all units across all orchestrators and sources.
    let discovered = discover_all(&registry, sources);

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
    // Only re-analyzed units contribute stats; copied (unchanged) units do not,
    // matching the pre-existing behavior when stats travelled as warnings.
    let mut method_call_stats = svt_core::analysis::MethodCallStats::default();

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

        // Project-level extra items (always emitted), per source, deduplicated
        // so overlapping sources do not emit the same item twice.
        let mut seen_extra: std::collections::HashSet<String> = std::collections::HashSet::new();
        for source in sources {
            for item in orchestrator.extra_items(&source.root) {
                if seen_extra.insert(item.qualified_name.clone()) {
                    all_items.push(item);
                }
            }
        }

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
                method_call_stats.merge(&result.method_call_stats);
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

    // Phase 6.6: Type-flow analysis — infer transforms and data_flow edges.
    {
        let _flow_span = info_span!("type_flow_analysis").entered();
        let combined = svt_core::analysis::ParseResult {
            items: all_items.clone(),
            relations: all_relations.clone(),
            warnings: vec![],
            ..Default::default()
        };
        let type_flow = crate::type_flow::TypeFlowAnalysis::from_parse_results(&[combined]);
        let flow_relations = type_flow.analyze();
        debug!(
            flow_edges = flow_relations.len(),
            "type-flow analysis complete"
        );
        all_relations.extend(flow_relations);
    }

    // Phase 7: Map to graph and upsert (overwrites copied data for changed units).
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // Method-call resolution stats, carried as typed per-shape counters.
    let method_calls_resolved = method_call_stats.resolved();
    let method_calls_unresolved = method_call_stats.unresolved();

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
        method_call_stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use svt_core::model::DEFAULT_PROJECT_ID;
    use svt_core::store::CozoStore;

    #[test]
    fn empty_exclude_list_excludes_nothing() {
        let root = Path::new("/proj");
        assert!(!is_excluded(Path::new("/proj/vendor/a.rs"), root, &[]));
    }

    #[test]
    fn exclude_matches_directory_prefix_relative_to_source_root() {
        let root = Path::new("/proj");
        let exclude = vec!["vendor".to_string()];
        assert!(is_excluded(Path::new("/proj/vendor/a.rs"), root, &exclude));
        assert!(is_excluded(
            Path::new("/proj/vendor/deep/b.rs"),
            root,
            &exclude
        ));
        assert!(!is_excluded(Path::new("/proj/src/a.rs"), root, &exclude));
    }

    #[test]
    fn exclude_tolerates_trailing_and_leading_slashes() {
        let root = Path::new("/proj");
        for pattern in ["vendor/", "/vendor", "./vendor/"] {
            assert!(
                is_excluded(Path::new("/proj/vendor/a.rs"), root, &[pattern.to_string()]),
                "pattern {pattern} should match"
            );
        }
    }

    #[test]
    fn patterns_that_normalize_to_nothing_exclude_nothing() {
        let root = Path::new("/proj");
        for pattern in ["", "/", ".", "./", "//"] {
            assert!(
                exclude_components(pattern).is_none(),
                "pattern {pattern:?} should normalize to nothing"
            );
            assert!(
                !is_excluded(Path::new("/proj/src/a.rs"), root, &[pattern.to_string()]),
                "pattern {pattern:?} must not exclude anything"
            );
        }
    }

    #[test]
    fn exclude_matches_multi_component_paths() {
        let root = Path::new("/proj");
        let exclude = vec!["third_party/generated".to_string()];
        assert!(is_excluded(
            Path::new("/proj/third_party/generated/a.rs"),
            root,
            &exclude
        ));
        assert!(!is_excluded(
            Path::new("/proj/third_party/kept/a.rs"),
            root,
            &exclude
        ));
    }

    #[test]
    fn exclude_does_not_match_nested_occurrence_of_pattern() {
        // Excludes anchor at the source root, so a nested `vendor/` is kept.
        let root = Path::new("/proj");
        let exclude = vec!["vendor".to_string()];
        assert!(!is_excluded(
            Path::new("/proj/src/vendor/a.rs"),
            root,
            &exclude
        ));
    }

    #[test]
    fn exclude_does_not_match_partial_component_name() {
        let root = Path::new("/proj");
        let exclude = vec!["vend".to_string()];
        assert!(!is_excluded(Path::new("/proj/vendor/a.rs"), root, &exclude));
    }

    #[test]
    fn paths_outside_the_source_root_are_not_excluded() {
        let root = Path::new("/proj");
        let exclude = vec!["vendor".to_string()];
        assert!(!is_excluded(
            Path::new("/other/vendor/a.rs"),
            root,
            &exclude
        ));
    }

    /// Build a minimal unit for exclude-filter tests.
    fn test_unit(root: &str, files: &[&str]) -> crate::orchestrator::LanguageUnit {
        crate::orchestrator::LanguageUnit {
            name: "unit".to_string(),
            language: "rust".to_string(),
            root: PathBuf::from(root),
            source_root: PathBuf::from(root),
            source_files: files.iter().map(PathBuf::from).collect(),
            top_level_kind: svt_core::model::NodeKind::Component,
            top_level_sub_kind: "crate".to_string(),
            source_ref: format!("{root}/Cargo.toml"),
            parent_qualified_name: None,
            workspace_dependencies: vec![],
        }
    }

    #[test]
    fn excluded_source_files_are_dropped_from_a_unit() {
        let unit = test_unit("/proj/a", &["/proj/a/src/x.rs", "/proj/vendor/y.rs"]);
        let filtered = apply_excludes(unit, Path::new("/proj"), &["vendor".to_string()])
            .expect("unit retained");
        assert_eq!(
            filtered.source_files,
            vec![PathBuf::from("/proj/a/src/x.rs")]
        );
    }

    #[test]
    fn unit_rooted_in_an_excluded_directory_is_dropped_entirely() {
        let unit = test_unit("/proj/vendor/lib", &["/proj/vendor/lib/x.rs"]);
        assert!(apply_excludes(unit, Path::new("/proj"), &["vendor".to_string()]).is_none());
    }

    #[test]
    fn unit_with_all_source_files_excluded_is_dropped() {
        // The unit root itself is not excluded, but every file under it is.
        let unit = test_unit("/proj/a", &["/proj/gen/x.rs", "/proj/gen/y.rs"]);
        assert!(apply_excludes(unit, Path::new("/proj"), &["gen".to_string()]).is_none());
    }

    #[test]
    fn unit_is_unchanged_when_no_excludes_are_configured() {
        let unit = test_unit("/proj/a", &["/proj/a/src/x.rs"]);
        let filtered = apply_excludes(unit, Path::new("/proj"), &[]).expect("unit retained");
        assert_eq!(filtered.source_files.len(), 1);
    }

    #[test]
    fn analysis_source_constructors_set_root_and_excludes() {
        let plain = AnalysisSource::new("/proj");
        assert_eq!(plain.root, PathBuf::from("/proj"));
        assert!(plain.exclude.is_empty());

        let with_ex = AnalysisSource::with_excludes("/proj", vec!["vendor".to_string()]);
        assert_eq!(with_ex.exclude, vec!["vendor".to_string()]);
    }

    #[test]
    fn analyzing_a_nonexistent_source_returns_an_error() {
        let mut store = CozoStore::new_in_memory().expect("store");
        let err = analyze_sources_with_registry(
            &mut store,
            DEFAULT_PROJECT_ID,
            Path::new("/definitely/not/here"),
            &[AnalysisSource::new("/definitely/not/here")],
            None,
            OrchestratorRegistry::with_defaults(),
        )
        .expect_err("expected error");
        assert!(
            err.to_string().contains("does not exist"),
            "unexpected error: {err}"
        );
    }

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
