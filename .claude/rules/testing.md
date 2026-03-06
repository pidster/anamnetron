# Testing Standards

- **>95% region coverage is mandatory** — new code must maintain or improve coverage (check with `cargo llvm-cov`)
- Thoroughness is preferred over speed of delivery
- Unit tests live alongside the code in `#[cfg(test)]` modules
- Integration tests live in `tests/` directories within each crate
- Use property-based testing (proptest) for graph operations, serialization round-trips, and conformance logic
- Test names describe the behaviour being tested, not the function name: `#[test] fn cyclic_dependency_is_detected()`
- Use `assert_eq!` with descriptive messages where the failure cause isn't obvious
- Test both happy paths and error cases
- No `#[ignore]` without an accompanying issue or TODO explaining why
- Snapshot testing (insta) for complex output like Mermaid/SVG generation where appropriate
