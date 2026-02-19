//! `svt-core` -- Core data model, CozoDB graph store, validation, and conformance logic.
//!
//! This crate is the foundation of software-visualizer-tool. It compiles to both
//! native and WASM targets, enabling shared logic across CLI, server, and browser.

#![warn(missing_docs)]

/// Canonical path utilities: kebab-case conversion, glob matching, path validation.
pub mod canonical;

/// Interchange format: YAML/JSON parsing, validation, wire types.
pub mod interchange;

/// Graph data model and schema definitions.
pub mod model;

/// CozoDB graph store operations.
#[cfg(feature = "store")]
pub mod store;

/// Interchange store operations: load and export.
#[cfg(feature = "store")]
pub mod interchange_store;

/// Conformance evaluation: constraint checking and report generation.
#[cfg(feature = "store")]
pub mod conformance;

/// Export graph data in various formats (Mermaid, JSON).
#[cfg(feature = "store")]
pub mod export;

/// Conformance and validation rules.
#[cfg(feature = "store")]
pub mod validation;
