//! WASM bridge to `svt-core` for browser-side graph queries.
//!
//! This crate exposes a `WasmStore` struct via `wasm_bindgen` that wraps
//! the CozoDB-backed `GraphStore` implementation, allowing the web frontend
//! to run graph queries entirely in the browser.

#![warn(missing_docs)]

use wasm_bindgen::prelude::*;

/// WASM-accessible graph store backed by an in-memory CozoDB instance.
#[wasm_bindgen]
pub struct WasmStore {
    _placeholder: bool,
}

#[wasm_bindgen]
impl WasmStore {
    /// Create a new empty in-memory store.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmStore, JsError> {
        Ok(WasmStore { _placeholder: true })
    }
}
