//! Rust language orchestrator.
//!
//! Discovers Rust crates via `cargo metadata` and delegates parsing to
//! [`RustAnalyzer`](crate::languages::rust::RustAnalyzer). Handles workspace
//! detection and qualified name mapping.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use svt_core::model::{EdgeKind, NodeKind};
use tracing::{debug, info};

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
    /// Mapping from raw Rust crate identifiers (e.g., `aeon_consensus`) to
    /// workspace-qualified names (e.g., `aeon::consensus`).
    ///
    /// Only contains entries where the two differ. Used in
    /// [`post_process()`](LanguageOrchestrator::post_process) to rewrite
    /// cross-crate qualified names that use the raw crate identifier.
    crate_name_map: Mutex<HashMap<String, String>>,
    /// Workspace-wide type registry: short name → list of qualified names.
    ///
    /// Accumulated across units during [`post_process()`] so that later
    /// units can reparent orphaned impl items to types defined in earlier
    /// (dependency) crates. Since `cargo metadata` typically lists crates in
    /// topological order, dependencies are processed before dependents.
    workspace_types: Mutex<HashMap<String, Vec<String>>>,
}

impl RustOrchestrator {
    /// Create a new `RustOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: RustAnalyzer::new(),
            crate_deps: Mutex::new(Vec::new()),
            crate_name_map: Mutex::new(HashMap::new()),
            workspace_types: Mutex::new(HashMap::new()),
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

        // Cache crate-level dependency pairs and crate name mappings for post_process().
        let mut dep_pairs = Vec::new();
        let mut name_map = HashMap::new();
        for c in &layout.crates {
            let source_qn = workspace_qualified_name(&c.name, layout.workspace_name.as_deref());
            for dep_name in &c.workspace_dependencies {
                let target_qn =
                    workspace_qualified_name(dep_name, layout.workspace_name.as_deref());
                dep_pairs.push((source_qn.clone(), target_qn));
            }
            // Map raw crate identifier → workspace-qualified name, where they differ.
            // Skip entries where the raw name equals the workspace name (e.g., crate
            // "aeon" in workspace "aeon" produces raw_name "aeon"). That name is a
            // prefix of every other crate's workspace-qualified name (aeon::core,
            // aeon::consensus, etc.), so adding it to the map would incorrectly
            // rewrite all of them.
            let raw_name = c.name.replace('-', "_");
            let is_workspace_prefix = layout
                .workspace_name
                .as_deref()
                .is_some_and(|ws| raw_name == ws.replace('-', "_"));
            if raw_name != source_qn && !is_workspace_prefix {
                name_map.insert(raw_name, source_qn.clone());
            }
        }
        if let Ok(mut deps) = self.crate_deps.lock() {
            *deps = dep_pairs;
        }
        if let Ok(mut map) = self.crate_name_map.lock() {
            if !name_map.is_empty() {
                debug!(
                    mappings = ?name_map,
                    "crate name map for cross-crate prefix rewriting"
                );
            }
            *map = name_map;
        }

        info!(
            workspace = ?layout.workspace_name,
            crates = layout.crates.len(),
            "discovered Rust project"
        );

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
                    workspace_dependencies: vec![],
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

        // Step 2: Rewrite cross-crate qualified names that use raw crate
        // identifiers (e.g., `aeon_consensus::...`) to their workspace-qualified
        // form (e.g., `aeon::consensus::...`).
        if let Ok(map) = self.crate_name_map.lock() {
            if !map.is_empty() {
                rewrite_cross_crate_prefixes(result, &map);
            }
        }

        // Step 3: Reparent orphaned impl methods whose parent type doesn't
        // exist in the result but has a unique match by short name elsewhere
        // in the same crate or in previously-processed workspace crates.
        // Dependency information is used to disambiguate when multiple
        // cross-crate candidates exist.
        {
            let cross_crate_types = self.workspace_types.lock().ok();
            let crate_deps = self.crate_deps.lock().ok();
            reparent_orphaned_impl_items(
                result,
                &unit.name,
                cross_crate_types.as_deref(),
                crate_deps.as_deref(),
            );
        } // drop the locks before re-acquiring workspace_types below.

        // Register this unit's types into the workspace-wide registry so
        // later units can resolve cross-crate references.
        if let Ok(mut ws_types) = self.workspace_types.lock() {
            for item in &result.items {
                if matches!(
                    item.sub_kind.as_str(),
                    "struct" | "enum" | "trait" | "type_alias"
                ) {
                    if let Some(short) = item.qualified_name.rsplit("::").next() {
                        ws_types
                            .entry(short.to_string())
                            .or_default()
                            .push(item.qualified_name.clone());
                    }
                }
            }
        }
    }
}

/// Rewrite qualified names that start with a raw crate prefix to use the
/// workspace-qualified form instead.
///
/// For example, if `map` contains `aeon_consensus → aeon::consensus`, then
/// `aeon_consensus::multi_raft::errors::RaftError` becomes
/// `aeon::consensus::multi_raft::errors::RaftError`.
fn rewrite_cross_crate_prefixes(result: &mut ParseResult, map: &HashMap<String, String>) {
    /// Replace a raw crate prefix in `qn` with its workspace-qualified form.
    fn rewrite(qn: &str, map: &HashMap<String, String>) -> Option<String> {
        for (raw, ws_qn) in map {
            if let Some(suffix) = qn.strip_prefix(raw.as_str()) {
                if suffix.is_empty() || suffix.starts_with("::") {
                    return Some(format!("{ws_qn}{suffix}"));
                }
            }
        }
        None
    }

    for item in &mut result.items {
        if let Some(rewritten) = rewrite(&item.qualified_name, map) {
            item.qualified_name = rewritten;
        }
        if let Some(ref parent) = item.parent_qualified_name {
            if let Some(rewritten) = rewrite(parent, map) {
                item.parent_qualified_name = Some(rewritten);
            }
        }
    }
    for rel in &mut result.relations {
        if let Some(rewritten) = rewrite(&rel.source_qualified_name, map) {
            rel.source_qualified_name = rewritten;
        }
        if let Some(rewritten) = rewrite(&rel.target_qualified_name, map) {
            rel.target_qualified_name = rewritten;
        }
    }
}

/// Reparent items whose `parent_qualified_name` points to a non-existent node.
///
/// When a glob import (`use types::*`) is used, the analyzer can't resolve
/// individual type names to their definition location. This creates items
/// parented under the importing module instead of the defining module.
///
/// This function finds such orphans and, if exactly one item with the same
/// short name exists in the same crate (or across the workspace via
/// `cross_crate_types`), reparents the orphan to match. When multiple
/// cross-crate candidates exist, dependency information (`crate_deps`) is
/// used to prefer types from direct dependencies of the current crate.
fn reparent_orphaned_impl_items(
    result: &mut ParseResult,
    crate_name: &str,
    cross_crate_types: Option<&HashMap<String, Vec<String>>>,
    crate_deps: Option<&Vec<(String, String)>>,
) {
    // Build a set of all known qualified names.
    let known_qns: HashSet<&str> = result
        .items
        .iter()
        .map(|i| i.qualified_name.as_str())
        .collect();

    // Build a map from short name → list of qualified names for items that
    // could be parent types (structs, enums, traits, type aliases).
    let mut short_name_to_qns: HashMap<&str, Vec<&str>> = HashMap::new();
    for item in &result.items {
        if matches!(
            item.sub_kind.as_str(),
            "struct" | "enum" | "trait" | "type_alias"
        ) {
            if let Some(short) = item.qualified_name.rsplit("::").next() {
                short_name_to_qns
                    .entry(short)
                    .or_default()
                    .push(&item.qualified_name);
            }
        }
    }

    // Collect reparenting operations: (old_parent_qn, new_parent_qn).
    let mut reparent_map: HashMap<String, String> = HashMap::new();

    for item in &result.items {
        let parent = match &item.parent_qualified_name {
            Some(p) => p,
            None => continue,
        };

        // Skip if the parent already exists as a known item or is the crate root.
        if known_qns.contains(parent.as_str()) || parent == crate_name {
            continue;
        }

        // Extract the type name (last segment).
        let type_name = match parent.rsplit("::").next() {
            Some(n) => n,
            None => continue,
        };

        let crate_prefix = format!("{crate_name}::");
        let is_local_parent = parent.starts_with(&crate_prefix);

        // First, look for exactly one match within the same crate.
        if is_local_parent {
            if let Some(candidates) = short_name_to_qns.get(type_name) {
                let in_crate: Vec<&&str> = candidates
                    .iter()
                    .filter(|qn| qn.starts_with(&crate_prefix))
                    .collect();
                if in_crate.len() == 1 {
                    reparent_map.insert(parent.clone(), (*in_crate[0]).to_string());
                    continue;
                }
            }
        }

        // Check cross-crate types from earlier units. This handles both:
        // (a) local parents with no within-crate match
        // (b) parents in other workspace crate namespaces (e.g., re-exported types
        //     imported via `use other_crate::module::Type`)
        if let Some(ws_types) = cross_crate_types {
            if let Some(candidates) = ws_types.get(type_name) {
                if candidates.len() == 1 {
                    reparent_map.insert(parent.clone(), candidates[0].clone());
                } else if candidates.len() > 1 {
                    debug!(
                        type_name = %type_name,
                        candidates = ?candidates,
                        crate_name = %crate_name,
                        parent = %parent,
                        "multiple cross-crate candidates, attempting disambiguation"
                    );
                    let mut resolved = false;

                    // Strategy 1: Use dependency info to prefer types from
                    // direct dependencies of this crate.
                    if !resolved {
                        if let Some(deps) = crate_deps {
                            let dep_crates: HashSet<&str> = deps
                                .iter()
                                .filter(|(src, _)| src == crate_name)
                                .map(|(_, tgt)| tgt.as_str())
                                .collect();
                            let from_deps: Vec<&String> = candidates
                                .iter()
                                .filter(|qn| {
                                    dep_crates.iter().any(|dep| {
                                        qn.starts_with(dep)
                                            && (qn.len() == dep.len()
                                                || qn[dep.len()..].starts_with("::"))
                                    })
                                })
                                .collect();
                            if from_deps.len() == 1 {
                                reparent_map.insert(parent.clone(), from_deps[0].clone());
                                resolved = true;
                            }
                        }
                    }

                    // Strategy 2: Match on module path suffix. When the orphan
                    // parent is e.g. `aeon::client::protocol::Record` (from a
                    // re-export chain), progressively shorter path suffixes are
                    // tried against candidates. For example:
                    //   parent = `aeon::client::protocol::Record`
                    //   try `client::protocol::Record` → no unique match
                    //   try `protocol::Record` → matches `aeon::protocol::Record`
                    if !resolved {
                        let segments: Vec<&str> = parent.split("::").collect();
                        // Try progressively shorter suffixes, starting from
                        // dropping the first segment (crate prefix) and going
                        // shorter. Stop at 2 segments (module::Type minimum).
                        for start in 1..segments.len().saturating_sub(1) {
                            let suffix = segments[start..].join("::");
                            let suffix_with_sep = format!("::{suffix}");
                            let by_suffix: Vec<&String> = candidates
                                .iter()
                                .filter(|qn| qn.ends_with(&suffix_with_sep) || *qn == &suffix)
                                .collect();
                            if by_suffix.len() == 1 {
                                debug!(
                                    suffix = %suffix,
                                    matched = %by_suffix[0],
                                    "disambiguated by path suffix"
                                );
                                reparent_map.insert(parent.clone(), by_suffix[0].clone());
                                resolved = true;
                                break;
                            }
                        }
                    }

                    if !resolved {
                        debug!(
                            type_name = %type_name,
                            "could not disambiguate, leaving as-is"
                        );
                    }
                }
            }
        }
    }

    // Apply reparenting.
    if reparent_map.is_empty() {
        return;
    }

    for (old_parent, new_parent) in &reparent_map {
        debug!(
            from = %old_parent,
            to = %new_parent,
            "reparenting orphaned impl items"
        );
    }

    for item in &mut result.items {
        if let Some(ref parent) = item.parent_qualified_name {
            if let Some(new_parent) = reparent_map.get(parent.as_str()) {
                // Rewrite this item's qualified name: replace the old parent prefix.
                if let Some(suffix) = item.qualified_name.strip_prefix(parent.as_str()) {
                    item.qualified_name = format!("{new_parent}{suffix}");
                }
                item.parent_qualified_name = Some(new_parent.clone());
            }
        }
    }

    // Also update relations that reference old qualified names.
    for rel in &mut result.relations {
        for (old_prefix, new_prefix) in &reparent_map {
            if let Some(suffix) = rel.source_qualified_name.strip_prefix(old_prefix.as_str()) {
                if suffix.is_empty() || suffix.starts_with("::") {
                    rel.source_qualified_name = format!("{new_prefix}{suffix}");
                }
            }
            if let Some(suffix) = rel.target_qualified_name.strip_prefix(old_prefix.as_str()) {
                if suffix.is_empty() || suffix.starts_with("::") {
                    rel.target_qualified_name = format!("{new_prefix}{suffix}");
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

    #[test]
    fn cross_crate_qualified_names_normalized_in_post_process() {
        let mut map = HashMap::new();
        map.insert("aeon_consensus".to_string(), "aeon::consensus".to_string());
        map.insert("aeon_client".to_string(), "aeon::client".to_string());

        let mut result = ParseResult {
            items: vec![
                AnalysisItem {
                    qualified_name: "aeon_consensus::multi_raft::errors::RaftError".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "struct".to_string(),
                    parent_qualified_name: Some("aeon_consensus::multi_raft::errors".to_string()),
                    source_ref: "src/lib.rs:10".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
                AnalysisItem {
                    qualified_name: "aeon_client::protocol::record".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "module".to_string(),
                    parent_qualified_name: Some("aeon_client::protocol".to_string()),
                    source_ref: "src/protocol.rs:1".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
                // An item that should NOT be rewritten (no matching prefix).
                AnalysisItem {
                    qualified_name: "aeon::core::config::Config".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "struct".to_string(),
                    parent_qualified_name: Some("aeon::core::config".to_string()),
                    source_ref: "src/config.rs:5".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
            ],
            relations: vec![AnalysisRelation {
                source_qualified_name: "aeon::core::errors::from_impl".to_string(),
                target_qualified_name: "aeon_consensus::multi_raft::errors::RaftError".to_string(),
                kind: EdgeKind::Depends,
            }],
            warnings: vec![],
        };

        rewrite_cross_crate_prefixes(&mut result, &map);

        assert_eq!(
            result.items[0].qualified_name, "aeon::consensus::multi_raft::errors::RaftError",
            "should rewrite aeon_consensus prefix to aeon::consensus"
        );
        assert_eq!(
            result.items[0].parent_qualified_name.as_deref(),
            Some("aeon::consensus::multi_raft::errors")
        );
        assert_eq!(
            result.items[1].qualified_name, "aeon::client::protocol::record",
            "should rewrite aeon_client prefix to aeon::client"
        );
        assert_eq!(
            result.items[1].parent_qualified_name.as_deref(),
            Some("aeon::client::protocol")
        );
        // Unchanged item.
        assert_eq!(
            result.items[2].qualified_name, "aeon::core::config::Config",
            "should not rewrite items that don't start with a raw crate prefix"
        );
        // Relation target rewritten.
        assert_eq!(
            result.relations[0].target_qualified_name,
            "aeon::consensus::multi_raft::errors::RaftError"
        );
        // Relation source unchanged.
        assert_eq!(
            result.relations[0].source_qualified_name,
            "aeon::core::errors::from_impl"
        );
    }

    #[test]
    fn post_process_reparents_orphaned_impl_methods() {
        let crate_name = "aeon::rest";

        // Scenario: QueryResults is defined in handlers::types but an impl
        // method is parented under handlers::data::QueryResults (due to glob import).
        let mut result = ParseResult {
            items: vec![
                // The real type definition.
                AnalysisItem {
                    qualified_name: "aeon::rest::handlers::types::QueryResults".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "struct".to_string(),
                    parent_qualified_name: Some("aeon::rest::handlers::types".to_string()),
                    source_ref: "src/handlers/types.rs:10".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
                // The orphaned method with wrong parent.
                AnalysisItem {
                    qualified_name: "aeon::rest::handlers::data::QueryResults::new".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "function".to_string(),
                    parent_qualified_name: Some(
                        "aeon::rest::handlers::data::QueryResults".to_string(),
                    ),
                    source_ref: "src/handlers/data.rs:20".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
                // A module node (this should be in known_qns but not as a type).
                AnalysisItem {
                    qualified_name: "aeon::rest::handlers::data".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "module".to_string(),
                    parent_qualified_name: Some("aeon::rest::handlers".to_string()),
                    source_ref: "src/handlers/data.rs:1".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
            ],
            relations: vec![AnalysisRelation {
                source_qualified_name: "aeon::rest::handlers::data::QueryResults::new".to_string(),
                target_qualified_name: "aeon::rest::handlers::data::QueryResults".to_string(),
                kind: EdgeKind::Contains,
            }],
            warnings: vec![],
        };

        reparent_orphaned_impl_items(&mut result, crate_name, None, None);

        // The method should now be reparented.
        assert_eq!(
            result.items[1].qualified_name, "aeon::rest::handlers::types::QueryResults::new",
            "orphaned method should be reparented to the correct type"
        );
        assert_eq!(
            result.items[1].parent_qualified_name.as_deref(),
            Some("aeon::rest::handlers::types::QueryResults"),
        );

        // The real type should be unchanged.
        assert_eq!(
            result.items[0].qualified_name,
            "aeon::rest::handlers::types::QueryResults"
        );

        // The relation should also be updated.
        assert_eq!(
            result.relations[0].source_qualified_name,
            "aeon::rest::handlers::types::QueryResults::new"
        );
        assert_eq!(
            result.relations[0].target_qualified_name,
            "aeon::rest::handlers::types::QueryResults"
        );
    }

    #[test]
    fn rewrite_does_not_apply_workspace_name_as_prefix() {
        // When crate name == workspace name (e.g., "aeon" in workspace "aeon"),
        // the raw name "aeon" must NOT be added to the map, because it would
        // match every qualified name in the workspace (aeon::core::..., etc.).
        let mut map = HashMap::new();
        // Simulate only the non-workspace crates being in the map.
        map.insert("aeon_consensus".to_string(), "aeon::consensus".to_string());
        // Crucially, "aeon" → "aeon::app" must NOT be in the map.

        let mut result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "aeon::core::config::Config".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "struct".to_string(),
                parent_qualified_name: Some("aeon::core::config".to_string()),
                source_ref: "src/config.rs:5".to_string(),
                language: "rust".to_string(),
                metadata: None,
                tags: vec![],
            }],
            relations: vec![],
            warnings: vec![],
        };

        rewrite_cross_crate_prefixes(&mut result, &map);

        assert_eq!(
            result.items[0].qualified_name, "aeon::core::config::Config",
            "items starting with the workspace prefix must not be rewritten"
        );
    }

    #[test]
    fn reparent_skips_ambiguous_matches() {
        let crate_name = "my_crate";

        // Two types with the same short name in different modules.
        let mut result = ParseResult {
            items: vec![
                AnalysisItem {
                    qualified_name: "my_crate::mod_a::Config".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "struct".to_string(),
                    parent_qualified_name: Some("my_crate::mod_a".to_string()),
                    source_ref: "src/mod_a.rs:1".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
                AnalysisItem {
                    qualified_name: "my_crate::mod_b::Config".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "struct".to_string(),
                    parent_qualified_name: Some("my_crate::mod_b".to_string()),
                    source_ref: "src/mod_b.rs:1".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
                // Orphaned method — ambiguous because two Configs exist.
                AnalysisItem {
                    qualified_name: "my_crate::mod_c::Config::new".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "function".to_string(),
                    parent_qualified_name: Some("my_crate::mod_c::Config".to_string()),
                    source_ref: "src/mod_c.rs:5".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
            ],
            relations: vec![],
            warnings: vec![],
        };

        reparent_orphaned_impl_items(&mut result, crate_name, None, None);

        // Should NOT reparent because there are two Config types.
        assert_eq!(
            result.items[2].qualified_name, "my_crate::mod_c::Config::new",
            "should not reparent when match is ambiguous"
        );
        assert_eq!(
            result.items[2].parent_qualified_name.as_deref(),
            Some("my_crate::mod_c::Config"),
        );
    }

    #[test]
    fn reparent_uses_cross_crate_types_when_no_local_match() {
        let crate_name = "aeon::client";

        // The orphaned method: impl TryFrom for Record in aeon-client,
        // but Record is defined in aeon-protocol.
        let mut result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "aeon::client::protocol::Record::try_from".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "function".to_string(),
                parent_qualified_name: Some("aeon::client::protocol::Record".to_string()),
                source_ref: "src/protocol.rs:20".to_string(),
                language: "rust".to_string(),
                metadata: None,
                tags: vec![],
            }],
            relations: vec![],
            warnings: vec![],
        };

        // Simulate types from previously-processed crates.
        let mut cross_crate = HashMap::new();
        cross_crate.insert(
            "Record".to_string(),
            vec!["aeon::protocol::Record".to_string()],
        );

        reparent_orphaned_impl_items(&mut result, crate_name, Some(&cross_crate), None);

        assert_eq!(
            result.items[0].qualified_name, "aeon::protocol::Record::try_from",
            "should reparent to cross-crate type when no local match exists"
        );
        assert_eq!(
            result.items[0].parent_qualified_name.as_deref(),
            Some("aeon::protocol::Record"),
        );
    }

    #[test]
    fn reparent_prefers_local_match_over_cross_crate() {
        let crate_name = "my_crate";

        let mut result = ParseResult {
            items: vec![
                // Local type definition.
                AnalysisItem {
                    qualified_name: "my_crate::types::Foo".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "struct".to_string(),
                    parent_qualified_name: Some("my_crate::types".to_string()),
                    source_ref: "src/types.rs:1".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
                // Orphaned method.
                AnalysisItem {
                    qualified_name: "my_crate::other::Foo::bar".to_string(),
                    kind: NodeKind::Unit,
                    sub_kind: "function".to_string(),
                    parent_qualified_name: Some("my_crate::other::Foo".to_string()),
                    source_ref: "src/other.rs:5".to_string(),
                    language: "rust".to_string(),
                    metadata: None,
                    tags: vec![],
                },
            ],
            relations: vec![],
            warnings: vec![],
        };

        // Cross-crate also has a Foo.
        let mut cross_crate = HashMap::new();
        cross_crate.insert("Foo".to_string(), vec!["other_crate::Foo".to_string()]);

        reparent_orphaned_impl_items(&mut result, crate_name, Some(&cross_crate), None);

        // Should use the local match, not the cross-crate one.
        assert_eq!(
            result.items[1].parent_qualified_name.as_deref(),
            Some("my_crate::types::Foo"),
            "should prefer local match over cross-crate"
        );
    }

    #[test]
    fn reparent_disambiguates_via_dependencies() {
        let crate_name = "aeon::client";
        // Method parented under a non-existent Record in aeon::client.
        let mut result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "aeon::client::protocol::Record::try_from".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "function".to_string(),
                parent_qualified_name: Some("aeon::client::protocol::Record".to_string()),
                source_ref: "src/protocol/mod.rs:10".to_string(),
                language: "rust".to_string(),
                metadata: None,
                tags: vec![],
            }],
            relations: vec![],
            warnings: vec![],
        };

        // Three cross-crate Record types, simulating workspace ambiguity.
        let mut cross_crate = HashMap::new();
        cross_crate.insert(
            "Record".to_string(),
            vec![
                "aeon::protocol::Record".to_string(),
                "aeon::wal::ring::record::Record".to_string(),
                "aeon::wal::evo::format::record::Record".to_string(),
            ],
        );

        // aeon::client depends on aeon::protocol (but not the wal crates).
        let deps = vec![
            ("aeon::client".to_string(), "aeon::protocol".to_string()),
            ("aeon::client".to_string(), "aeon::core".to_string()),
        ];

        reparent_orphaned_impl_items(&mut result, crate_name, Some(&cross_crate), Some(&deps));

        assert_eq!(
            result.items[0].parent_qualified_name.as_deref(),
            Some("aeon::protocol::Record"),
            "should disambiguate via dependency info"
        );
        assert_eq!(
            result.items[0].qualified_name, "aeon::protocol::Record::try_from",
            "qualified name should be rewritten"
        );
    }

    #[test]
    fn reparent_skips_when_fully_ambiguous() {
        // When both dep-based AND suffix-based disambiguation fail, the orphan
        // should remain untouched.
        let crate_name = "aeon::client";
        let mut result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "aeon::client::types::Config::default".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "function".to_string(),
                parent_qualified_name: Some("aeon::client::types::Config".to_string()),
                source_ref: "src/types/mod.rs:10".to_string(),
                language: "rust".to_string(),
                metadata: None,
                tags: vec![],
            }],
            relations: vec![],
            warnings: vec![],
        };

        // Two Config types, both with matching suffixes (types::Config).
        let mut cross_crate = HashMap::new();
        cross_crate.insert(
            "Config".to_string(),
            vec![
                "aeon::core::types::Config".to_string(),
                "aeon::protocol::types::Config".to_string(),
            ],
        );

        // aeon::client depends on both.
        let deps = vec![
            ("aeon::client".to_string(), "aeon::core".to_string()),
            ("aeon::client".to_string(), "aeon::protocol".to_string()),
        ];

        reparent_orphaned_impl_items(&mut result, crate_name, Some(&cross_crate), Some(&deps));

        // Should NOT reparent because both candidates match deps AND suffix.
        assert_eq!(
            result.items[0].parent_qualified_name.as_deref(),
            Some("aeon::client::types::Config"),
            "should not reparent when all disambiguation strategies fail"
        );
    }

    #[test]
    fn reparent_cross_namespace_via_path_suffix() {
        // Scenario: aeon::rest has `impl TryFrom<DataPoint> for Record` where
        // Record is imported via `use aeon_client::protocol::Record` — after
        // prefix rewriting, the parent becomes `aeon::client::protocol::Record`
        // which is NOT in the `aeon::rest::` namespace.
        //
        // aeon::rest does NOT directly depend on aeon::protocol, so dep-based
        // disambiguation fails. Instead, path suffix matching resolves it:
        // parent suffix "client::protocol::Record" doesn't match, but the
        // suffix "protocol::Record" appears in "aeon::protocol::Record".
        let crate_name = "aeon::rest";
        let mut result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "aeon::client::protocol::Record::try_from".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "function".to_string(),
                parent_qualified_name: Some("aeon::client::protocol::Record".to_string()),
                source_ref: "src/handlers/websocket.rs:173".to_string(),
                language: "rust".to_string(),
                metadata: None,
                tags: vec![],
            }],
            relations: vec![],
            warnings: vec![],
        };

        // Three Record types in the workspace.
        let mut cross_crate = HashMap::new();
        cross_crate.insert(
            "Record".to_string(),
            vec![
                "aeon::protocol::Record".to_string(),
                "aeon::wal::ring::record::Record".to_string(),
                "aeon::wal::evo::format::record::Record".to_string(),
            ],
        );

        // aeon::rest depends on aeon::client but NOT aeon::protocol directly.
        let deps = vec![
            ("aeon::rest".to_string(), "aeon::client".to_string()),
            ("aeon::rest".to_string(), "aeon::data".to_string()),
        ];

        reparent_orphaned_impl_items(&mut result, crate_name, Some(&cross_crate), Some(&deps));

        assert_eq!(
            result.items[0].parent_qualified_name.as_deref(),
            Some("aeon::protocol::Record"),
            "should reparent cross-namespace orphan using path suffix matching"
        );
        assert_eq!(
            result.items[0].qualified_name, "aeon::protocol::Record::try_from",
            "qualified name should be rewritten for cross-namespace reparent"
        );
    }
}
