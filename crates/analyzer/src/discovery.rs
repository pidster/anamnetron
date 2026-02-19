//! Project discovery: workspace detection, crate enumeration, source file walking.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::types::{CrateInfo, CrateType, ProjectLayout, TsPackageInfo};

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
                name: if crate_type == CrateType::Bin && target.name != package.name {
                    target.name.clone()
                } else {
                    package.name.clone()
                },
                crate_type,
                root: crate_root,
                entry_point,
                source_files,
            });
        }
    }

    Ok(ProjectLayout {
        workspace_root,
        crates,
    })
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

        // svt-cli has a binary target named "svt"
        let cli_bin = layout
            .crates
            .iter()
            .find(|c| c.name == "svt-cli" || c.name == "svt");
        assert!(cli_bin.is_some(), "should find CLI binary crate");
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
}
