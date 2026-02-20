//! Project discovery: workspace detection, crate enumeration, source file walking.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::types::{
    CrateInfo, CrateType, GoPackage, GoPackageInfo, ProjectLayout, PythonPackageInfo, TsPackageInfo,
};

/// Errors during project discovery.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// Failed to run cargo metadata.
    #[error("cargo metadata failed: {0}")]
    CargoMetadata(String),
    /// IO error during file walking.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Discover the layout of a Rust project at the given root.
///
/// Runs `cargo metadata` to find workspace members and their targets,
/// then walks each crate's `src/` directory for `.rs` files.
pub fn discover_project(project_root: &Path) -> Result<ProjectLayout, DiscoveryError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .current_dir(project_root)
        .no_deps()
        .exec()
        .map_err(|e| DiscoveryError::CargoMetadata(e.to_string()))?;

    let workspace_root = metadata.workspace_root.clone().into_std_path_buf();
    let mut crates = Vec::new();

    for package in metadata.workspace_packages() {
        for target in &package.targets {
            let crate_type = if target.is_lib() {
                CrateType::Lib
            } else if target.is_bin() {
                CrateType::Bin
            } else {
                continue; // skip test, example, bench targets
            };

            let entry_point = target.src_path.clone().into_std_path_buf();
            let crate_root = package
                .manifest_path
                .parent()
                .map(|p| p.as_std_path().to_path_buf())
                .or_else(|| entry_point.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| entry_point.clone());

            let source_files = walk_rs_files(&crate_root.join("src"));

            crates.push(CrateInfo {
                name: package.name.clone(),
                crate_type,
                root: crate_root,
                entry_point,
                source_files,
            });
        }
    }

    let workspace_name = detect_workspace_name(&crates);

    Ok(ProjectLayout {
        workspace_root,
        crates,
        workspace_name,
    })
}

/// Detect the workspace name from the common prefix of crate package names.
///
/// For crates `["svt-core", "svt-cli", "svt-server"]`, returns `Some("svt")`.
/// Returns `None` if there are fewer than 2 crates or no common hyphen-separated prefix.
fn detect_workspace_name(crates: &[CrateInfo]) -> Option<String> {
    if crates.len() < 2 {
        return None;
    }

    let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
    let first = names[0];

    // Find longest common prefix
    let mut prefix_len = first.len();
    for name in &names[1..] {
        prefix_len = first
            .bytes()
            .zip(name.bytes())
            .take(prefix_len)
            .take_while(|(a, b)| a == b)
            .count();
    }

    let prefix = &first[..prefix_len];

    // Truncate to last hyphen boundary
    if let Some(hyphen_pos) = prefix.rfind('-') {
        let workspace = &prefix[..hyphen_pos];
        if !workspace.is_empty() {
            return Some(workspace.to_string());
        }
    }

    None
}

/// Recursively walk a directory and collect all `.rs` files.
fn walk_rs_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|e| e.into_path())
        .collect()
}

/// Discover TypeScript/JavaScript packages in a project tree.
///
/// Walks the directory tree looking for `package.json` files (skipping
/// `node_modules/`, `dist/`, `build/`, `.svt/`, `target/`). For each
/// package found, collects `.ts`, `.tsx`, and `.svelte` source files.
pub fn discover_ts_packages(project_root: &Path) -> Result<Vec<TsPackageInfo>, DiscoveryError> {
    let skip_dirs = ["node_modules", "dist", "build", ".svt", "target"];
    let mut packages = Vec::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !skip_dirs.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_name() != "package.json" || !entry.file_type().is_file() {
            continue;
        }

        let pkg_dir = entry.path().parent().unwrap_or(project_root);
        let content = std::fs::read_to_string(entry.path())?;
        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue, // Skip malformed package.json
        };

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|n| {
                // Strip npm scope prefix (e.g., @scope/name -> name)
                n.rsplit('/').next().unwrap_or(n).to_string()
            })
            .unwrap_or_else(|| {
                pkg_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

        let source_root = if pkg_dir.join("src").is_dir() {
            pkg_dir.join("src")
        } else {
            pkg_dir.to_path_buf()
        };

        let source_files = walk_ts_files(&source_root);

        if !source_files.is_empty() {
            packages.push(TsPackageInfo {
                name,
                root: pkg_dir.to_path_buf(),
                source_root,
                source_files,
            });
        }
    }

    Ok(packages)
}

/// Recursively walk a directory and collect all `.ts`, `.tsx`, and `.svelte` files.
///
/// Skips test files (`*.test.ts`, `*.spec.ts`) and declaration files (`*.d.ts`).
fn walk_ts_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != "node_modules" && name != "__tests__"
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            match ext {
                "ts" | "tsx" => {
                    !filename.ends_with(".test.ts")
                        && !filename.ends_with(".spec.ts")
                        && !filename.ends_with(".test.tsx")
                        && !filename.ends_with(".spec.tsx")
                        && !filename.ends_with(".d.ts")
                }
                "svelte" => true,
                _ => false,
            }
        })
        .map(|e| e.into_path())
        .collect()
}

/// Discover Go modules in a project tree.
///
/// Walks the directory tree looking for `go.mod` files (skipping `vendor/`,
/// `node_modules/`, `target/`). For each module, collects `.go` source files
/// (excluding `_test.go` files and `vendor/` directories).
pub fn discover_go_packages(project_root: &Path) -> Result<Vec<GoPackageInfo>, DiscoveryError> {
    let skip_dirs = ["vendor", "node_modules", "target", ".git", "dist"];
    let mut packages = Vec::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !skip_dirs.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        if entry.file_name() != "go.mod" || !entry.file_type().is_file() {
            continue;
        }

        let mod_dir = entry.path().parent().unwrap_or(project_root);
        let content = std::fs::read_to_string(entry.path())?;

        // Parse module path from first "module" line
        let module_path = content
            .lines()
            .find(|line| line.starts_with("module "))
            .and_then(|line| line.strip_prefix("module "))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if module_path.is_empty() {
            continue;
        }

        let name = module_path
            .rsplit('/')
            .next()
            .unwrap_or(&module_path)
            .to_string();

        let source_files = walk_go_files(mod_dir);

        // Group files by package directory
        let mut pkg_dirs: std::collections::HashMap<PathBuf, Vec<PathBuf>> =
            std::collections::HashMap::new();
        for file in &source_files {
            let dir = file.parent().unwrap_or(mod_dir).to_path_buf();
            pkg_dirs.entry(dir).or_default().push(file.clone());
        }

        let go_packages: Vec<GoPackage> = pkg_dirs
            .into_iter()
            .map(|(dir, files)| {
                let import_path = dir
                    .strip_prefix(mod_dir)
                    .unwrap_or(Path::new(""))
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                GoPackage {
                    import_path,
                    dir,
                    source_files: files,
                }
            })
            .collect();

        if !source_files.is_empty() {
            packages.push(GoPackageInfo {
                module_path,
                name,
                root: mod_dir.to_path_buf(),
                source_files,
                packages: go_packages,
            });
        }
    }

    Ok(packages)
}

/// Recursively walk a directory and collect all `.go` files.
///
/// Skips `_test.go` files and `vendor/` directories.
fn walk_go_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != "vendor" && name != "node_modules" && name != "testdata"
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ext == "go" && !filename.ends_with("_test.go")
        })
        .map(|e| e.into_path())
        .collect()
}

/// Discover Python packages in a project tree.
///
/// Looks for `pyproject.toml` or `setup.py` files (skipping `.venv/`, `venv/`,
/// `__pycache__/`, `node_modules/`, `target/`). For each package found,
/// collects `.py` source files.
pub fn discover_python_packages(
    project_root: &Path,
) -> Result<Vec<PythonPackageInfo>, DiscoveryError> {
    let skip_dirs = [
        ".venv",
        "venv",
        "__pycache__",
        "node_modules",
        "target",
        ".git",
        ".tox",
        "dist",
        "build",
        ".eggs",
    ];
    let mut packages = Vec::new();
    let mut seen_roots: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !skip_dirs.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        let is_pyproject = entry.file_name() == "pyproject.toml";
        let is_setup_py = entry.file_name() == "setup.py";

        if (!is_pyproject && !is_setup_py) || !entry.file_type().is_file() {
            continue;
        }

        let pkg_dir = entry.path().parent().unwrap_or(project_root);
        if !seen_roots.insert(pkg_dir.to_path_buf()) {
            continue; // Already processed this directory
        }

        let name = if is_pyproject {
            parse_pyproject_name(entry.path())
        } else {
            parse_setup_py_name(entry.path())
        }
        .unwrap_or_else(|| {
            pkg_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        // Find source root: prefer src/<pkg_name>/, then <pkg_name>/, then root
        let pkg_name_underscore = name.replace('-', "_");
        let source_root = if pkg_dir.join("src").join(&pkg_name_underscore).is_dir() {
            pkg_dir.join("src").join(&pkg_name_underscore)
        } else if pkg_dir.join(&pkg_name_underscore).is_dir() {
            pkg_dir.join(&pkg_name_underscore)
        } else if pkg_dir.join("src").is_dir() {
            pkg_dir.join("src")
        } else {
            pkg_dir.to_path_buf()
        };

        let source_files = walk_py_files(&source_root);

        if !source_files.is_empty() {
            packages.push(PythonPackageInfo {
                name,
                root: pkg_dir.to_path_buf(),
                source_root,
                source_files,
            });
        }
    }

    Ok(packages)
}

/// Parse package name from `pyproject.toml`.
fn parse_pyproject_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    // Simple TOML parsing: look for name = "..." under [project]
    let mut in_project = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_project = trimmed == "[project]";
            continue;
        }
        if in_project && trimmed.starts_with("name") {
            if let Some(value) = trimmed.split('=').nth(1) {
                let name = value.trim().trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

/// Parse package name from `setup.py`.
fn parse_setup_py_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    // Simple heuristic: look for name='...' or name="..." anywhere in the file
    // Handles both `setup(name='foo')` and `name = 'foo'` formats
    for line in content.lines() {
        let trimmed = line.trim();
        // Find "name" followed by optional whitespace and "="
        if let Some(pos) = trimmed.find("name") {
            let after_name = &trimmed[pos + 4..].trim_start();
            if let Some(rest) = after_name.strip_prefix('=') {
                let value = rest
                    .trim()
                    .trim_end_matches(')')
                    .trim_end_matches(',')
                    .trim_matches('"')
                    .trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Recursively walk a directory and collect all `.py` files.
///
/// Skips `__pycache__/`, `.venv/`, `venv/`, test files.
fn walk_py_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            name != "__pycache__" && name != ".venv" && name != "venv" && name != ".tox"
        })
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            ext == "py"
                && !filename.starts_with("test_")
                && !filename.ends_with("_test.py")
                && filename != "conftest.py"
                && filename != "setup.py"
        })
        .map(|e| e.into_path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn discovers_workspace_crates() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let layout = discover_project(&project_root).unwrap();
        assert!(
            layout.crates.len() >= 4,
            "should find at least svt-core, svt-analyzer, svt-cli, svt-server, got {}",
            layout.crates.len()
        );

        let names: Vec<&str> = layout.crates.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"svt-core"), "should find svt-core");
        assert!(names.contains(&"svt-analyzer"), "should find svt-analyzer");
    }

    #[test]
    fn crate_info_has_source_files() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let layout = discover_project(&project_root).unwrap();

        let core = layout.crates.iter().find(|c| c.name == "svt-core").unwrap();
        assert!(
            !core.source_files.is_empty(),
            "svt-core should have .rs files"
        );
        assert!(
            core.source_files.iter().any(|f| f.ends_with("lib.rs")),
            "should include lib.rs"
        );
    }

    #[test]
    fn crate_type_detected_correctly() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let layout = discover_project(&project_root).unwrap();

        let core = layout.crates.iter().find(|c| c.name == "svt-core").unwrap();
        assert_eq!(core.crate_type, CrateType::Lib);

        // svt-cli always uses package name, even for binary targets
        let cli_bin = layout.crates.iter().find(|c| c.name == "svt-cli");
        assert!(cli_bin.is_some(), "should find CLI crate by package name");
    }

    #[test]
    fn detects_workspace_name_from_common_prefix() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let layout = discover_project(&project_root).unwrap();

        assert_eq!(
            layout.workspace_name,
            Some("svt".to_string()),
            "should detect 'svt' as workspace name from svt-core, svt-cli, etc."
        );
    }

    #[test]
    fn workspace_name_none_for_single_crate() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"single-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

        let layout = discover_project(dir.path()).unwrap();
        assert_eq!(layout.workspace_name, None);
    }

    #[test]
    fn discovers_single_crate_project() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"single-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

        let layout = discover_project(dir.path()).unwrap();
        assert_eq!(layout.crates.len(), 1, "should find exactly 1 crate");
        assert_eq!(layout.crates[0].name, "single-crate");
        assert_eq!(layout.crates[0].crate_type, CrateType::Lib);
    }

    #[test]
    fn non_rust_directory_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = discover_project(dir.path());
        assert!(
            result.is_err(),
            "directory without Cargo.toml should return error"
        );
    }

    #[test]
    fn discovers_ts_package_from_package_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": "my-app"}"#).unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/index.ts"), "export const x = 1;").unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-app");
        assert!(!packages[0].source_files.is_empty());
    }

    #[test]
    fn ts_discovery_skips_node_modules() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": "my-app"}"#).unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/app.ts"), "export const x = 1;").unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules/dep")).unwrap();
        std::fs::write(
            dir.path().join("node_modules/dep/package.json"),
            r#"{"name": "dep"}"#,
        )
        .unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(
            packages.len(),
            1,
            "should only find root package, not node_modules"
        );
    }

    #[test]
    fn ts_discovery_collects_svelte_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": "svelte-app"}"#).unwrap();
        std::fs::create_dir_all(dir.path().join("src/components")).unwrap();
        std::fs::write(
            dir.path().join("src/main.ts"),
            "import App from './App.svelte';",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("src/components/App.svelte"),
            "<script lang=\"ts\">\nlet x = 1;\n</script>",
        )
        .unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        let svelte_files: Vec<_> = packages[0]
            .source_files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e == "svelte"))
            .collect();
        assert!(!svelte_files.is_empty(), "should collect .svelte files");
    }

    #[test]
    fn ts_discovery_skips_test_and_declaration_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"name": "my-app"}"#).unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/app.ts"), "export const x = 1;").unwrap();
        std::fs::write(dir.path().join("src/app.test.ts"), "test('x', () => {});").unwrap();
        std::fs::write(dir.path().join("src/app.spec.ts"), "test('x', () => {});").unwrap();
        std::fs::write(dir.path().join("src/types.d.ts"), "declare module 'x';").unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(
            packages[0].source_files.len(),
            1,
            "should only include app.ts"
        );
    }

    #[test]
    fn ts_discovery_falls_back_to_dir_name_when_no_name_field() {
        let dir = TempDir::new().unwrap();
        let pkg_dir = dir.path().join("my-project");
        std::fs::create_dir_all(pkg_dir.join("src")).unwrap();
        std::fs::write(pkg_dir.join("package.json"), r#"{}"#).unwrap();
        std::fs::write(pkg_dir.join("src/index.ts"), "export const x = 1;").unwrap();

        let packages = discover_ts_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-project");
    }

    #[test]
    fn ts_discovery_returns_empty_when_no_package_json() {
        let dir = TempDir::new().unwrap();
        let packages = discover_ts_packages(dir.path()).unwrap();
        assert!(packages.is_empty());
    }

    // --- Go discovery tests ---

    #[test]
    fn discovers_go_module_from_go_mod() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/user/myapp\n\ngo 1.21\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("cmd/server")).unwrap();
        std::fs::write(
            dir.path().join("main.go"),
            "package main\n\nfunc main() {}\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("cmd/server/server.go"),
            "package server\n\nfunc Run() {}\n",
        )
        .unwrap();

        let packages = discover_go_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].module_path, "github.com/user/myapp");
        assert_eq!(packages[0].name, "myapp");
        assert!(!packages[0].source_files.is_empty());
    }

    #[test]
    fn go_discovery_skips_vendor_and_test_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/app\n\ngo 1.21\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("main.go"), "package main\n").unwrap();
        std::fs::write(dir.path().join("main_test.go"), "package main\n").unwrap();
        std::fs::create_dir_all(dir.path().join("vendor/dep")).unwrap();
        std::fs::write(dir.path().join("vendor/dep/lib.go"), "package dep\n").unwrap();

        let packages = discover_go_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        for f in &packages[0].source_files {
            let name = f.file_name().unwrap().to_str().unwrap();
            assert!(!name.ends_with("_test.go"), "should skip test files");
            assert!(
                !f.to_str().unwrap().contains("vendor"),
                "should skip vendor dir"
            );
        }
    }

    #[test]
    fn go_discovery_returns_empty_when_no_go_mod() {
        let dir = TempDir::new().unwrap();
        let packages = discover_go_packages(dir.path()).unwrap();
        assert!(packages.is_empty());
    }

    // --- Python discovery tests ---

    #[test]
    fn discovers_python_package_from_pyproject_toml() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"my-app\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src/my_app")).unwrap();
        std::fs::write(dir.path().join("src/my_app/__init__.py"), "").unwrap();
        std::fs::write(dir.path().join("src/my_app/core.py"), "def hello(): pass\n").unwrap();

        let packages = discover_python_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "my-app");
        assert!(!packages[0].source_files.is_empty());
    }

    #[test]
    fn discovers_python_package_from_setup_py() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("setup.py"),
            "from setuptools import setup\nsetup(name='legacy-app')\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("legacy_app")).unwrap();
        std::fs::write(dir.path().join("legacy_app/__init__.py"), "").unwrap();
        std::fs::write(dir.path().join("legacy_app/main.py"), "def run(): pass\n").unwrap();

        let packages = discover_python_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "legacy-app");
    }

    #[test]
    fn python_discovery_skips_venv() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"app\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(dir.path().join("app/__init__.py"), "").unwrap();
        std::fs::write(dir.path().join("app/core.py"), "x = 1\n").unwrap();
        std::fs::create_dir_all(dir.path().join(".venv/lib")).unwrap();
        std::fs::write(dir.path().join(".venv/lib/site.py"), "x = 1\n").unwrap();

        let packages = discover_python_packages(dir.path()).unwrap();
        assert_eq!(packages.len(), 1);
        for f in &packages[0].source_files {
            assert!(
                !f.to_str().unwrap().contains(".venv"),
                "should skip .venv dir"
            );
        }
    }

    #[test]
    fn python_discovery_returns_empty_when_no_manifest() {
        let dir = TempDir::new().unwrap();
        let packages = discover_python_packages(dir.path()).unwrap();
        assert!(packages.is_empty());
    }
}
