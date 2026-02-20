# Rust Conventions

## Build Commands

This is a Rust project. Always use `cargo` commands, never `go`, `npm`, `pip`, or other language toolchains:

```bash
cargo build              # Build all crates
cargo test               # Run all tests
cargo clippy             # Lint
cargo fmt --check        # Format check
cargo audit              # Dependency audit
```

Package-specific variants are acceptable when targeting a single crate:

```bash
cargo test -p svt-core   # Test a specific crate
cargo clippy -p svt-cli  # Lint a specific crate
```

The project contains Go, TypeScript, and Python analyzers in `crates/analyzer/src/languages/` — these are tree-sitter parsers for analyzing other languages' source code. They do NOT mean this project uses Go/TypeScript/Python toolchains.

## Code Style

- Use Rust 2021 edition
- All public types, functions, and modules must have doc comments
- Prefer `Result<T, E>` over panicking — `unwrap()` and `expect()` are only acceptable in tests
- Use `thiserror` for error types in library crates, `anyhow` for the CLI/application crate
- No `unsafe` blocks without documented justification and explicit review
- Prefer zero-copy where practical (`&str` over `String`, `Cow<'_, str>` when ownership is conditional)
- Use `#[must_use]` on functions where ignoring the return value is likely a bug
- Derive `Debug` on all public types
- Minimize trait objects — prefer generics with trait bounds unless dynamic dispatch is required
- Keep dependency count low — justify each new crate addition
