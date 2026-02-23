//! `svt-core` -- Core data model, CozoDB graph store, validation, and conformance logic.
//!
//! This crate is the foundation of Anamnetron. It compiles to both
//! native and WASM targets, enabling shared logic across CLI, server, and browser.

#![warn(missing_docs)]

/// Canonical path utilities: kebab-case conversion, glob matching, path validation.
pub mod canonical;

/// Interchange format: YAML/JSON parsing, validation, wire types.
pub mod interchange;

/// Graph data model and schema definitions.
pub mod model;

/// Analysis pipeline types: items, relations, warnings, parse results.
pub mod analysis;

/// CozoDB graph store operations.
#[cfg(feature = "store")]
pub mod store;

/// Interchange store operations: load and export.
#[cfg(feature = "store")]
pub mod interchange_store;

/// Conformance evaluation: constraint checking and report generation.
#[cfg(feature = "store")]
pub mod conformance;

/// Snapshot diffing: compute changes between two graph versions.
#[cfg(feature = "store")]
pub mod diff;

/// Export graph data in various formats (Mermaid, JSON, DOT).
#[cfg(feature = "store")]
pub mod export;

/// Plugin API: trait, error types, and the `declare_plugin!` macro.
#[cfg(feature = "store")]
pub mod plugin;

/// Conformance and validation rules.
#[cfg(feature = "store")]
pub mod validation;
