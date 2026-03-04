# Code Review Standards

When reviewing or writing code, verify each change against this checklist.

## Correctness

- Does the code handle all edge cases? (empty inputs, missing data, boundary values)
- Are error paths handled explicitly — not silently ignored?
- Are invariants maintained? (e.g., graph consistency after mutation)
- Is the logic correct under concurrent access if applicable?

## Architecture Compliance

- Does the change respect the dependency flow? (cli/server → analyzer → core, never reverse)
- Does new code in `core/` avoid platform-specific dependencies? (must compile to WASM)
- Are new public APIs documented and justified?
- Is the graph store used as the primary data representation — not bypassed with ad-hoc structures?
- Does the change align with the three-mode model? (design/discovery/conformance)

## Quality

- Are there tests for new functionality — both happy path and error cases?
- Do test names describe the behaviour being tested?
- Is property-based testing used where applicable? (graph ops, serialization, conformance)
- Is the code free of `unwrap()`/`expect()` outside of tests?
- Are new dependencies justified and audited?

## Design

- Is this the simplest solution that meets the requirement?
- Does it follow existing patterns in the codebase rather than introducing new ones?
- Are abstractions earned (used in multiple places) rather than speculative?
- Is the public API surface minimal — expose only what's needed?

## Performance

- Are allocations minimized in hot paths? (prefer `&str` over `String`, use `Cow` when ownership is conditional)
- Are graph queries pushed into the store engine rather than filtered in application code?
- Does large-dataset handling degrade gracefully? (pagination, streaming, progressive rendering)
