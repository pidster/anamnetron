# ADR-001: Rust as Backend Language

## Status

Accepted

## Context

The tool needs a backend that can:
- Perform fast code analysis across large codebases
- Produce single-binary distributions with no runtime dependencies
- Compile core logic to WASM for browser-side execution
- Provide native tree-sitter integration for multi-language parsing

## Decision

Use Rust as the backend language for all crates (core, analyzer, cli, server).

## Alternatives Considered

- **TypeScript (Node.js)** — unified language with frontend, but weaker performance for code analysis, no native WASM compilation of business logic, tree-sitter via FFI.
- **Python** — strongest static analysis ecosystem, but separate type system from frontend, slower execution, harder to distribute as a single binary.
- **Go** — fast, single binary, but no WASM compilation of libraries, less expressive type system for the graph data model.

## Consequences

- Core logic compiles to both native and WASM targets, sharing validation/conformance across CLI, server, and browser.
- Single binary CLI with no runtime dependencies (usability principle).
- Tree-sitter is a native Rust library — no FFI overhead.
- Frontend types must be generated or bridged via wasm-bindgen/tsify.
- Slower iteration speed than TypeScript, accepted as a trade-off.
