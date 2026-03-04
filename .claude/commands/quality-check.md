Run a comprehensive quality check across the codebase.

## Steps

1. Run `cargo fmt --check` — verify formatting
2. Run `cargo clippy --all-targets --all-features -- -D warnings` — verify linting
3. Run `cargo test --all-targets --all-features` — run all Rust tests
4. Run `cd web && npm test` — run all web tests
5. Run `cargo audit` — check for dependency vulnerabilities
6. Check for any `unwrap()` or `expect()` outside of test modules in library code
7. Check for any `unsafe` blocks and verify they have justification comments
8. Check for any public items missing doc comments in core/ and analyzer/
9. Report results with pass/fail status for each check

## Output

Quality report:
- **Formatting**: pass/fail
- **Linting**: pass/fail + issue count
- **Rust tests**: pass/fail + test count
- **Web tests**: pass/fail + test count
- **Dependency audit**: pass/fail + vulnerability count
- **Code hygiene**: unwrap/expect/unsafe findings
- **Documentation**: missing doc comments count
- **Overall**: pass/fail with summary
