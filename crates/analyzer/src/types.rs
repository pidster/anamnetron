//! Intermediate types for the analysis pipeline.

use std::path::PathBuf;

use svt_core::model::{EdgeKind, NodeKind};

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
    /// Crate name (e.g., "svt-core").
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
}

/// A code element extracted by tree-sitter (before canonical path mapping).
#[derive(Debug, Clone)]
pub struct AnalysisItem {
    /// Language-specific qualified name (e.g., "svt_core::model::Node").
    pub qualified_name: String,
    /// Abstraction level.
    pub kind: NodeKind,
    /// Language-specific type (e.g., "crate", "module", "struct", "function").
    pub sub_kind: String,
    /// Qualified name of the containment parent, if any.
    pub parent_qualified_name: Option<String>,
    /// Source file and line reference (e.g., "crates/core/src/model/mod.rs:42").
    pub source_ref: String,
    /// Source language.
    pub language: String,
}

/// A relationship between code elements (before canonical path mapping).
#[derive(Debug, Clone)]
pub struct AnalysisRelation {
    /// Qualified name of the source element.
    pub source_qualified_name: String,
    /// Qualified name of the target element.
    pub target_qualified_name: String,
    /// Relationship type.
    pub kind: EdgeKind,
}

/// A warning produced during analysis (non-fatal).
#[derive(Debug, Clone)]
pub struct AnalysisWarning {
    /// Source file and line where the issue was found.
    pub source_ref: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Summary of an analysis run.
#[derive(Debug, Clone)]
pub struct AnalysisSummary {
    /// Version number of the created analysis snapshot.
    pub version: svt_core::model::Version,
    /// Number of crates analyzed.
    pub crates_analyzed: usize,
    /// Number of source files parsed.
    pub files_analyzed: usize,
    /// Number of nodes created in the store.
    pub nodes_created: usize,
    /// Number of edges created in the store.
    pub edges_created: usize,
    /// Warnings produced during analysis.
    pub warnings: Vec<AnalysisWarning>,
}
