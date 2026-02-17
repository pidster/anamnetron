//! `svt-analyzer` -- Tree-sitter based code analysis and structure discovery.
//!
//! This crate scans source code using tree-sitter grammars to extract
//! architectural elements (modules, types, functions, dependencies) and
//! populate the core graph model.

#![warn(missing_docs)]

pub mod discovery;
pub mod languages;
pub mod mapping;
pub mod types;
