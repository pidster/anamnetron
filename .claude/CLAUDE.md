# Software Visualizer Tool

A tool for designing, documenting, and validating software architecture. Rust backend with web GUI frontend.

## Principles

See @PRINCIPLES.md for the full set of development principles governing this project.

## Project Structure

Rust workspace (planned):

```
crates/
  core/        — data model, validation, conformance (compiles to WASM)
  analyzer/    — tree-sitter analysis, discovery mode
  cli/         — CLI entry point, export (Mermaid/SVG/PNG)
  server/      — API service (Axum)

web/           — frontend (TypeScript + WASM core)
```

## Three Modes

1. **Design mode** (prescriptive) — define intended architecture, boundaries, allowed dependencies
2. **Discovery mode** (descriptive) — static analysis of real code, deriving the actual architecture
3. **Conformance mode** (comparative) — overlay design onto discovery, detect violations and drift

## Build Commands

```bash
cargo build              # Build all crates
cargo test               # Run all tests
cargo clippy             # Lint
cargo fmt --check        # Format check
cargo audit              # Dependency audit
```

## Coding Standards

- Rust 2021 edition
- `clippy` and `rustfmt` enforced (pre-commit hooks)
- Public APIs require documentation (`#[warn(missing_docs)]`)
- Property-based tests for graph operations (proptest)
- All layers target high test coverage

## Conventions

- Prefer returning `Result` over panicking
- Use `thiserror` for library error types, `anyhow` for application error types
- Minimize dependencies — each dependency must be justified
- No `unsafe` without documented justification and review
