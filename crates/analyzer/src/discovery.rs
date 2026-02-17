//! Project discovery: workspace detection, crate enumeration, source file walking.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::types::{CrateInfo, CrateType, ProjectLayout};

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
                .unwrap_or_else(|| entry_point.parent().unwrap().to_path_buf());

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
