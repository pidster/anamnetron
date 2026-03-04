//! Generic orchestrator driven by [`LanguageDescriptor`] and [`LanguageParser`].
//!
//! [`DescriptorOrchestrator`] wraps a descriptor (discovery configuration) and a
//! parser (source code analysis), implementing [`LanguageOrchestrator`] by
//! walking the project tree according to the descriptor's manifest files and
//! source extensions.

use std::path::{Path, PathBuf};

use svt_core::analysis::{LanguageDescriptor, LanguageParser};
use svt_core::model::EdgeKind;

use crate::languages::ParseResult;
use crate::types::{AnalysisItem, AnalysisRelation};

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

        // Emit cross-package Depends edges for workspace-internal dependencies.
        for dep in &unit.workspace_dependencies {
            result.relations.push(AnalysisRelation {
                source_qualified_name: unit.name.clone(),
                target_qualified_name: dep.clone(),
                kind: EdgeKind::Depends,
            });
        }
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
            workspace_dependencies: vec![],
        });
    }

    // Resolve workspace-internal dependencies: parse each unit's manifest
    // for declared dependencies and filter to only those that match another
    // discovered unit name.
    resolve_workspace_dependencies(&mut units, descriptor);

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

/// Resolve workspace-internal dependencies for all discovered units.
///
/// Builds a set of known unit names, then parses each unit's manifest to
/// extract declared dependencies. Only dependencies whose names match
/// another discovered unit are kept (workspace-internal filtering).
///
/// For Go modules, dependency names are full module paths (e.g.,
/// `github.com/user/repo`) while unit names use the last path segment
/// (e.g., `repo`). This function maps full module paths to unit names
/// via the `go.mod` manifest of each discovered unit.
fn resolve_workspace_dependencies(units: &mut [LanguageUnit], descriptor: &LanguageDescriptor) {
    use std::collections::{HashMap, HashSet};

    let known_names: HashSet<String> = units.iter().map(|u| u.name.clone()).collect();
    if known_names.len() <= 1 {
        return; // No cross-package dependencies possible with 0 or 1 units.
    }

    // For Go: build a mapping from full module path → unit name, so we can
    // match `require github.com/user/other` against the discovered unit
    // named `other`.
    let go_module_to_name: HashMap<String, String> = if descriptor.language_id == "go" {
        units
            .iter()
            .filter_map(|u| {
                let go_mod_path = u.root.join("go.mod");
                let content = std::fs::read_to_string(&go_mod_path).ok()?;
                let module_path = content
                    .lines()
                    .find(|line| line.trim().starts_with("module "))
                    .and_then(|line| line.trim().strip_prefix("module "))
                    .map(|s| s.trim().to_string())?;
                Some((module_path, u.name.clone()))
            })
            .collect()
    } else {
        HashMap::new()
    };

    // For Python: dependency names are normalized (- → _), so build a
    // lookup from normalized name → unit name.
    let py_normalized_to_name: HashMap<String, String> = if descriptor.language_id == "python" {
        units
            .iter()
            .map(|u| (u.name.replace('-', "_"), u.name.clone()))
            .collect()
    } else {
        HashMap::new()
    };

    for unit in units.iter_mut() {
        // Find the manifest file path by checking which descriptor manifest
        // files exist in the unit root.
        let manifest_path = descriptor
            .manifest_files
            .iter()
            .map(|f| unit.root.join(f))
            .find(|p| p.is_file());

        let manifest_path = match manifest_path {
            Some(p) => p,
            None => continue,
        };

        let raw_deps = extract_dependencies_from_manifest(&manifest_path, &descriptor.language_id);

        let workspace_deps: Vec<String> = raw_deps
            .into_iter()
            .filter_map(|dep| {
                // Direct name match (TypeScript, Python with exact names).
                if known_names.contains(&dep) && dep != unit.name {
                    return Some(dep);
                }
                // Go: map full module path → unit name.
                if let Some(name) = go_module_to_name.get(&dep) {
                    if name != &unit.name {
                        return Some(name.clone());
                    }
                }
                // Python: match normalized name (- → _) to unit name.
                if let Some(name) = py_normalized_to_name.get(&dep) {
                    if name != &unit.name {
                        return Some(name.clone());
                    }
                }
                None
            })
            .collect();

        unit.workspace_dependencies = workspace_deps;
    }
}

/// Extract dependency names from a build-tool manifest file.
///
/// Dispatches to language-specific parsers based on the language identifier:
/// - `"typescript"` — parses `package.json` (`dependencies` + `devDependencies`)
/// - `"go"` — parses `go.mod` (`require` directives)
/// - `"python"` — parses `pyproject.toml` (`[project].dependencies`)
///
/// Returns an empty vec for unsupported languages or if the file cannot be read.
fn extract_dependencies_from_manifest(manifest_path: &Path, language_id: &str) -> Vec<String> {
    let content = match std::fs::read_to_string(manifest_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    match language_id {
        "typescript" => parse_ts_dependencies(&content),
        "go" => parse_go_requires(&content),
        "python" => parse_pyproject_dependencies(&content),
        _ => vec![],
    }
}

/// Parse dependency names from a `package.json` file.
///
/// Extracts keys from both `"dependencies"` and `"devDependencies"` objects.
/// Handles scoped packages by stripping the scope prefix (e.g., `@scope/name` becomes `name`).
fn parse_ts_dependencies(content: &str) -> Vec<String> {
    let json: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut deps = Vec::new();

    for section in &["dependencies", "devDependencies"] {
        if let Some(obj) = json.get(section).and_then(|v| v.as_object()) {
            for key in obj.keys() {
                deps.push(key.clone());
            }
        }
    }

    deps
}

/// Parse required module paths from a `go.mod` file.
///
/// Handles both single-line `require path/to/mod v1.0.0` directives and
/// multi-line `require ( ... )` blocks. Returns the full module path for
/// each required dependency.
fn parse_go_requires(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_require_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "require (" {
            in_require_block = true;
            continue;
        }
        if in_require_block {
            if trimmed == ")" {
                in_require_block = false;
                continue;
            }
            // Lines inside require block: "module/path v1.2.3"
            if let Some(module_path) = trimmed.split_whitespace().next() {
                if !module_path.starts_with("//") {
                    deps.push(module_path.to_string());
                }
            }
            continue;
        }
        // Single-line require: "require module/path v1.2.3"
        if let Some(rest) = trimmed.strip_prefix("require ") {
            let rest = rest.trim();
            // Skip if it's a block opening
            if rest == "(" {
                in_require_block = true;
                continue;
            }
            if let Some(module_path) = rest.split_whitespace().next() {
                deps.push(module_path.to_string());
            }
        }
    }

    deps
}

/// Parse dependency names from a `pyproject.toml` file.
///
/// Extracts package names from the `[project].dependencies` list.
/// Dependency specifiers (version constraints, extras) are stripped; only
/// the bare package name is returned, normalized with `-` replaced by `_`.
fn parse_pyproject_dependencies(content: &str) -> Vec<String> {
    // Use the toml crate since the descriptor module already depends on it.
    let toml_val: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let deps_array = match toml_val
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        Some(arr) => arr,
        None => return vec![],
    };

    deps_array
        .iter()
        .filter_map(|v| v.as_str())
        .map(|spec| {
            // Strip version specifiers: "requests>=2.0" -> "requests"
            // Also handles extras: "package[extra]>=1.0" -> "package"
            let name = spec
                .split(&['>', '<', '=', '!', '~', ';', '['][..])
                .next()
                .unwrap_or(spec)
                .trim();
            // Normalize: PEP 503 says `-` and `_` are equivalent
            name.replace('-', "_")
        })
        .filter(|n| !n.is_empty())
        .collect()
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
            workspace_dependencies: vec![],
        };

        let result = orch.analyze(&unit);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qualified_name, "test-unit::test");
    }

    // --- Dependency parsing tests ---

    #[test]
    fn parse_ts_dependencies_extracts_deps_and_dev_deps() {
        let content = r#"{
            "name": "my-app",
            "dependencies": {
                "lib-a": "^1.0.0",
                "lib-b": "~2.0.0"
            },
            "devDependencies": {
                "vitest": "^0.34.0"
            }
        }"#;

        let deps = parse_ts_dependencies(content);
        assert!(deps.contains(&"lib-a".to_string()));
        assert!(deps.contains(&"lib-b".to_string()));
        assert!(deps.contains(&"vitest".to_string()));
    }

    #[test]
    fn parse_ts_dependencies_returns_empty_for_no_deps() {
        let content = r#"{"name": "standalone"}"#;
        let deps = parse_ts_dependencies(content);
        assert!(deps.is_empty());
    }

    #[test]
    fn parse_ts_dependencies_returns_empty_for_invalid_json() {
        let deps = parse_ts_dependencies("not json");
        assert!(deps.is_empty());
    }

    #[test]
    fn parse_go_requires_extracts_from_block() {
        let content = "\
module github.com/user/myapp

go 1.21

require (
\tgithub.com/user/lib-a v1.0.0
\tgithub.com/user/lib-b v2.0.0
\tgithub.com/external/pkg v0.5.0
)
";

        let deps = parse_go_requires(content);
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&"github.com/user/lib-a".to_string()));
        assert!(deps.contains(&"github.com/user/lib-b".to_string()));
        assert!(deps.contains(&"github.com/external/pkg".to_string()));
    }

    #[test]
    fn parse_go_requires_extracts_single_line() {
        let content = "\
module github.com/user/app

go 1.21

require github.com/user/util v1.0.0
";

        let deps = parse_go_requires(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], "github.com/user/util");
    }

    #[test]
    fn parse_go_requires_returns_empty_when_no_requires() {
        let content = "\
module github.com/user/app

go 1.21
";

        let deps = parse_go_requires(content);
        assert!(deps.is_empty());
    }

    #[test]
    fn parse_pyproject_dependencies_extracts_names() {
        let content = "\
[project]
name = \"my-app\"
dependencies = [
    \"lib-a>=1.0\",
    \"lib_b~=2.0\",
    \"requests\",
]
";

        let deps = parse_pyproject_dependencies(content);
        assert!(
            deps.contains(&"lib_a".to_string()),
            "should normalize - to _"
        );
        assert!(deps.contains(&"lib_b".to_string()));
        assert!(deps.contains(&"requests".to_string()));
    }

    #[test]
    fn parse_pyproject_dependencies_strips_extras() {
        let content = "\
[project]
name = \"my-app\"
dependencies = [
    \"package[extra]>=1.0\",
]
";

        let deps = parse_pyproject_dependencies(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], "package");
    }

    #[test]
    fn parse_pyproject_dependencies_returns_empty_when_no_deps() {
        let content = "\
[project]
name = \"standalone\"
";

        let deps = parse_pyproject_dependencies(content);
        assert!(deps.is_empty());
    }

    #[test]
    fn parse_pyproject_dependencies_returns_empty_for_invalid_toml() {
        let deps = parse_pyproject_dependencies("not valid toml {{{{");
        assert!(deps.is_empty());
    }

    // --- Workspace dependency resolution tests ---

    #[test]
    fn ts_package_workspace_dependencies_detected() {
        let dir = TempDir::new().unwrap();

        // Create two TS packages: app depends on lib
        let app_dir = dir.path().join("app");
        let lib_dir = dir.path().join("lib");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::create_dir_all(&lib_dir).unwrap();

        std::fs::write(
            app_dir.join("package.json"),
            r#"{"name": "app", "dependencies": {"lib": "^1.0.0", "external-dep": "^2.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(app_dir.join("index.ts"), "import { x } from 'lib';").unwrap();

        std::fs::write(lib_dir.join("package.json"), r#"{"name": "lib"}"#).unwrap();
        std::fs::write(lib_dir.join("index.ts"), "export const x = 1;").unwrap();

        let descriptor = LanguageDescriptor {
            language_id: "typescript".to_string(),
            manifest_files: vec!["package.json".to_string()],
            source_extensions: vec![".ts".to_string(), ".tsx".to_string()],
            skip_directories: vec!["node_modules".to_string()],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
        };

        let units = discover_by_descriptor(dir.path(), &descriptor);
        assert_eq!(units.len(), 2, "should discover both packages");

        let app_unit = units
            .iter()
            .find(|u| u.name == "app")
            .expect("should find app");
        assert_eq!(
            app_unit.workspace_dependencies,
            vec!["lib".to_string()],
            "app should depend on lib (workspace-internal), not external-dep"
        );

        let lib_unit = units
            .iter()
            .find(|u| u.name == "lib")
            .expect("should find lib");
        assert!(
            lib_unit.workspace_dependencies.is_empty(),
            "lib should have no workspace dependencies"
        );
    }

    #[test]
    fn go_module_workspace_dependencies_detected() {
        let dir = TempDir::new().unwrap();

        // Create two Go modules: svc depends on shared
        let svc_dir = dir.path().join("svc");
        let shared_dir = dir.path().join("shared");
        std::fs::create_dir_all(&svc_dir).unwrap();
        std::fs::create_dir_all(&shared_dir).unwrap();

        std::fs::write(
            svc_dir.join("go.mod"),
            "\
module github.com/user/svc

go 1.21

require (
\tgithub.com/user/shared v0.0.0
\tgithub.com/external/pkg v1.0.0
)
",
        )
        .unwrap();
        std::fs::write(svc_dir.join("main.go"), "package main\nfunc main() {}\n").unwrap();

        std::fs::write(
            shared_dir.join("go.mod"),
            "module github.com/user/shared\n\ngo 1.21\n",
        )
        .unwrap();
        std::fs::write(
            shared_dir.join("lib.go"),
            "package shared\nfunc Hello() {}\n",
        )
        .unwrap();

        let descriptor = LanguageDescriptor {
            language_id: "go".to_string(),
            manifest_files: vec!["go.mod".to_string()],
            source_extensions: vec![".go".to_string()],
            skip_directories: vec!["vendor".to_string()],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        };

        let units = discover_by_descriptor(dir.path(), &descriptor);
        assert_eq!(units.len(), 2, "should discover both modules");

        let svc_unit = units
            .iter()
            .find(|u| u.name == "svc")
            .expect("should find svc");
        assert_eq!(
            svc_unit.workspace_dependencies,
            vec!["shared".to_string()],
            "svc should depend on shared (workspace-internal), not external/pkg"
        );

        let shared_unit = units
            .iter()
            .find(|u| u.name == "shared")
            .expect("should find shared");
        assert!(
            shared_unit.workspace_dependencies.is_empty(),
            "shared should have no workspace dependencies"
        );
    }

    #[test]
    fn python_package_workspace_dependencies_detected() {
        let dir = TempDir::new().unwrap();

        // Create two Python packages: app depends on core-lib
        let app_dir = dir.path().join("app");
        let lib_dir = dir.path().join("core-lib");
        std::fs::create_dir_all(app_dir.join("app")).unwrap();
        std::fs::create_dir_all(lib_dir.join("core_lib")).unwrap();

        std::fs::write(
            app_dir.join("pyproject.toml"),
            "\
[project]
name = \"app\"
dependencies = [
    \"core-lib>=0.1\",
    \"requests>=2.0\",
]
",
        )
        .unwrap();
        std::fs::write(app_dir.join("app/__init__.py"), "").unwrap();
        std::fs::write(app_dir.join("app/main.py"), "def run(): pass\n").unwrap();

        std::fs::write(
            lib_dir.join("pyproject.toml"),
            "\
[project]
name = \"core-lib\"
dependencies = []
",
        )
        .unwrap();
        std::fs::write(lib_dir.join("core_lib/__init__.py"), "").unwrap();
        std::fs::write(lib_dir.join("core_lib/utils.py"), "def helper(): pass\n").unwrap();

        let descriptor = LanguageDescriptor {
            language_id: "python".to_string(),
            manifest_files: vec!["pyproject.toml".to_string(), "setup.py".to_string()],
            source_extensions: vec![".py".to_string()],
            skip_directories: vec![".venv".to_string(), "__pycache__".to_string()],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
        };

        let units = discover_by_descriptor(dir.path(), &descriptor);
        assert_eq!(units.len(), 2, "should discover both packages");

        let app_unit = units
            .iter()
            .find(|u| u.name == "app")
            .expect("should find app");
        assert_eq!(
            app_unit.workspace_dependencies,
            vec!["core-lib".to_string()],
            "app should depend on core-lib (workspace-internal), not requests"
        );

        let lib_unit = units
            .iter()
            .find(|u| u.name == "core-lib")
            .expect("should find core-lib");
        assert!(
            lib_unit.workspace_dependencies.is_empty(),
            "core-lib should have no workspace dependencies"
        );
    }

    #[test]
    fn post_process_emits_workspace_dependency_edges() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.mock");
        std::fs::write(&file, "content").unwrap();

        let orch = DescriptorOrchestrator::new(test_descriptor(), Box::new(MockParser));
        let unit = LanguageUnit {
            name: "app".to_string(),
            language: "mock".to_string(),
            root: dir.path().to_path_buf(),
            source_root: dir.path().to_path_buf(),
            source_files: vec![file],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
            source_ref: "test".to_string(),
            parent_qualified_name: None,
            workspace_dependencies: vec!["lib-a".to_string(), "lib-b".to_string()],
        };

        let mut result = ParseResult::default();
        orch.post_process(&unit, &mut result);

        assert_eq!(
            result.relations.len(),
            2,
            "should emit 2 Depends edges for workspace dependencies"
        );
        assert_eq!(result.relations[0].source_qualified_name, "app");
        assert_eq!(result.relations[0].target_qualified_name, "lib-a");
        assert_eq!(result.relations[0].kind, EdgeKind::Depends);
        assert_eq!(result.relations[1].source_qualified_name, "app");
        assert_eq!(result.relations[1].target_qualified_name, "lib-b");
        assert_eq!(result.relations[1].kind, EdgeKind::Depends);
    }
}
