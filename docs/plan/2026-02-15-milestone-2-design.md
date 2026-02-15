# Milestone 2: Interchange Format, Conformance, CLI — Design

## Goal

First end-to-end story: author a design YAML, load it into the store, run conformance checks, get a report. Demonstrates the tool's core value proposition.

## Scope

- Canonical path utilities (glob matching, kebab-case conversion, validation)
- Interchange format (YAML/JSON import and export, flat + nested shorthand)
- Conformance evaluation (design-only mode, `must_not_depend` constraint)
- CLI commands (`svt import`, `svt check`)
- Dog-food: load `design/architecture.yaml` and check it

## Approach

All new modules in `crates/core/` (Approach A from brainstorming). Matches the dog-food architecture model. CLI crate gets real commands.

## Canonical Path Utilities

**Module:** `crates/core/src/canonical.rs` — always compiled, WASM-safe.

**Functions:**

- `to_kebab_case(segment: &str) -> String` — converts PascalCase, snake_case, ALLCAPS to kebab-case. Handles acronyms (`HTTPServer` → `http-server`).
- `canonical_path_matches(path: &str, pattern: &str) -> bool` — glob matching: `*` matches one segment, `**` matches any depth.
- `validate_canonical_path(path: &str) -> Result<(), String>` — checks leading `/`, no trailing slash, lowercase kebab-case segments.
- `parent_path(path: &str) -> Option<&str>` — `/a/b/c` → `Some("/a/b")`.
- `path_name(path: &str) -> &str` — last segment of path.

Not in scope: language-specific `to_canonical()` / `from_canonical()` (deferred to analyzer crate).

## Interchange Format

**Modules:**
- `crates/core/src/interchange.rs` — parsing and validation (always compiled, WASM-safe)
- `crates/core/src/interchange_store.rs` — store operations (feature-gated behind `store`)

### Wire Types

```rust
pub struct InterchangeDocument {
    pub format: String,                     // "svt/v1"
    pub kind: SnapshotKind,
    pub version: Option<Version>,
    pub metadata: Option<serde_json::Value>,
    pub nodes: Vec<InterchangeNode>,
    pub edges: Vec<InterchangeEdge>,
    pub constraints: Vec<InterchangeConstraint>,
}
```

`InterchangeNode`, `InterchangeEdge`, `InterchangeConstraint` use canonical paths for edge references (not UUIDs), support optional fields with inference, and support `children` for nested shorthand.

### Functions

- `parse_yaml(input: &str) -> Result<InterchangeDocument>` — flat + nested shorthand, field inference
- `parse_json(input: &str) -> Result<InterchangeDocument>` — flat form only
- `validate_document(doc: &InterchangeDocument) -> Result<Vec<ValidationWarning>>` — format version, path validity, edge consistency
- `load_into_store(store: &mut impl GraphStore, doc: &InterchangeDocument) -> Result<Version>` — creates snapshot, assigns UUIDs, resolves paths, inserts (store feature)
- `export_yaml(store: &impl GraphStore, version: Version) -> Result<String>` — flat YAML export (store feature)
- `export_json(store: &impl GraphStore, version: Version) -> Result<String>` — JSON export (store feature)

### Design Choices

- Edges reference canonical paths, not UUIDs — human-readable and diffable
- Provenance inferred from top-level `kind`
- `name` inferred from last segment of canonical path if omitted
- `sub_kind` defaults to generic for the kind if omitted
- Contains edges generated from nested shorthand
- Format version `svt/v1` checked on parse, unknown versions rejected

## Conformance Evaluation

**Module:** `crates/core/src/conformance.rs` — feature-gated behind `store`.

### Types

```rust
pub struct ConstraintResult {
    pub constraint_name: String,
    pub constraint_kind: String,
    pub status: ConstraintStatus,
    pub severity: Severity,
    pub message: String,
    pub violations: Vec<Violation>,
}

pub enum ConstraintStatus { Pass, Fail, NotEvaluable }

pub struct Violation {
    pub source_path: String,
    pub target_path: Option<String>,
    pub edge_id: Option<String>,
    pub edge_kind: Option<EdgeKind>,
    pub source_ref: Option<String>,
}

pub struct ConformanceReport {
    pub design_version: Version,
    pub analysis_version: Option<Version>,
    pub constraint_results: Vec<ConstraintResult>,
    pub unimplemented: Vec<UnmatchedNode>,
    pub undocumented: Vec<UnmatchedNode>,
    pub summary: ConformanceSummary,
}

pub struct ConformanceSummary {
    pub passed: usize,
    pub failed: usize,
    pub warned: usize,
    pub not_evaluable: usize,
    pub unimplemented: usize,
    pub undocumented: usize,
}
```

### Functions

- `evaluate_design(store, design_version) -> Result<ConformanceReport>` — design-only mode: structural validation + self-check of evaluable constraints. Non-evaluable constraints marked as `NotEvaluable`.
- `evaluate_constraint_must_not_depend(store, constraint, version) -> Result<ConstraintResult>` — evaluates a single must_not_depend constraint using canonical_path_matches.
- `evaluate(store, design_version, analysis_version) -> Result<ConformanceReport>` — stubbed for Milestone 2, returns error "analysis not available".

### How `must_not_depend` Works

1. Get constraints of kind `must_not_depend` for the version
2. Get all nodes for the version
3. Get all `Depends` edges via `get_all_edges`
4. For each constraint: find nodes matching `scope`, find nodes matching `target`, check for forbidden depends edges between them
5. Uses `canonical_path_matches` for glob matching

## CLI Commands

**Crate:** `crates/cli/`

### `svt import <file>`

```
svt import <file>           # Load a design YAML/JSON into the store
    --store <path>          # Store location (default: .svt/store)
```

Flow: open/create CozoStore → read file → detect format by extension → parse → validate → load_into_store → print summary.

### `svt check`

```
svt check                   # Run conformance check
    --store <path>          # Store location (default: .svt/store)
    --design <version>      # Design version (default: latest design)
    --fail-on <severity>    # error (default), warning, info
    --format <format>       # human (default), json
```

Flow: open CozoStore → find latest design version → evaluate_design → format report → exit code based on severity.

**Human-readable output:**
```
Checking design v1...

  PASS  core-no-outward-deps: Core must not depend on analyzer
  PASS  core-no-cli-deps: Core must not depend on CLI
  N/A   cli-has-check-command: must_contain not evaluable (design-only mode)

  Structural validation:
    Containment: acyclic
    Referential integrity: valid

  5 passed, 0 failed, 0 warnings, 3 not evaluable
```

**JSON output:** ConformanceReport struct serialised directly.

**Exit codes:** 0 = no failures at or above fail-on severity, 1 = failures found.

### Store Location

`.svt/store` in current directory by default (SQLite-backed CozoDB). Analogous to `.git/`.

## Dependencies

| Crate | Where | Feature-gated | Justification |
|-------|-------|---------------|---------------|
| `serde_yaml` | svt-core | No (WASM-safe) | YAML parsing for interchange |
| `glob-match` | svt-core | No (WASM-safe) | Glob pattern matching for canonical paths |
| `anyhow` | svt-cli | N/A | Application error handling |

## Feature Gating

```toml
[features]
default = ["store"]
store = ["dep:cozo", "dep:chrono"]
wasm = ["dep:wasm-bindgen"]
```

- `canonical.rs`, `interchange.rs`: always compiled (WASM-safe)
- `interchange_store.rs`, `conformance.rs`, `validation.rs`, `store/`: behind `store` feature

## Testing Strategy

### Canonical tests (unit, in canonical.rs)
- to_kebab_case: PascalCase, snake_case, ALLCAPS, acronyms, no-op
- canonical_path_matches: exact, `*`, `**`, root, no match
- validate_canonical_path: valid, missing `/`, trailing slash, uppercase, empty
- parent_path and path_name: various depths

### Interchange tests (unit + integration)
- Parse flat YAML round-trip
- Parse nested shorthand generates contains edges
- Field inference (name, sub_kind, provenance)
- Invalid format version rejected
- Edge referencing non-existent path caught
- load_into_store assigns UUIDs, creates snapshot
- export_yaml produces re-parseable output
- Dog-food: load design/architecture.yaml, verify counts

### Conformance tests (integration)
- must_not_depend: no violations → Pass, with violation → Fail with evidence
- must_not_depend with glob patterns
- evaluate_design: clean design passes, cyclic contains reported
- Non-evaluable constraints marked correctly
- Dog-food: import architecture.yaml, evaluate_design, all must_not_depend pass

### CLI tests (integration)
- svt import on dog-food model succeeds
- svt check after import succeeds, exit 0
- svt check --format json produces valid JSON
- svt check on empty store gives clear error
- svt import on invalid YAML gives clear error

### Proptest
- N nodes import + export round-trip preserves all
- Random must_not_depend on random graph: violations verified against actual edges
