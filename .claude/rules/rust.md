# Rust Conventions

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
