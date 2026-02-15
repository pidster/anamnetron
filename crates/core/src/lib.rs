//! `svt-core` -- Core data model, CozoDB graph store, validation, and conformance logic.
//!
//! This crate is the foundation of software-visualizer-tool. It compiles to both
//! native and WASM targets, enabling shared logic across CLI, server, and browser.

#![warn(missing_docs)]

/// Graph data model and schema definitions.
pub mod model;

/// CozoDB graph store operations.
pub mod store;

/// Conformance and validation rules.
pub mod validation;
