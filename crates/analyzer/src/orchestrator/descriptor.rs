//! Generic orchestrator driven by [`LanguageDescriptor`] and [`LanguageParser`].
//!
//! [`DescriptorOrchestrator`] wraps a descriptor (discovery configuration) and a
//! parser (source code analysis), implementing [`LanguageOrchestrator`] by
//! walking the project tree according to the descriptor's manifest files and
//! source extensions.

use std::path::{Path, PathBuf};

use svt_core::analysis::{LanguageDescriptor, LanguageParser};

use crate::languages::ParseResult;
use crate::types::AnalysisItem;

use super::{LanguageOrchestrator, LanguageUnit};

/// A generic orchestrator that delegates discovery to a [`LanguageDescriptor`]
/// and parsing to a [`LanguageParser`].
pub struct DescriptorOrchestrator {
    descriptor: LanguageDescriptor,
    parser: Box<dyn LanguageParser>,
}

impl std::fmt::Debug for DescriptorOrchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DescriptorOrchestrator")
            .field("language_id", &self.descriptor.language_id)
            .finish()
    }
}

impl DescriptorOrchestrator {
    /// Create a new `DescriptorOrchestrator` from a descriptor and parser.
    pub fn new(descriptor: LanguageDescriptor, parser: Box<dyn LanguageParser>) -> Self {
        Self { descriptor, parser }
    }
}

impl LanguageOrchestrator for DescriptorOrchestrator {
    fn language_id(&self) -> &str {
        &self.descriptor.language_id
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_by_descriptor(root, &self.descriptor)
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.parser.parse(&unit.name, &file_refs)
    }

    fn emit_structural_items(&self, unit: &LanguageUnit) -> Vec<AnalysisItem> {
        self.parser
            .emit_structural_items(&unit.source_root, &unit.name, &unit.source_files)
    }

    fn post_process(&self, unit: &LanguageUnit, result: &mut ParseResult) {
        self.parser
            .post_process(&unit.source_root, &unit.name, result);
    }
}

/// Walk the project root recursively, using the descriptor to find manifest
/// files and collect source files for each discovered unit.
fn discover_by_descriptor(root: &Path, descriptor: &LanguageDescriptor) -> Vec<LanguageUnit> {
    let mut units = Vec::new();

    let walker = walkdir::WalkDir::new(root).follow_links(false);
    for entry in walker.into_iter().filter_entry(|e| {
        if !e.file_type().is_dir() {
            return true;
        }
        let name = e.file_name().to_str().unwrap_or("");
        !descriptor.skip_directories.contains(&name.to_string())
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let file_name = entry.file_name().to_str().unwrap_or("");
        if !descriptor.manifest_files.contains(&file_name.to_string()) {
            continue;
        }

        // Found a manifest — the parent directory is a unit.
        let manifest_path = entry.path();
        let unit_dir = match manifest_path.parent() {
            Some(d) => d,
            None => continue,
        };

        let name = extract_name_from_manifest(manifest_path, unit_dir);
        let source_files = collect_source_files(unit_dir, descriptor);

        units.push(LanguageUnit {
            name,
            language: descriptor.language_id.clone(),
            root: unit_dir.to_path_buf(),
            source_root: unit_dir.to_path_buf(),
            source_files,
            top_level_kind: descriptor.top_level_kind,
            top_level_sub_kind: descriptor.top_level_sub_kind.clone(),
            source_ref: manifest_path.display().to_string(),
            parent_qualified_name: None,
        });
    }

    units
}

/// Try to extract a package/module name from a manifest file.
///
/// Strategy (tried in order):
/// 1. `go.mod` — regex for `module <path>`, use last path segment
/// 2. JSON — look for `"name"` field
/// 3. TOML — look for `[project].name` or `[package].name`
/// 4. Fallback — use the directory name
fn extract_name_from_manifest(manifest_path: &Path, unit_dir: &Path) -> String {
    let content = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(_) => return dir_name_fallback(unit_dir),
    };

    let file_name = manifest_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // go.mod: `module github.com/user/repo`
    if file_name == "go.mod" {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("module ") {
                let module_path = rest.trim();
                if let Some(last) = module_path.rsplit('/').next() {
                    return last.to_string();
                }
            }
        }
    }

    // JSON: { "name": "..." }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }

    // TOML: [project].name or [package].name
    if let Ok(toml_val) = content.parse::<toml::Value>() {
        if let Some(name) = toml_val
            .get("project")
            .and_then(|t| t.get("name"))
            .and_then(|v| v.as_str())
        {
            if !name.is_empty() {
                return name.to_string();
            }
        }
        if let Some(name) = toml_val
            .get("package")
            .and_then(|t| t.get("name"))
            .and_then(|v| v.as_str())
        {
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }

    dir_name_fallback(unit_dir)
}

/// Fallback: use the directory name as the unit name.
fn dir_name_fallback(unit_dir: &Path) -> String {
    unit_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Collect all source files under `unit_dir` matching the descriptor's extensions,
/// skipping directories listed in `skip_directories`.
fn collect_source_files(unit_dir: &Path, descriptor: &LanguageDescriptor) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let walker = walkdir::WalkDir::new(unit_dir).follow_links(false);
    for entry in walker.into_iter().filter_entry(|e| {
        if !e.file_type().is_dir() {
            return true;
        }
        let name = e.file_name().to_str().unwrap_or("");
        !descriptor.skip_directories.contains(&name.to_string())
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let dot_ext = format!(".{ext}");
            if descriptor.source_extensions.contains(&dot_ext) {
                files.push(path.to_path_buf());
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use svt_core::analysis::{AnalysisItem, ParseResult};
    use svt_core::model::NodeKind;
    use tempfile::TempDir;

    /// A mock parser that returns one item per file.
    struct MockParser;

    impl LanguageParser for MockParser {
        fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult {
            ParseResult {
                items: files
                    .iter()
                    .map(|f| AnalysisItem {
                        qualified_name: format!(
                            "{unit_name}::{}",
                            f.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown")
                        ),
                        kind: NodeKind::Unit,
                        sub_kind: "file".to_string(),
                        parent_qualified_name: Some(unit_name.to_string()),
                        source_ref: f.display().to_string(),
                        language: "mock".to_string(),
                        metadata: None,
                        tags: vec![],
                    })
                    .collect(),
                relations: vec![],
                warnings: vec![],
            }
        }
    }

    fn test_descriptor() -> LanguageDescriptor {
        LanguageDescriptor {
            language_id: "mock".to_string(),
            manifest_files: vec!["manifest.json".to_string()],
            source_extensions: vec![".mock".to_string()],
            skip_directories: vec!["skip_me".to_string()],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
        }
    }

    #[test]
    fn descriptor_orchestrator_language_id() {
        let orch = DescriptorOrchestrator::new(test_descriptor(), Box::new(MockParser));
        assert_eq!(orch.language_id(), "mock");
    }

    #[test]
    fn discover_finds_unit_from_manifest() {
        let dir = TempDir::new().unwrap();
        let pkg_dir = dir.path().join("my-pkg");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("manifest.json"), r#"{"name": "my-package"}"#).unwrap();
        std::fs::write(pkg_dir.join("main.mock"), "content").unwrap();

        let orch = DescriptorOrchestrator::new(test_descriptor(), Box::new(MockParser));
        let units = orch.discover(dir.path());

        assert_eq!(units.len(), 1);
        assert_eq!(units[0].name, "my-package");
        assert_eq!(units[0].language, "mock");
        assert!(!units[0].source_files.is_empty());
    }

    #[test]
    fn discover_skips_excluded_directories() {
        let dir = TempDir::new().unwrap();
        let skip_dir = dir.path().join("skip_me");
        std::fs::create_dir_all(&skip_dir).unwrap();
        std::fs::write(skip_dir.join("manifest.json"), r#"{"name": "hidden"}"#).unwrap();

        let orch = DescriptorOrchestrator::new(test_descriptor(), Box::new(MockParser));
        let units = orch.discover(dir.path());
        assert!(units.is_empty(), "should skip excluded directories");
    }

    #[test]
    fn discover_empty_directory_returns_empty() {
        let dir = TempDir::new().unwrap();
        let orch = DescriptorOrchestrator::new(test_descriptor(), Box::new(MockParser));
        let units = orch.discover(dir.path());
        assert!(units.is_empty());
    }

    #[test]
    fn extract_name_from_json_manifest() {
        let dir = TempDir::new().unwrap();
        let manifest = dir.path().join("manifest.json");
        std::fs::write(&manifest, r#"{"name": "json-pkg"}"#).unwrap();
        assert_eq!(
            extract_name_from_manifest(&manifest, dir.path()),
            "json-pkg"
        );
    }

    #[test]
    fn extract_name_from_toml_project() {
        let dir = TempDir::new().unwrap();
        let manifest = dir.path().join("pyproject.toml");
        std::fs::write(&manifest, "[project]\nname = \"toml-pkg\"\n").unwrap();
        assert_eq!(
            extract_name_from_manifest(&manifest, dir.path()),
            "toml-pkg"
        );
    }

    #[test]
    fn extract_name_from_toml_package() {
        let dir = TempDir::new().unwrap();
        let manifest = dir.path().join("Cargo.toml");
        std::fs::write(&manifest, "[package]\nname = \"cargo-pkg\"\n").unwrap();
        assert_eq!(
            extract_name_from_manifest(&manifest, dir.path()),
            "cargo-pkg"
        );
    }

    #[test]
    fn extract_name_from_go_mod() {
        let dir = TempDir::new().unwrap();
        let manifest = dir.path().join("go.mod");
        std::fs::write(&manifest, "module github.com/user/myrepo\n\ngo 1.21\n").unwrap();
        assert_eq!(extract_name_from_manifest(&manifest, dir.path()), "myrepo");
    }

    #[test]
    fn extract_name_falls_back_to_directory_name() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("fallback-dir");
        std::fs::create_dir_all(&sub).unwrap();
        let manifest = sub.join("manifest.json");
        std::fs::write(&manifest, "not valid json or toml").unwrap();
        assert_eq!(extract_name_from_manifest(&manifest, &sub), "fallback-dir");
    }

    #[test]
    fn analyze_delegates_to_parser() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.mock");
        std::fs::write(&file, "content").unwrap();

        let orch = DescriptorOrchestrator::new(test_descriptor(), Box::new(MockParser));
        let unit = LanguageUnit {
            name: "test-unit".to_string(),
            language: "mock".to_string(),
            root: dir.path().to_path_buf(),
            source_root: dir.path().to_path_buf(),
            source_files: vec![file],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
            source_ref: "test".to_string(),
            parent_qualified_name: None,
        };

        let result = orch.analyze(&unit);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qualified_name, "test-unit::test");
    }
}
