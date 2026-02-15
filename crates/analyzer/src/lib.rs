//! `svt-analyzer` -- Tree-sitter based code analysis and structure discovery.
//!
//! This crate scans source code using tree-sitter grammars to extract
//! architectural elements (modules, types, functions, dependencies) and
//! populate the core graph model.

#![warn(missing_docs)]

/// Language-specific analysis drivers.
pub mod languages {}

/// Project discovery and file scanning.
pub mod discovery {}
