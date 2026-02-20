//! Intermediate types for the analysis pipeline.

use std::path::PathBuf;

/// Type of crate (library or binary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateType {
    /// Library crate (`lib.rs` entry point).
    Lib,
    /// Binary crate (`main.rs` entry point).
    Bin,
}

/// Information about a single crate in the project.
#[derive(Debug, Clone)]
pub struct CrateInfo {
    /// Crate name — always the Cargo package name (e.g., "svt-core").
    pub name: String,
    /// Library or binary.
    pub crate_type: CrateType,
    /// Root directory of the crate.
    pub root: PathBuf,
    /// Entry point file (e.g., `src/lib.rs` or `src/main.rs`).
    pub entry_point: PathBuf,
    /// All `.rs` source files under `src/`.
    pub source_files: Vec<PathBuf>,
}

/// Layout of a Rust project (workspace or single crate).
#[derive(Debug, Clone)]
pub struct ProjectLayout {
    /// Workspace root directory.
    pub workspace_root: PathBuf,
    /// All crates in the workspace.
    pub crates: Vec<CrateInfo>,
    /// Detected workspace name from the common crate name prefix.
    ///
    /// For a workspace with crates `svt-core`, `svt-cli`, `svt-server`,
    /// the workspace name is `"svt"`. `None` if no common prefix is detected
    /// or the project is a single-crate project.
    pub workspace_name: Option<String>,
}

/// Information about a TypeScript/JavaScript package.
#[derive(Debug, Clone)]
pub struct TsPackageInfo {
    /// Package name (from package.json "name" field).
    pub name: String,
    /// Root directory of the package (where package.json lives).
    pub root: PathBuf,
    /// Source root directory (typically root/src/).
    pub source_root: PathBuf,
    /// All .ts, .tsx, .svelte source files under the source root.
    pub source_files: Vec<PathBuf>,
}

/// Information about a Go module discovered in the project.
#[derive(Debug, Clone)]
pub struct GoPackageInfo {
    /// Module path from go.mod (e.g., "github.com/user/repo").
    pub module_path: String,
    /// Short name derived from the module path (last segment).
    pub name: String,
    /// Root directory of the module (where go.mod lives).
    pub root: PathBuf,
    /// All .go source files (excluding _test.go and vendor/).
    pub source_files: Vec<PathBuf>,
    /// Go package directories discovered (relative to root).
    pub packages: Vec<GoPackage>,
}

/// A single Go package (directory with .go files).
#[derive(Debug, Clone)]
pub struct GoPackage {
    /// Package import path relative to module (e.g., "cmd/server").
    pub import_path: String,
    /// Directory containing the package's .go files.
    pub dir: PathBuf,
    /// .go source files in this package directory.
    pub source_files: Vec<PathBuf>,
}

/// Information about a Python package discovered in the project.
#[derive(Debug, Clone)]
pub struct PythonPackageInfo {
    /// Package name (from pyproject.toml name field, setup.py, or directory name).
    pub name: String,
    /// Root directory of the package (where pyproject.toml/setup.py lives).
    pub root: PathBuf,
    /// Source root directory (root/src/<name>/ or root/<name>/ or root/).
    pub source_root: PathBuf,
    /// All .py source files under the source root.
    pub source_files: Vec<PathBuf>,
}

// Re-export analysis pipeline types from svt-core.
// These were moved to core so plugin authors can use them.
pub use svt_core::analysis::{AnalysisItem, AnalysisRelation, AnalysisWarning};

/// Summary of an analysis run.
#[derive(Debug, Clone)]
pub struct AnalysisSummary {
    /// Version number of the created analysis snapshot.
    pub version: svt_core::model::Version,
    /// Number of Rust crates analyzed.
    pub crates_analyzed: usize,
    /// Number of TypeScript packages analyzed.
    pub ts_packages_analyzed: usize,
    /// Number of Go modules analyzed.
    pub go_packages_analyzed: usize,
    /// Number of Python packages analyzed.
    pub python_packages_analyzed: usize,
    /// Number of source files parsed.
    pub files_analyzed: usize,
    /// Number of nodes created in the store.
    pub nodes_created: usize,
    /// Number of edges created in the store.
    pub edges_created: usize,
    /// Warnings produced during analysis.
    pub warnings: Vec<AnalysisWarning>,
}
