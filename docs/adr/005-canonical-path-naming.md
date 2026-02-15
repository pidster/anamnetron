# ADR-005: Language-Neutral Canonical Path Naming

## Status

Accepted

## Context

The tool analyses code in multiple languages, each with different naming conventions (Rust `snake_case::paths`, Java `com.example.PascalCase`, Python `snake_case.paths`, C# `PascalCase.Namespaces`, TypeScript `camelCase`). The graph needs a single consistent identity for nodes regardless of source language. Design models need to reference nodes without knowing the implementation language.

## Decision

Use a language-neutral canonical path as the primary node identity:
- Forward-slash separated (`/`)
- Lowercase kebab-case segments
- Derived from the containment hierarchy
- Example: `/payments-service/handlers/create-order`

Each analyzer provides bidirectional mapping (`to_canonical()` / `from_canonical()`). Mapping rules are convention-based in core with per-analyzer overrides.

## Alternatives Considered

- **Language-specific qualified names as identity** — `crate::module::Struct` for Rust, `com.example.Class` for Java. Fails for polyglot projects, forces design models to choose a language, makes cross-language conformance impossible.
- **UUIDs as identity** — opaque, meaningless to humans. Breaks the human-authored design model use case. Diffs are unreadable.
- **Dual identity (qualified name + UUID)** — adds complexity without solving the cross-language problem.

## Consequences

- Design models are language-agnostic — an architect can define structure without knowing the implementation language.
- Conformance matching works across languages in polyglot projects.
- All analyzers must implement the canonical mapping convention.
- Configurable prefix stripping is needed for languages with boilerplate prefixes (Java `com.example.`, C# namespace prefixes).
- Collision handling is needed when different language constructs map to the same canonical path (rare, resolved by node kind).

See [docs/design/CANONICAL_PATH_MAPPING.md](../design/CANONICAL_PATH_MAPPING.md) for the full mapping rules.
