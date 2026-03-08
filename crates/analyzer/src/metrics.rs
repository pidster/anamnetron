//! Post-analysis metric enrichment using `rust-code-analysis`.
//!
//! This module runs as a separate enrichment pass between post-processing
//! and graph mapping. It groups [`AnalysisItem`]s by source file, calls
//! [`get_function_spaces`] once per file, matches [`FuncSpace`] results
//! back to items by start line, and merges metrics into existing metadata.

use std::collections::HashMap;
use std::path::Path;

use rust_code_analysis::{get_function_spaces, FuncSpace, SpaceKind, LANG};
use tracing::{debug, warn};

use crate::types::AnalysisItem;

/// Map our language identifiers to `rust-code-analysis` [`LANG`] values.
///
/// Returns `None` for languages not supported by the library (e.g., Go).
fn to_rca_lang(language: &str) -> Option<LANG> {
    match language {
        "rust" => Some(LANG::Rust),
        "typescript" => Some(LANG::Typescript),
        "python" => Some(LANG::Python),
        "java" => Some(LANG::Java),
        "javascript" => Some(LANG::Javascript),
        _ => None,
    }
}

/// Parse a `source_ref` string into `(file_path, start_line)`.
///
/// Expected format: `"path/to/file.rs:42"`. Returns `None` if the
/// format is invalid or the line number cannot be parsed.
fn parse_source_ref(source_ref: &str) -> Option<(&str, usize)> {
    let colon = source_ref.rfind(':')?;
    let path = &source_ref[..colon];
    let line: usize = source_ref[colon + 1..].parse().ok()?;
    if path.is_empty() {
        return None;
    }
    Some((path, line))
}

/// Flatten a [`FuncSpace`] tree into a map keyed by `start_line`.
///
/// The root (file-level, `kind == Unit`) is stored at line 0 so it can
/// be matched to module-level items that lack a specific line number.
fn flatten_func_spaces(root: &FuncSpace) -> HashMap<usize, &FuncSpace> {
    let mut map = HashMap::new();
    flatten_recursive(root, &mut map);
    map
}

fn flatten_recursive<'a>(space: &'a FuncSpace, map: &mut HashMap<usize, &'a FuncSpace>) {
    let key = if space.kind == SpaceKind::Unit {
        0
    } else {
        space.start_line
    };
    // First entry wins (prefer the outermost scope at a given line).
    map.entry(key).or_insert(space);
    for child in &space.spaces {
        flatten_recursive(child, map);
    }
}

/// Insert a finite numeric value into the metadata JSON object.
///
/// NaN and infinity are silently skipped to avoid invalid JSON.
fn insert_if_finite(obj: &mut serde_json::Map<String, serde_json::Value>, key: &str, val: f64) {
    if val.is_finite() {
        obj.insert(key.to_string(), serde_json::json!(val));
    }
}

/// Extract metrics from a [`FuncSpace`] and merge them into an item's metadata.
fn merge_metrics(item: &mut AnalysisItem, space: &FuncSpace) {
    let meta = item.metadata.get_or_insert_with(|| serde_json::json!({}));
    let obj = match meta.as_object_mut() {
        Some(obj) => obj,
        None => return,
    };

    let m = &space.metrics;

    // Cyclomatic complexity
    insert_if_finite(obj, "cyclomatic", m.cyclomatic.cyclomatic());
    // Cognitive complexity
    insert_if_finite(obj, "cognitive", m.cognitive.cognitive());
    // Lines of code variants
    insert_if_finite(obj, "sloc", m.loc.sloc());
    insert_if_finite(obj, "ploc", m.loc.ploc());
    insert_if_finite(obj, "cloc", m.loc.cloc());
    // Halstead metrics
    insert_if_finite(obj, "halstead_volume", m.halstead.volume());
    insert_if_finite(obj, "halstead_difficulty", m.halstead.difficulty());
    insert_if_finite(obj, "halstead_bugs", m.halstead.bugs());
    // Maintainability Index
    insert_if_finite(obj, "mi", m.mi.mi_original());
    // Function arguments
    insert_if_finite(obj, "nargs", m.nargs.fn_args());
    // Exit points
    insert_if_finite(obj, "nexits", m.nexits.exit());
}

/// Enrich analysis items with code metrics from `rust-code-analysis`.
///
/// Groups items by source file, runs metric computation once per file,
/// and matches results back to items by start line number.
pub fn enrich_metrics(items: &mut [AnalysisItem], project_root: &Path) {
    // Step 1: Build a map of (relative_file_path, start_line) → item index.
    // Also collect unique file paths with their language.
    let mut file_items: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut file_languages: HashMap<String, String> = HashMap::new();

    for (idx, item) in items.iter().enumerate() {
        if let Some((path, line)) = parse_source_ref(&item.source_ref) {
            file_items
                .entry(path.to_string())
                .or_default()
                .push((idx, line));
            file_languages
                .entry(path.to_string())
                .or_insert_with(|| item.language.clone());
        }
    }

    let mut files_processed = 0usize;
    let mut items_enriched = 0usize;

    // Step 2: For each file, compute metrics and match back.
    for (rel_path, item_indices) in &file_items {
        let lang_str = match file_languages.get(rel_path.as_str()) {
            Some(l) => l.as_str(),
            None => continue,
        };
        let lang = match to_rca_lang(lang_str) {
            Some(l) => l,
            None => continue,
        };

        let abs_path = project_root.join(rel_path);
        let source = match std::fs::read(&abs_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                debug!(path = %abs_path.display(), error = %e, "skipping metrics: cannot read file");
                continue;
            }
        };

        let func_space = match get_function_spaces(&lang, source, &abs_path, None) {
            Some(fs) => fs,
            None => {
                debug!(path = %abs_path.display(), "skipping metrics: get_function_spaces returned None");
                continue;
            }
        };

        files_processed += 1;
        let space_map = flatten_func_spaces(&func_space);

        for &(item_idx, start_line) in item_indices {
            // Primary match: exact start_line.
            // Fallback: ±1 line tolerance.
            // File-level (Unit) metrics: match items at line 0 or 1.
            let matched = space_map
                .get(&start_line)
                .or_else(|| space_map.get(&(start_line.saturating_sub(1))))
                .or_else(|| space_map.get(&(start_line + 1)))
                .or_else(|| {
                    // For items at line 1 or 0, try the file-level Unit space.
                    if start_line <= 1 {
                        space_map.get(&0)
                    } else {
                        None
                    }
                });

            if let Some(space) = matched {
                merge_metrics(&mut items[item_idx], space);
                items_enriched += 1;
            }
        }
    }

    debug!(
        files = files_processed,
        items = items_enriched,
        "metric enrichment pass complete"
    );
    if files_processed == 0 && !items.is_empty() {
        warn!("metric enrichment processed 0 files — check source_ref paths");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use svt_core::model::NodeKind;

    fn make_item(qualified_name: &str, source_ref: &str, language: &str) -> AnalysisItem {
        AnalysisItem {
            qualified_name: qualified_name.to_string(),
            kind: NodeKind::Unit,
            sub_kind: "function".to_string(),
            parent_qualified_name: None,
            source_ref: source_ref.to_string(),
            language: language.to_string(),
            metadata: Some(serde_json::json!({"loc": 10})),
            tags: vec![],
        }
    }

    #[test]
    fn parse_source_ref_valid() {
        let (path, line) = parse_source_ref("src/lib.rs:42").unwrap();
        assert_eq!(path, "src/lib.rs");
        assert_eq!(line, 42);
    }

    #[test]
    fn parse_source_ref_nested_path() {
        let (path, line) = parse_source_ref("crates/core/src/model/mod.rs:1").unwrap();
        assert_eq!(path, "crates/core/src/model/mod.rs");
        assert_eq!(line, 1);
    }

    #[test]
    fn parse_source_ref_invalid() {
        assert!(parse_source_ref("no_colon").is_none());
        assert!(parse_source_ref(":42").is_none());
        assert!(parse_source_ref("file.rs:abc").is_none());
    }

    #[test]
    fn to_rca_lang_maps_known_languages() {
        assert_eq!(to_rca_lang("rust"), Some(LANG::Rust));
        assert_eq!(to_rca_lang("typescript"), Some(LANG::Typescript));
        assert_eq!(to_rca_lang("python"), Some(LANG::Python));
        assert_eq!(to_rca_lang("java"), Some(LANG::Java));
        assert_eq!(to_rca_lang("javascript"), Some(LANG::Javascript));
    }

    #[test]
    fn to_rca_lang_returns_none_for_unsupported() {
        assert_eq!(to_rca_lang("go"), None);
        assert_eq!(to_rca_lang("haskell"), None);
    }

    #[test]
    fn insert_if_finite_skips_nan() {
        let mut obj = serde_json::Map::new();
        insert_if_finite(&mut obj, "nan_val", f64::NAN);
        assert!(!obj.contains_key("nan_val"));
    }

    #[test]
    fn insert_if_finite_skips_infinity() {
        let mut obj = serde_json::Map::new();
        insert_if_finite(&mut obj, "inf_val", f64::INFINITY);
        assert!(!obj.contains_key("inf_val"));
    }

    #[test]
    fn insert_if_finite_inserts_normal_value() {
        let mut obj = serde_json::Map::new();
        insert_if_finite(&mut obj, "good", 42.5);
        assert_eq!(obj["good"], serde_json::json!(42.5));
    }

    #[test]
    fn enrich_metrics_on_real_source_file() {
        // Use this crate's own lib.rs as a test subject.
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir.parent().unwrap().parent().unwrap();
        let rel_path = "crates/analyzer/src/lib.rs";

        // Find the actual line number of analyze_project dynamically
        let source = std::fs::read_to_string(project_root.join(rel_path)).unwrap();
        let line_num = source
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("pub fn analyze_project("))
            .map(|(i, _)| i + 1)
            .unwrap_or(49);

        let mut items = vec![make_item(
            "svt_analyzer::analyze_project",
            &format!("{rel_path}:{line_num}"),
            "rust",
        )];

        enrich_metrics(&mut items, project_root);

        let meta = items[0].metadata.as_ref().unwrap();
        // The original loc metadata should still be present.
        assert_eq!(meta["loc"], 10, "original metadata should be preserved");
        // At least cyclomatic complexity should be computed for a real function.
        assert!(
            meta.get("cyclomatic").is_some(),
            "cyclomatic metric should be present after enrichment"
        );
    }

    #[test]
    fn enrich_metrics_skips_go_files() {
        let project_root = std::path::Path::new("/nonexistent");
        let mut items = vec![make_item("go_pkg::main", "main.go:1", "go")];
        // Should not panic; Go is unsupported by RCA.
        enrich_metrics(&mut items, project_root);
        let meta = items[0].metadata.as_ref().unwrap();
        assert!(
            meta.get("cyclomatic").is_none(),
            "Go items should not get RCA metrics"
        );
    }

    #[test]
    fn enrich_metrics_preserves_existing_metadata() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir.parent().unwrap().parent().unwrap();
        let rel_path = "crates/analyzer/src/lib.rs";

        let mut items = vec![AnalysisItem {
            qualified_name: "svt_analyzer".to_string(),
            kind: NodeKind::Service,
            sub_kind: "crate".to_string(),
            parent_qualified_name: None,
            source_ref: format!("{rel_path}:1"),
            language: "rust".to_string(),
            metadata: Some(serde_json::json!({"loc": 500, "custom_key": "preserved"})),
            tags: vec!["test".to_string()],
        }];

        enrich_metrics(&mut items, project_root);

        let meta = items[0].metadata.as_ref().unwrap();
        assert_eq!(
            meta["custom_key"], "preserved",
            "existing keys should survive enrichment"
        );
        assert_eq!(meta["loc"], 500, "existing loc should not be overwritten");
    }
}
