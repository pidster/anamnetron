# Milestone 2: Interchange, Conformance, CLI — Implementation Plan

## Status: COMPLETE

Completed: 2026-02-15

---

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** First end-to-end story: author a design YAML, load it into the store, run conformance checks, get a report.

**Architecture:** Four new modules in `crates/core/` plus real CLI commands. `canonical.rs` and `interchange.rs` are always compiled (WASM-safe). `interchange_store.rs` and `conformance.rs` are feature-gated behind `store`. CLI gets `svt import` and `svt check` subcommands.

**Tech Stack:** Rust 2021, serde_yaml 0.9, glob-match 0.2, existing svt-core + CozoDB stack.

**Design doc:** `docs/plan/2026-02-15-milestone-2-design.md`

---

## Task Dependency Graph

```
Task 0  (deps)
├── Task 1  (get_all_nodes)
├── Task 2  (to_kebab_case) → Task 3 (validate/parent/name) → Task 4 (path_matches) → Task 5 (wire canonical)
└── Task 6  (interchange types)
        └── Task 7 (parse_yaml flat) → Task 8 (nested + inference) → Task 9 (parse_json + validate, needs Task 5)

Task 1 + Task 9 → Task 10 (load_into_store) → Task 11 (export)
Task 1 + Task 5 → Task 12 (conformance must_not_depend) → Task 13 (evaluate_design)
Task 10 + Task 13 → Task 14 (CLI import) → Task 15 (CLI check)
Task 15 → Task 16 (dog-food integration)
Task 11 → Task 17 (proptest round-trips)
Task 16 + Task 17 → Task 18 (final cleanup)
```

**Parallelism opportunities:** Tasks 2-5 (canonical) and Tasks 6-8 (interchange types/parse) can run on separate agents after Task 0. Task 1 can also run in parallel with both tracks.

---

### Task 0: Add serde_yaml and glob-match dependencies

**Files:**
- Modify: `crates/core/Cargo.toml`

**Step 1: Add dependencies**

Add to `[dependencies]` in `crates/core/Cargo.toml`:

```toml
serde_yaml = "0.9"
glob-match = "0.2"
```

**Step 2: Verify it compiles**

Run: `cargo check -p svt-core`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/core/Cargo.toml
git commit -m "Add serde_yaml and glob-match dependencies"
```

---

### Task 1: Add get_all_nodes to GraphStore trait and CozoDB implementation

**Files:**
- Modify: `crates/core/src/store/mod.rs`
- Modify: `crates/core/src/store/cozo.rs`
- Test: inline in `crates/core/src/store/cozo.rs` (existing test module)

**Depends on:** Task 0

**Step 1: Write the failing test**

Add to the test module at the bottom of `crates/core/src/store/cozo.rs`:

```rust
#[test]
fn get_all_nodes_returns_all_nodes_for_version() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();

    let n1 = make_node("n1", "/app", NodeKind::System, "workspace");
    let n2 = make_node("n2", "/app/api", NodeKind::Component, "module");
    store.add_node(v, &n1).unwrap();
    store.add_node(v, &n2).unwrap();

    let all = store.get_all_nodes(v).unwrap();
    assert_eq!(all.len(), 2);

    let paths: Vec<&str> = all.iter().map(|n| n.canonical_path.as_str()).collect();
    assert!(paths.contains(&"/app"));
    assert!(paths.contains(&"/app/api"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core get_all_nodes_returns_all`
Expected: FAIL — method `get_all_nodes` not found

**Step 3: Add trait method**

In `crates/core/src/store/mod.rs`, add after `get_all_edges`:

```rust
    /// Get all nodes for a version.
    fn get_all_nodes(&self, version: Version) -> Result<Vec<Node>>;
```

**Step 4: Implement in CozoDB**

In `crates/core/src/store/cozo.rs`, add to the `impl GraphStore for CozoStore` block:

```rust
    fn get_all_nodes(&self, version: Version) -> Result<Vec<Node>> {
        let query = "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] := *nodes{id, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata}";
        let params = BTreeMap::from([("version".to_string(), DataValue::from(version as i64))]);
        let result = self.run_query(query, params)?;
        result
            .rows
            .iter()
            .map(|row| self.row_to_node(row))
            .collect()
    }
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p svt-core get_all_nodes_returns_all`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/core/src/store/mod.rs crates/core/src/store/cozo.rs
git commit -m "Add get_all_nodes to GraphStore trait"
```

---

### Task 2: canonical.rs — to_kebab_case

**Files:**
- Create: `crates/core/src/canonical.rs`
- Test: inline `#[cfg(test)]` module

**Depends on:** Task 0

**Step 1: Write the failing tests**

Create `crates/core/src/canonical.rs`:

```rust
//! Canonical path utilities: kebab-case conversion, glob matching, path validation.
//!
//! All functions are WASM-safe — no platform-specific dependencies.

/// Convert a segment from PascalCase, snake_case, or ALLCAPS to kebab-case.
///
/// Handles acronyms: `HTTPServer` becomes `http-server`.
pub fn to_kebab_case(_segment: &str) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_from_pascal_case() {
        assert_eq!(to_kebab_case("PaymentService"), "payment-service");
    }

    #[test]
    fn kebab_from_snake_case() {
        assert_eq!(to_kebab_case("payment_service"), "payment-service");
    }

    #[test]
    fn kebab_from_allcaps() {
        assert_eq!(to_kebab_case("ALLCAPS"), "allcaps");
    }

    #[test]
    fn kebab_from_acronym_prefix() {
        assert_eq!(to_kebab_case("HTTPServer"), "http-server");
    }

    #[test]
    fn kebab_from_mixed_camel_acronym() {
        assert_eq!(to_kebab_case("getHTTPClient"), "get-http-client");
    }

    #[test]
    fn kebab_noop_for_already_kebab() {
        assert_eq!(to_kebab_case("already-kebab"), "already-kebab");
    }

    #[test]
    fn kebab_single_lowercase_word() {
        assert_eq!(to_kebab_case("core"), "core");
    }
}
```

**Step 2: Wire module to lib.rs**

In `crates/core/src/lib.rs`, add before the `model` module:

```rust
/// Canonical path utilities: kebab-case conversion, glob matching, path validation.
pub mod canonical;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-core kebab`
Expected: FAIL — `todo!()` panics

**Step 4: Implement to_kebab_case**

Replace the `todo!()` implementation:

```rust
/// Convert a segment from PascalCase, snake_case, or ALLCAPS to kebab-case.
///
/// Handles acronyms: `HTTPServer` becomes `http-server`.
#[must_use]
pub fn to_kebab_case(segment: &str) -> String {
    let mut result = String::with_capacity(segment.len() + 4);
    let chars: Vec<char> = segment.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        // Replace underscores and preserve existing hyphens as separators
        if c == '_' || c == '-' {
            if !result.is_empty() && !result.ends_with('-') {
                result.push('-');
            }
            continue;
        }

        if c.is_uppercase() {
            let prev_lower = i > 0 && chars[i - 1].is_lowercase();
            let prev_upper = i > 0 && chars[i - 1].is_uppercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();

            // Split before: lowercase→uppercase (camelCase) or acronym→word (HTTPServer)
            if prev_lower || (prev_upper && next_lower) {
                if !result.is_empty() && !result.ends_with('-') {
                    result.push('-');
                }
            }
        }

        result.push(c.to_ascii_lowercase());
    }

    result
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-core kebab`
Expected: all 7 tests PASS

**Step 6: Commit**

```bash
git add crates/core/src/canonical.rs crates/core/src/lib.rs
git commit -m "Add to_kebab_case canonical path utility"
```

---

### Task 3: canonical.rs — validate_canonical_path, parent_path, path_name

**Files:**
- Modify: `crates/core/src/canonical.rs`

**Depends on:** Task 2

**Step 1: Write the failing tests**

Add to the `tests` module in `canonical.rs`:

```rust
    // --- validate_canonical_path ---

    #[test]
    fn valid_canonical_path() {
        assert!(validate_canonical_path("/svt/core/model").is_ok());
    }

    #[test]
    fn valid_root_level_path() {
        assert!(validate_canonical_path("/svt").is_ok());
    }

    #[test]
    fn invalid_missing_leading_slash() {
        assert!(validate_canonical_path("svt/core").is_err());
    }

    #[test]
    fn invalid_trailing_slash() {
        assert!(validate_canonical_path("/svt/core/").is_err());
    }

    #[test]
    fn invalid_uppercase_segment() {
        assert!(validate_canonical_path("/svt/Core").is_err());
    }

    #[test]
    fn invalid_empty_segment() {
        assert!(validate_canonical_path("/svt//core").is_err());
    }

    #[test]
    fn valid_path_with_digits() {
        assert!(validate_canonical_path("/svt/v2/core").is_ok());
    }

    // --- parent_path ---

    #[test]
    fn parent_of_deep_path() {
        assert_eq!(parent_path("/a/b/c"), Some("/a/b"));
    }

    #[test]
    fn parent_of_two_segment_path() {
        assert_eq!(parent_path("/a/b"), Some("/a"));
    }

    #[test]
    fn parent_of_root_level_path() {
        assert_eq!(parent_path("/a"), None);
    }

    // --- path_name ---

    #[test]
    fn name_of_deep_path() {
        assert_eq!(path_name("/a/b/c"), "c");
    }

    #[test]
    fn name_of_root_level_path() {
        assert_eq!(path_name("/a"), "a");
    }
```

Add stub functions above the `tests` module:

```rust
/// Validate that a canonical path is well-formed.
///
/// Requirements: leading `/`, no trailing slash, lowercase kebab-case segments,
/// no empty segments.
pub fn validate_canonical_path(_path: &str) -> Result<(), String> {
    todo!()
}

/// Get the parent path. Returns `None` for root-level paths (e.g., `/a`).
pub fn parent_path(_path: &str) -> Option<&str> {
    todo!()
}

/// Get the last segment of a canonical path.
pub fn path_name(_path: &str) -> &str {
    todo!()
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core canonical::tests::valid_canonical`
Expected: FAIL — `todo!()` panics

**Step 3: Implement the three functions**

Replace the stubs:

```rust
/// Validate that a canonical path is well-formed.
///
/// Requirements: leading `/`, no trailing slash, lowercase kebab-case segments,
/// no empty segments.
pub fn validate_canonical_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') {
        return Err("must start with '/'".to_string());
    }
    if path.len() > 1 && path.ends_with('/') {
        return Err("must not end with '/'".to_string());
    }
    let segments: Vec<&str> = path[1..].split('/').collect();
    for segment in &segments {
        if segment.is_empty() {
            return Err("empty segment (double slash)".to_string());
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!("segment '{}' is not lowercase kebab-case", segment));
        }
    }
    Ok(())
}

/// Get the parent path. Returns `None` for root-level paths (e.g., `/a`).
#[must_use]
pub fn parent_path(path: &str) -> Option<&str> {
    let last_slash = path.rfind('/')?;
    if last_slash == 0 {
        None
    } else {
        Some(&path[..last_slash])
    }
}

/// Get the last segment of a canonical path.
#[must_use]
pub fn path_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core canonical::tests`
Expected: all canonical tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/canonical.rs
git commit -m "Add validate_canonical_path, parent_path, path_name"
```

---

### Task 4: canonical.rs — canonical_path_matches

**Files:**
- Modify: `crates/core/src/canonical.rs`

**Depends on:** Task 3

**Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    // --- canonical_path_matches ---

    #[test]
    fn matches_exact_path() {
        assert!(canonical_path_matches("/svt/core", "/svt/core"));
    }

    #[test]
    fn matches_star_one_segment() {
        assert!(canonical_path_matches("/svt/core/model", "/svt/*/model"));
    }

    #[test]
    fn star_does_not_match_multiple_segments() {
        assert!(!canonical_path_matches("/svt/core/store/cozo", "/svt/*/cozo"));
    }

    #[test]
    fn matches_globstar_any_depth() {
        assert!(canonical_path_matches("/svt/core/model", "/svt/**"));
    }

    #[test]
    fn globstar_matches_deeply_nested() {
        assert!(canonical_path_matches("/svt/core/store/cozo", "/svt/core/**"));
    }

    #[test]
    fn globstar_matches_immediate_child() {
        assert!(canonical_path_matches("/svt/core", "/svt/**"));
    }

    #[test]
    fn no_match_different_path() {
        assert!(!canonical_path_matches("/svt/analyzer", "/svt/core/**"));
    }

    #[test]
    fn root_pattern_matches_root() {
        assert!(canonical_path_matches("/svt", "/svt"));
    }
```

Add stub function:

```rust
/// Check whether a canonical path matches a glob pattern.
///
/// `*` matches one segment, `**` matches any depth.
pub fn canonical_path_matches(_path: &str, _pattern: &str) -> bool {
    todo!()
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core matches_exact`
Expected: FAIL — `todo!()` panics

**Step 3: Implement**

Replace the stub:

```rust
/// Check whether a canonical path matches a glob pattern.
///
/// `*` matches one segment, `**` matches any depth.
#[must_use]
pub fn canonical_path_matches(path: &str, pattern: &str) -> bool {
    glob_match::glob_match(pattern, path)
}
```

Add the import at the top of the file (after the module doc comment):

```rust
// No `use` needed — glob_match is called with full path
```

Actually, no import needed since we use the full path `glob_match::glob_match`.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core canonical::tests`
Expected: all tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/canonical.rs
git commit -m "Add canonical_path_matches with glob support"
```

---

### Task 5: Wire canonical module and run clippy

**Files:**
- Modify: `crates/core/src/lib.rs` (already done in Task 2 — verify it's wired)

**Depends on:** Task 4

**Step 1: Verify canonical module is wired in lib.rs**

`crates/core/src/lib.rs` should already contain `pub mod canonical;` from Task 2. Verify.

**Step 2: Run full test suite + clippy**

Run: `cargo test -p svt-core && cargo clippy -p svt-core -- -D warnings`
Expected: all tests pass, no clippy warnings

**Step 3: Commit (if any fixes needed)**

```bash
git add crates/core/src/
git commit -m "Wire canonical module, clippy clean"
```

---

### Task 6: Interchange wire types and error type

**Files:**
- Create: `crates/core/src/interchange.rs`

**Depends on:** Task 0

**Step 1: Create interchange.rs with types**

Create `crates/core/src/interchange.rs`:

```rust
//! Interchange format: YAML/JSON import and export wire types.
//!
//! This module defines the serialization types and parsing functions for
//! the `svt/v1` interchange format. Always compiled, WASM-safe.

use serde::{Deserialize, Serialize};

use crate::model::{EdgeKind, NodeKind, Provenance, Severity, SnapshotKind, Version};

/// Errors during interchange parsing or validation.
#[derive(Debug, thiserror::Error)]
pub enum InterchangeError {
    /// YAML or JSON parse error.
    #[error("parse error: {0}")]
    Parse(String),

    /// Unsupported format version.
    #[error("unsupported format: expected 'svt/v1', got '{0}'")]
    UnsupportedFormat(String),

    /// Document validation error.
    #[error("validation error: {0}")]
    Validation(String),
}

/// A warning produced during document validation (non-fatal).
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// The path or element the warning relates to.
    pub path: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Top-level interchange document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeDocument {
    /// Format version string, must be `"svt/v1"`.
    pub format: String,
    /// Snapshot kind (design, analysis, import).
    pub kind: SnapshotKind,
    /// Optional version number (informational).
    pub version: Option<Version>,
    /// Optional metadata.
    pub metadata: Option<serde_json::Value>,
    /// Node definitions (may contain nested children).
    #[serde(default)]
    pub nodes: Vec<InterchangeNode>,
    /// Edge definitions (canonical path references).
    #[serde(default)]
    pub edges: Vec<InterchangeEdge>,
    /// Constraint definitions.
    #[serde(default)]
    pub constraints: Vec<InterchangeConstraint>,
}

/// A node in the interchange format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeNode {
    /// Canonical path (required).
    pub canonical_path: String,
    /// Node kind (required).
    pub kind: NodeKind,
    /// Human-readable name. Inferred from last path segment if omitted.
    pub name: Option<String>,
    /// Language-specific type. Defaults to generic for the kind if omitted.
    pub sub_kind: Option<String>,
    /// Language-specific qualified name.
    pub qualified_name: Option<String>,
    /// Source language.
    pub language: Option<String>,
    /// Provenance. Inferred from document kind if omitted.
    pub provenance: Option<Provenance>,
    /// File path or URL reference.
    pub source_ref: Option<String>,
    /// Extensible metadata.
    pub metadata: Option<serde_json::Value>,
    /// Nested children (shorthand for containment).
    #[serde(default)]
    pub children: Option<Vec<InterchangeNode>>,
}

/// An edge in the interchange format. References canonical paths, not UUIDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeEdge {
    /// Source node canonical path.
    pub source: String,
    /// Target node canonical path.
    pub target: String,
    /// Edge kind.
    pub kind: EdgeKind,
    /// Extensible metadata.
    pub metadata: Option<serde_json::Value>,
}

/// A constraint in the interchange format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeConstraint {
    /// Human-readable name.
    pub name: String,
    /// Constraint kind (e.g., "must_not_depend", "boundary").
    pub kind: String,
    /// Scope pattern (canonical path glob).
    pub scope: String,
    /// Target pattern (for dependency constraints).
    pub target: Option<String>,
    /// Additional parameters.
    pub params: Option<serde_json::Value>,
    /// Description shown on violation.
    pub message: String,
    /// Severity. Defaults to Error if omitted.
    pub severity: Option<Severity>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interchange_node_deserialises_from_yaml() {
        let yaml = r#"
canonical_path: /svt/core
kind: service
sub_kind: crate
name: core
"#;
        let node: InterchangeNode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(node.canonical_path, "/svt/core");
        assert_eq!(node.kind, NodeKind::Service);
        assert_eq!(node.sub_kind, Some("crate".to_string()));
    }

    #[test]
    fn interchange_node_optional_fields_default_to_none() {
        let yaml = r#"
canonical_path: /svt/core
kind: service
"#;
        let node: InterchangeNode = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(node.name, None);
        assert_eq!(node.sub_kind, None);
        assert_eq!(node.children, None);
    }

    #[test]
    fn interchange_edge_deserialises_from_yaml() {
        let yaml = r#"
source: /svt/cli
target: /svt/core
kind: depends
"#;
        let edge: InterchangeEdge = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(edge.source, "/svt/cli");
        assert_eq!(edge.kind, EdgeKind::Depends);
    }

    #[test]
    fn interchange_constraint_deserialises_severity() {
        let yaml = r#"
name: no-outward
kind: must_not_depend
scope: /svt/core/**
target: /svt/cli/**
message: "Core must not depend on CLI"
severity: error
"#;
        let c: InterchangeConstraint = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(c.severity, Some(Severity::Error));
    }
}
```

**Step 2: Wire module to lib.rs**

In `crates/core/src/lib.rs`, add:

```rust
/// Interchange format: YAML/JSON parsing, validation, wire types.
pub mod interchange;
```

**Step 3: Run tests**

Run: `cargo test -p svt-core interchange::tests`
Expected: all 4 tests PASS

**Step 4: Commit**

```bash
git add crates/core/src/interchange.rs crates/core/src/lib.rs
git commit -m "Add interchange wire types and error type"
```

---

### Task 7: parse_yaml — flat form

**Files:**
- Modify: `crates/core/src/interchange.rs`

**Depends on:** Task 6

**Step 1: Write the failing test**

Add to the `tests` module in `interchange.rs`:

```rust
    #[test]
    fn parse_yaml_flat_document() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
  - canonical_path: /app/api
    kind: component
    sub_kind: module
    name: api
edges:
  - source: /app/api
    target: /app
    kind: contains
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(doc.format, "svt/v1");
        assert_eq!(doc.kind, SnapshotKind::Design);
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(doc.edges.len(), 1);
    }

    #[test]
    fn parse_yaml_rejects_unknown_format() {
        let yaml = r#"
format: svt/v99
kind: design
nodes: []
"#;
        let err = parse_yaml(yaml).unwrap_err();
        assert!(err.to_string().contains("svt/v99"));
    }
```

Add stub function (above `#[cfg(test)]`):

```rust
/// Parse a YAML string into an interchange document.
///
/// Checks the format version and flattens nested children into
/// explicit nodes and `Contains` edges.
pub fn parse_yaml(_input: &str) -> Result<InterchangeDocument, InterchangeError> {
    todo!()
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core parse_yaml_flat`
Expected: FAIL — `todo!()` panics

**Step 3: Implement parse_yaml (flat form only)**

Replace the stub:

```rust
/// Parse a YAML string into an interchange document.
///
/// Checks the format version and flattens nested children into
/// explicit nodes and `Contains` edges.
pub fn parse_yaml(input: &str) -> Result<InterchangeDocument, InterchangeError> {
    let mut doc: InterchangeDocument =
        serde_yaml::from_str(input).map_err(|e| InterchangeError::Parse(e.to_string()))?;

    if doc.format != "svt/v1" {
        return Err(InterchangeError::UnsupportedFormat(doc.format));
    }

    // Flatten nested children (generates Contains edges)
    let (flat_nodes, contains_edges) = flatten_nodes(&doc.nodes);
    doc.nodes = flat_nodes;
    doc.edges.extend(contains_edges);

    Ok(doc)
}

/// Recursively flatten nested children into a flat node list and Contains edges.
fn flatten_nodes(nodes: &[InterchangeNode]) -> (Vec<InterchangeNode>, Vec<InterchangeEdge>) {
    let mut flat = Vec::new();
    let mut edges = Vec::new();

    fn recurse(
        node: &InterchangeNode,
        flat: &mut Vec<InterchangeNode>,
        edges: &mut Vec<InterchangeEdge>,
    ) {
        // Add this node without children
        let mut flat_node = node.clone();
        flat_node.children = None;
        flat.push(flat_node);

        if let Some(children) = &node.children {
            for child in children {
                edges.push(InterchangeEdge {
                    source: node.canonical_path.clone(),
                    target: child.canonical_path.clone(),
                    kind: EdgeKind::Contains,
                    metadata: None,
                });
                recurse(child, flat, edges);
            }
        }
    }

    for node in nodes {
        recurse(node, &mut flat, &mut edges);
    }

    (flat, edges)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core parse_yaml`
Expected: both parse_yaml tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/interchange.rs
git commit -m "Add parse_yaml with flat form and format validation"
```

---

### Task 8: parse_yaml — nested shorthand and field inference

**Files:**
- Modify: `crates/core/src/interchange.rs`

**Depends on:** Task 7

**Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn parse_yaml_nested_generates_contains_edges() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
    children:
      - canonical_path: /app/api
        kind: component
        sub_kind: module
        name: api
      - canonical_path: /app/db
        kind: component
        sub_kind: module
        name: db
edges: []
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(doc.nodes.len(), 3, "should flatten to 3 nodes");
        // 2 contains edges generated from children
        let contains: Vec<_> = doc
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Contains)
            .collect();
        assert_eq!(contains.len(), 2);
        assert_eq!(contains[0].source, "/app");
        assert_eq!(contains[0].target, "/app/api");
    }

    #[test]
    fn parse_yaml_deeply_nested_children() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
        children:
          - canonical_path: /app/core/model
            kind: component
"#;
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(doc.nodes.len(), 3);
        assert_eq!(doc.edges.len(), 2, "should have 2 contains edges");
    }
```

**Step 2: Run tests to verify they pass**

These tests should already pass since `flatten_nodes` was implemented in Task 7.

Run: `cargo test -p svt-core nested_generates`
Expected: PASS (already implemented)

Run: `cargo test -p svt-core deeply_nested`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/core/src/interchange.rs
git commit -m "Add tests for nested shorthand parsing"
```

---

### Task 9: parse_json and validate_document

**Files:**
- Modify: `crates/core/src/interchange.rs`

**Depends on:** Task 8, Task 5 (canonical module needed for validate_document)

**Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn parse_json_flat_document() {
        let json = r#"{
            "format": "svt/v1",
            "kind": "design",
            "nodes": [
                {"canonical_path": "/app", "kind": "system", "sub_kind": "workspace", "name": "app"}
            ],
            "edges": [],
            "constraints": []
        }"#;
        let doc = parse_json(json).unwrap();
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.kind, SnapshotKind::Design);
    }

    #[test]
    fn parse_json_rejects_unknown_format() {
        let json = r#"{"format": "svt/v99", "kind": "design", "nodes": [], "edges": [], "constraints": []}"#;
        let err = parse_json(json).unwrap_err();
        assert!(err.to_string().contains("svt/v99"));
    }

    #[test]
    fn validate_catches_duplicate_paths() {
        let doc = InterchangeDocument {
            format: "svt/v1".to_string(),
            kind: SnapshotKind::Design,
            version: None,
            metadata: None,
            nodes: vec![
                InterchangeNode {
                    canonical_path: "/app".to_string(),
                    kind: NodeKind::System,
                    name: Some("app".to_string()),
                    sub_kind: None,
                    qualified_name: None,
                    language: None,
                    provenance: None,
                    source_ref: None,
                    metadata: None,
                    children: None,
                },
                InterchangeNode {
                    canonical_path: "/app".to_string(),
                    kind: NodeKind::System,
                    name: Some("app2".to_string()),
                    sub_kind: None,
                    qualified_name: None,
                    language: None,
                    provenance: None,
                    source_ref: None,
                    metadata: None,
                    children: None,
                },
            ],
            edges: vec![],
            constraints: vec![],
        };
        let err = validate_document(&doc).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn validate_catches_invalid_canonical_path() {
        let doc = InterchangeDocument {
            format: "svt/v1".to_string(),
            kind: SnapshotKind::Design,
            version: None,
            metadata: None,
            nodes: vec![InterchangeNode {
                canonical_path: "no-leading-slash".to_string(),
                kind: NodeKind::System,
                name: None,
                sub_kind: None,
                qualified_name: None,
                language: None,
                provenance: None,
                source_ref: None,
                metadata: None,
                children: None,
            }],
            edges: vec![],
            constraints: vec![],
        };
        let err = validate_document(&doc).unwrap_err();
        assert!(err.to_string().contains("invalid canonical path"));
    }

    #[test]
    fn validate_warns_on_missing_edge_target() {
        let doc = InterchangeDocument {
            format: "svt/v1".to_string(),
            kind: SnapshotKind::Design,
            version: None,
            metadata: None,
            nodes: vec![InterchangeNode {
                canonical_path: "/app".to_string(),
                kind: NodeKind::System,
                name: None,
                sub_kind: None,
                qualified_name: None,
                language: None,
                provenance: None,
                source_ref: None,
                metadata: None,
                children: None,
            }],
            edges: vec![InterchangeEdge {
                source: "/app".to_string(),
                target: "/nonexistent".to_string(),
                kind: EdgeKind::Depends,
                metadata: None,
            }],
            constraints: vec![],
        };
        let warnings = validate_document(&doc).unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("/nonexistent"));
    }
```

Add stub functions:

```rust
/// Parse a JSON string into an interchange document (flat form only).
pub fn parse_json(_input: &str) -> Result<InterchangeDocument, InterchangeError> {
    todo!()
}

/// Validate a parsed interchange document.
///
/// Returns warnings for non-fatal issues. Returns `Err` for fatal problems
/// like duplicate paths or invalid canonical paths.
pub fn validate_document(
    _doc: &InterchangeDocument,
) -> Result<Vec<ValidationWarning>, InterchangeError> {
    todo!()
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core parse_json_flat`
Expected: FAIL

**Step 3: Implement parse_json**

```rust
/// Parse a JSON string into an interchange document (flat form only).
pub fn parse_json(input: &str) -> Result<InterchangeDocument, InterchangeError> {
    let doc: InterchangeDocument =
        serde_json::from_str(input).map_err(|e| InterchangeError::Parse(e.to_string()))?;

    if doc.format != "svt/v1" {
        return Err(InterchangeError::UnsupportedFormat(doc.format));
    }

    Ok(doc)
}
```

**Step 4: Implement validate_document**

```rust
/// Validate a parsed interchange document.
///
/// Returns warnings for non-fatal issues. Returns `Err` for fatal problems
/// like duplicate paths or invalid canonical paths.
pub fn validate_document(
    doc: &InterchangeDocument,
) -> Result<Vec<ValidationWarning>, InterchangeError> {
    use std::collections::HashSet;

    let mut warnings = Vec::new();
    let mut seen_paths = HashSet::new();

    for node in &doc.nodes {
        // Check for valid canonical paths
        if let Err(msg) = crate::canonical::validate_canonical_path(&node.canonical_path) {
            return Err(InterchangeError::Validation(format!(
                "invalid canonical path '{}': {}",
                node.canonical_path, msg
            )));
        }

        // Check for duplicates
        if !seen_paths.insert(&node.canonical_path) {
            return Err(InterchangeError::Validation(format!(
                "duplicate canonical path: {}",
                node.canonical_path
            )));
        }
    }

    // Check edge references
    for edge in &doc.edges {
        if !seen_paths.contains(&edge.source) {
            warnings.push(ValidationWarning {
                path: edge.source.clone(),
                message: format!("edge source '{}' not found in nodes", edge.source),
            });
        }
        if !seen_paths.contains(&edge.target) {
            warnings.push(ValidationWarning {
                path: edge.target.clone(),
                message: format!("edge target '{}' not found in nodes", edge.target),
            });
        }
    }

    Ok(warnings)
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-core interchange::tests`
Expected: all interchange tests PASS

**Step 6: Commit**

```bash
git add crates/core/src/interchange.rs
git commit -m "Add parse_json and validate_document"
```

---

### Task 10: interchange_store.rs — load_into_store

**Files:**
- Create: `crates/core/src/interchange_store.rs`
- Modify: `crates/core/src/lib.rs` (add module declaration)

**Depends on:** Task 1 (get_all_nodes), Task 9 (interchange types + validate)

**Step 1: Write the failing test**

Create `crates/core/src/interchange_store.rs`:

```rust
//! Interchange store operations: loading documents into the graph store and exporting.
//!
//! Feature-gated behind `store`.

use std::collections::HashMap;

use crate::canonical::path_name;
use crate::interchange::*;
use crate::model::*;
use crate::store::{GraphStore, Result, StoreError};

/// Load an interchange document into the store, creating a new snapshot.
///
/// Assigns UUIDs, resolves canonical path references to node IDs,
/// and infers missing fields (name, sub_kind, provenance).
pub fn load_into_store(
    _store: &mut impl GraphStore,
    _doc: &InterchangeDocument,
) -> Result<Version> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interchange::parse_yaml;
    use crate::store::CozoStore;

    #[test]
    fn load_flat_document_creates_snapshot_and_nodes() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
  - canonical_path: /app/api
    kind: component
    sub_kind: module
    name: api
edges:
  - source: /app
    target: /app/api
    kind: contains
constraints:
  - name: no-outward
    kind: must_not_depend
    scope: /app/api/**
    target: /app/**
    message: "API must not depend outward"
    severity: error
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let nodes = store.get_all_nodes(version).unwrap();
        assert_eq!(nodes.len(), 2);

        let edges = store.get_all_edges(version, None).unwrap();
        assert_eq!(edges.len(), 1);

        let constraints = store.get_constraints(version).unwrap();
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].kind, "must_not_depend");
    }

    #[test]
    fn load_infers_name_from_path() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app/my-service
    kind: service
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let node = store
            .get_node_by_path(version, "/app/my-service")
            .unwrap()
            .unwrap();
        assert_eq!(node.name, "my-service");
    }

    #[test]
    fn load_infers_provenance_from_document_kind() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let node = store.get_node_by_path(version, "/app").unwrap().unwrap();
        assert_eq!(node.provenance, Provenance::Design);
    }

    #[test]
    fn load_nested_generates_contains_edges_with_resolved_ids() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let nodes = store.get_all_nodes(version).unwrap();
        assert_eq!(nodes.len(), 2);

        let edges = store.get_all_edges(version, Some(EdgeKind::Contains)).unwrap();
        assert_eq!(edges.len(), 1);

        // Edge should reference node UUIDs, not canonical paths
        let parent = store.get_node_by_path(version, "/app").unwrap().unwrap();
        let child = store.get_node_by_path(version, "/app/core").unwrap().unwrap();
        assert_eq!(edges[0].source, parent.id);
        assert_eq!(edges[0].target, child.id);
    }
}
```

**Step 2: Wire module to lib.rs**

In `crates/core/src/lib.rs`, add:

```rust
/// Interchange store operations: load and export.
#[cfg(feature = "store")]
pub mod interchange_store;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-core interchange_store::tests::load_flat`
Expected: FAIL — `todo!()` panics

**Step 4: Implement load_into_store**

Replace the `todo!()`:

```rust
/// Default sub_kind for a node kind when not specified.
fn default_sub_kind(kind: NodeKind) -> String {
    match kind {
        NodeKind::System => "system",
        NodeKind::Service => "service",
        NodeKind::Component => "component",
        NodeKind::Unit => "unit",
    }
    .to_string()
}

/// Infer provenance from the document's snapshot kind.
fn infer_provenance(kind: SnapshotKind) -> Provenance {
    match kind {
        SnapshotKind::Design => Provenance::Design,
        SnapshotKind::Analysis => Provenance::Analysis,
        SnapshotKind::Import => Provenance::Import,
    }
}

/// Load an interchange document into the store, creating a new snapshot.
///
/// Assigns UUIDs, resolves canonical path references to node IDs,
/// and infers missing fields (name, sub_kind, provenance).
pub fn load_into_store(
    store: &mut impl GraphStore,
    doc: &InterchangeDocument,
) -> Result<Version> {
    let version = store.create_snapshot(doc.kind, None)?;

    // Build nodes with UUIDs, collecting path→ID mapping
    let mut path_to_id: HashMap<String, String> = HashMap::new();
    let mut nodes = Vec::with_capacity(doc.nodes.len());

    for inode in &doc.nodes {
        let id = uuid::Uuid::new_v4().to_string();
        path_to_id.insert(inode.canonical_path.clone(), id.clone());

        nodes.push(Node {
            id,
            canonical_path: inode.canonical_path.clone(),
            qualified_name: inode.qualified_name.clone(),
            kind: inode.kind,
            sub_kind: inode
                .sub_kind
                .clone()
                .unwrap_or_else(|| default_sub_kind(inode.kind)),
            name: inode
                .name
                .clone()
                .unwrap_or_else(|| path_name(&inode.canonical_path).to_string()),
            language: inode.language.clone(),
            provenance: inode.provenance.unwrap_or_else(|| infer_provenance(doc.kind)),
            source_ref: inode.source_ref.clone(),
            metadata: inode.metadata.clone(),
        });
    }

    store.add_nodes_batch(version, &nodes)?;

    // Build edges, resolving canonical paths to node IDs
    let mut edges = Vec::with_capacity(doc.edges.len());
    for iedge in &doc.edges {
        let source_id = path_to_id.get(&iedge.source).ok_or_else(|| {
            StoreError::Internal(format!(
                "edge source path '{}' not found in nodes",
                iedge.source
            ))
        })?;
        let target_id = path_to_id.get(&iedge.target).ok_or_else(|| {
            StoreError::Internal(format!(
                "edge target path '{}' not found in nodes",
                iedge.target
            ))
        })?;

        edges.push(Edge {
            id: uuid::Uuid::new_v4().to_string(),
            source: source_id.clone(),
            target: target_id.clone(),
            kind: iedge.kind,
            provenance: infer_provenance(doc.kind),
            metadata: iedge.metadata.clone(),
        });
    }

    store.add_edges_batch(version, &edges)?;

    // Add constraints
    for ic in &doc.constraints {
        let constraint = Constraint {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ic.kind.clone(),
            name: ic.name.clone(),
            scope: ic.scope.clone(),
            target: ic.target.clone(),
            params: ic.params.clone(),
            message: ic.message.clone(),
            severity: ic.severity.unwrap_or(Severity::Error),
        };
        store.add_constraint(version, &constraint)?;
    }

    Ok(version)
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-core interchange_store::tests`
Expected: all 4 tests PASS

**Step 6: Commit**

```bash
git add crates/core/src/interchange_store.rs crates/core/src/lib.rs
git commit -m "Add load_into_store for interchange documents"
```

---

### Task 11: interchange_store.rs — export_yaml and export_json

**Files:**
- Modify: `crates/core/src/interchange_store.rs`

**Depends on:** Task 10

**Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn export_yaml_round_trips() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
  - canonical_path: /app/api
    kind: component
    sub_kind: module
    name: api
edges:
  - source: /app
    target: /app/api
    kind: contains
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let exported = export_yaml(&store, version).unwrap();
        let re_parsed = parse_yaml(&exported).unwrap();
        assert_eq!(re_parsed.nodes.len(), 2);
        assert_eq!(re_parsed.edges.len(), 1);
    }

    #[test]
    fn export_json_produces_valid_json() {
        let yaml = r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    sub_kind: workspace
    name: app
edges: []
constraints: []
"#;
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();

        let json_str = export_json(&store, version).unwrap();
        let re_parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(re_parsed["format"], "svt/v1");
    }
```

Add stub functions:

```rust
/// Export a version from the store as YAML (flat format).
pub fn export_yaml(_store: &impl GraphStore, _version: Version) -> Result<String> {
    todo!()
}

/// Export a version from the store as JSON (flat format).
pub fn export_json(_store: &impl GraphStore, _version: Version) -> Result<String> {
    todo!()
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core export_yaml`
Expected: FAIL

**Step 3: Implement export functions**

```rust
/// Build an InterchangeDocument from store data for a given version.
fn build_export_document(store: &impl GraphStore, version: Version) -> Result<InterchangeDocument> {
    // Find the snapshot for metadata
    let snapshots = store.list_snapshots()?;
    let snapshot = snapshots
        .iter()
        .find(|s| s.version == version)
        .ok_or(StoreError::VersionNotFound(version))?;

    let nodes = store.get_all_nodes(version)?;
    let edges = store.get_all_edges(version, None)?;
    let constraints = store.get_constraints(version)?;

    // Build ID→path mapping for edge resolution
    let id_to_path: HashMap<String, String> = nodes
        .iter()
        .map(|n| (n.id.clone(), n.canonical_path.clone()))
        .collect();

    let interchange_nodes: Vec<InterchangeNode> = nodes
        .iter()
        .map(|n| InterchangeNode {
            canonical_path: n.canonical_path.clone(),
            kind: n.kind,
            name: Some(n.name.clone()),
            sub_kind: Some(n.sub_kind.clone()),
            qualified_name: n.qualified_name.clone(),
            language: n.language.clone(),
            provenance: Some(n.provenance),
            source_ref: n.source_ref.clone(),
            metadata: n.metadata.clone(),
            children: None,
        })
        .collect();

    let interchange_edges: Vec<InterchangeEdge> = edges
        .iter()
        .filter_map(|e| {
            let source = id_to_path.get(&e.source)?;
            let target = id_to_path.get(&e.target)?;
            Some(InterchangeEdge {
                source: source.clone(),
                target: target.clone(),
                kind: e.kind,
                metadata: e.metadata.clone(),
            })
        })
        .collect();

    let interchange_constraints: Vec<InterchangeConstraint> = constraints
        .iter()
        .map(|c| InterchangeConstraint {
            name: c.name.clone(),
            kind: c.kind.clone(),
            scope: c.scope.clone(),
            target: c.target.clone(),
            params: c.params.clone(),
            message: c.message.clone(),
            severity: Some(c.severity),
        })
        .collect();

    Ok(InterchangeDocument {
        format: "svt/v1".to_string(),
        kind: snapshot.kind,
        version: Some(version),
        metadata: snapshot.metadata.clone(),
        nodes: interchange_nodes,
        edges: interchange_edges,
        constraints: interchange_constraints,
    })
}

/// Export a version from the store as YAML (flat format).
pub fn export_yaml(store: &impl GraphStore, version: Version) -> Result<String> {
    let doc = build_export_document(store, version)?;
    serde_yaml::to_string(&doc).map_err(|e| StoreError::Internal(e.to_string()))
}

/// Export a version from the store as JSON (flat format).
pub fn export_json(store: &impl GraphStore, version: Version) -> Result<String> {
    let doc = build_export_document(store, version)?;
    serde_json::to_string_pretty(&doc).map_err(|e| StoreError::Internal(e.to_string()))
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core interchange_store::tests`
Expected: all tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/interchange_store.rs
git commit -m "Add export_yaml and export_json"
```

---

### Task 12: conformance.rs — types and evaluate_constraint_must_not_depend

**Files:**
- Create: `crates/core/src/conformance.rs`
- Modify: `crates/core/src/lib.rs` (add module declaration)

**Depends on:** Task 1 (get_all_nodes), Task 5 (canonical_path_matches)

**Step 1: Create conformance.rs with types and stub**

Create `crates/core/src/conformance.rs`:

```rust
//! Conformance evaluation: constraint checking and report generation.
//!
//! Feature-gated behind `store`.

use serde::{Deserialize, Serialize};

use crate::canonical::canonical_path_matches;
use crate::model::*;
use crate::store::{GraphStore, Result};

/// Status of a single constraint evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintStatus {
    /// Constraint satisfied.
    Pass,
    /// Constraint violated.
    Fail,
    /// Cannot be evaluated (e.g., needs analysis data).
    NotEvaluable,
}

/// A single violation found during constraint evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    /// Canonical path of the violating source node.
    pub source_path: String,
    /// Canonical path of the forbidden target (if applicable).
    pub target_path: Option<String>,
    /// ID of the violating edge.
    pub edge_id: Option<String>,
    /// Kind of the violating edge.
    pub edge_kind: Option<EdgeKind>,
}

/// Result of evaluating a single constraint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintResult {
    /// Name of the constraint.
    pub constraint_name: String,
    /// Kind of the constraint (e.g., "must_not_depend").
    pub constraint_kind: String,
    /// Pass, Fail, or NotEvaluable.
    pub status: ConstraintStatus,
    /// Severity from the constraint definition.
    pub severity: Severity,
    /// Human-readable message.
    pub message: String,
    /// Specific violations found (empty if pass).
    pub violations: Vec<Violation>,
}

/// A node that is unmatched between design and analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmatchedNode {
    /// Canonical path.
    pub canonical_path: String,
    /// Node kind.
    pub kind: NodeKind,
    /// Human-readable name.
    pub name: String,
}

/// Summary counts for a conformance report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConformanceSummary {
    /// Constraints that passed.
    pub passed: usize,
    /// Constraints that failed.
    pub failed: usize,
    /// Constraints that produced warnings.
    pub warned: usize,
    /// Constraints that could not be evaluated.
    pub not_evaluable: usize,
    /// Design nodes not found in analysis.
    pub unimplemented: usize,
    /// Analysis nodes not found in design.
    pub undocumented: usize,
}

/// Full conformance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConformanceReport {
    /// Design version evaluated.
    pub design_version: Version,
    /// Analysis version (None in design-only mode).
    pub analysis_version: Option<Version>,
    /// Results for each constraint.
    pub constraint_results: Vec<ConstraintResult>,
    /// Design nodes not found in analysis.
    pub unimplemented: Vec<UnmatchedNode>,
    /// Analysis nodes not found in design.
    pub undocumented: Vec<UnmatchedNode>,
    /// Summary counts.
    pub summary: ConformanceSummary,
}

/// Evaluate a single `must_not_depend` constraint.
pub fn evaluate_constraint_must_not_depend(
    _store: &impl GraphStore,
    _constraint: &Constraint,
    _version: Version,
) -> Result<ConstraintResult> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interchange::{parse_yaml, InterchangeDocument};
    use crate::interchange_store::load_into_store;
    use crate::store::CozoStore;

    fn load_test_doc(yaml: &str) -> (CozoStore, Version) {
        let doc = parse_yaml(yaml).unwrap();
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = load_into_store(&mut store, &doc).unwrap();
        (store, version)
    }

    #[test]
    fn must_not_depend_passes_when_no_violations() {
        let (store, version) = load_test_doc(
            r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
      - canonical_path: /app/cli
        kind: service
edges:
  - source: /app/cli
    target: /app/core
    kind: depends
constraints:
  - name: core-no-cli-deps
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
"#,
        );
        let constraints = store.get_constraints(version).unwrap();
        let result =
            evaluate_constraint_must_not_depend(&store, &constraints[0], version).unwrap();
        assert_eq!(result.status, ConstraintStatus::Pass);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn must_not_depend_fails_with_violation() {
        let (store, version) = load_test_doc(
            r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
      - canonical_path: /app/cli
        kind: service
edges:
  - source: /app/core
    target: /app/cli
    kind: depends
constraints:
  - name: core-no-cli-deps
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
"#,
        );
        let constraints = store.get_constraints(version).unwrap();
        let result =
            evaluate_constraint_must_not_depend(&store, &constraints[0], version).unwrap();
        assert_eq!(result.status, ConstraintStatus::Fail);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].source_path, "/app/core");
        assert_eq!(
            result.violations[0].target_path,
            Some("/app/cli".to_string())
        );
    }
}
```

**Step 2: Wire module to lib.rs**

In `crates/core/src/lib.rs`, add:

```rust
/// Conformance evaluation: constraint checking and report generation.
#[cfg(feature = "store")]
pub mod conformance;
```

**Step 3: Run tests to verify they fail**

Run: `cargo test -p svt-core conformance::tests::must_not_depend_passes`
Expected: FAIL — `todo!()` panics

**Step 4: Implement evaluate_constraint_must_not_depend**

Replace the `todo!()`:

```rust
/// Evaluate a single `must_not_depend` constraint.
///
/// Finds all nodes matching `scope`, all nodes matching `target`,
/// and checks for `Depends` edges between them.
pub fn evaluate_constraint_must_not_depend(
    store: &impl GraphStore,
    constraint: &Constraint,
    version: Version,
) -> Result<ConstraintResult> {
    let all_nodes = store.get_all_nodes(version)?;
    let depends_edges = store.get_all_edges(version, Some(EdgeKind::Depends))?;

    let target_pattern = constraint.target.as_deref().unwrap_or("");

    // Build sets of node IDs matching scope and target patterns
    let mut scope_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut target_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut id_to_path: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();

    for node in &all_nodes {
        id_to_path.insert(&node.id, &node.canonical_path);
        if canonical_path_matches(&node.canonical_path, &constraint.scope) {
            scope_ids.insert(&node.id);
        }
        if canonical_path_matches(&node.canonical_path, target_pattern) {
            target_ids.insert(&node.id);
        }
    }

    // Find forbidden edges: scope node depends on target node
    let mut violations = Vec::new();
    for edge in &depends_edges {
        if scope_ids.contains(edge.source.as_str())
            && target_ids.contains(edge.target.as_str())
        {
            violations.push(Violation {
                source_path: id_to_path
                    .get(edge.source.as_str())
                    .unwrap_or(&"")
                    .to_string(),
                target_path: Some(
                    id_to_path
                        .get(edge.target.as_str())
                        .unwrap_or(&"")
                        .to_string(),
                ),
                edge_id: Some(edge.id.clone()),
                edge_kind: Some(edge.kind),
            });
        }
    }

    let status = if violations.is_empty() {
        ConstraintStatus::Pass
    } else {
        ConstraintStatus::Fail
    };

    Ok(ConstraintResult {
        constraint_name: constraint.name.clone(),
        constraint_kind: constraint.kind.clone(),
        status,
        severity: constraint.severity,
        message: constraint.message.clone(),
        violations,
    })
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p svt-core conformance::tests`
Expected: both tests PASS

**Step 6: Commit**

```bash
git add crates/core/src/conformance.rs crates/core/src/lib.rs
git commit -m "Add conformance types and evaluate_constraint_must_not_depend"
```

---

### Task 13: conformance.rs — evaluate_design

**Files:**
- Modify: `crates/core/src/conformance.rs`

**Depends on:** Task 12

**Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn evaluate_design_reports_all_constraints() {
        let (store, version) = load_test_doc(
            r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
      - canonical_path: /app/cli
        kind: service
edges:
  - source: /app/cli
    target: /app/core
    kind: depends
constraints:
  - name: core-no-cli-deps
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
  - name: cli-has-main
    kind: must_contain
    scope: /app/cli
    message: "CLI must contain main"
    severity: warning
"#,
        );
        let report = evaluate_design(&store, version).unwrap();
        assert_eq!(report.constraint_results.len(), 4); // 2 structural + 2 constraints
        assert_eq!(report.design_version, version);
        assert!(report.analysis_version.is_none());

        // must_not_depend should pass
        let mnd = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "core-no-cli-deps")
            .unwrap();
        assert_eq!(mnd.status, ConstraintStatus::Pass);

        // must_contain should be NotEvaluable
        let mc = report
            .constraint_results
            .iter()
            .find(|r| r.constraint_name == "cli-has-main")
            .unwrap();
        assert_eq!(mc.status, ConstraintStatus::NotEvaluable);
    }

    #[test]
    fn evaluate_design_summary_counts() {
        let (store, version) = load_test_doc(
            r#"
format: svt/v1
kind: design
nodes:
  - canonical_path: /app
    kind: system
    children:
      - canonical_path: /app/core
        kind: service
edges: []
constraints:
  - name: core-no-outward
    kind: must_not_depend
    scope: /app/core/**
    target: /app/**
    message: "Core stays clean"
    severity: error
"#,
        );
        let report = evaluate_design(&store, version).unwrap();
        // 2 structural (both pass) + 1 must_not_depend (pass)
        assert_eq!(report.summary.passed, 3);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.summary.not_evaluable, 0);
    }
```

Add stub function:

```rust
/// Evaluate a design version: structural checks + constraint evaluation.
///
/// Design-only mode: no analysis version. Non-evaluable constraints
/// (e.g., must_contain without analysis data) are marked `NotEvaluable`.
pub fn evaluate_design(
    _store: &impl GraphStore,
    _version: Version,
) -> Result<ConformanceReport> {
    todo!()
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core evaluate_design_reports`
Expected: FAIL

**Step 3: Implement evaluate_design**

Add import at top of file:

```rust
use crate::validation;
```

Replace the stub:

```rust
/// Evaluate a design version: structural checks + constraint evaluation.
///
/// Design-only mode: no analysis version. Non-evaluable constraints
/// (e.g., must_contain without analysis data) are marked `NotEvaluable`.
pub fn evaluate_design(
    store: &impl GraphStore,
    version: Version,
) -> Result<ConformanceReport> {
    let mut results = Vec::new();

    // Structural check: containment acyclicity
    let cycles = validation::validate_contains_acyclic(store, version)?;
    results.push(ConstraintResult {
        constraint_name: "containment-acyclic".to_string(),
        constraint_kind: "structural".to_string(),
        status: if cycles.is_empty() {
            ConstraintStatus::Pass
        } else {
            ConstraintStatus::Fail
        },
        severity: Severity::Error,
        message: if cycles.is_empty() {
            "Containment hierarchy is acyclic".to_string()
        } else {
            format!("Found {} cycle(s) in containment hierarchy", cycles.len())
        },
        violations: cycles
            .iter()
            .map(|c| Violation {
                source_path: c.node_ids.first().cloned().unwrap_or_default(),
                target_path: c.node_ids.last().cloned(),
                edge_id: None,
                edge_kind: Some(EdgeKind::Contains),
            })
            .collect(),
    });

    // Structural check: referential integrity
    let integrity_errors = validation::validate_referential_integrity(store, version)?;
    results.push(ConstraintResult {
        constraint_name: "referential-integrity".to_string(),
        constraint_kind: "structural".to_string(),
        status: if integrity_errors.is_empty() {
            ConstraintStatus::Pass
        } else {
            ConstraintStatus::Fail
        },
        severity: Severity::Error,
        message: if integrity_errors.is_empty() {
            "All edge references are valid".to_string()
        } else {
            format!(
                "Found {} referential integrity error(s)",
                integrity_errors.len()
            )
        },
        violations: integrity_errors
            .iter()
            .map(|e| Violation {
                source_path: e.missing_node_id.clone(),
                target_path: None,
                edge_id: Some(e.edge_id.clone()),
                edge_kind: None,
            })
            .collect(),
    });

    // Evaluate each constraint
    let constraints = store.get_constraints(version)?;
    for constraint in &constraints {
        let result = match constraint.kind.as_str() {
            "must_not_depend" => evaluate_constraint_must_not_depend(store, constraint, version)?,
            _ => ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!(
                    "{} not evaluable (design-only mode)",
                    constraint.kind
                ),
                violations: vec![],
            },
        };
        results.push(result);
    }

    // Compute summary
    let summary = ConformanceSummary {
        passed: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Pass)
            .count(),
        failed: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Fail)
            .count(),
        warned: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::Fail && r.severity == Severity::Warning)
            .count(),
        not_evaluable: results
            .iter()
            .filter(|r| r.status == ConstraintStatus::NotEvaluable)
            .count(),
        unimplemented: 0,
        undocumented: 0,
    };

    Ok(ConformanceReport {
        design_version: version,
        analysis_version: None,
        constraint_results: results,
        unimplemented: vec![],
        undocumented: vec![],
        summary,
    })
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core conformance::tests`
Expected: all tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "Add evaluate_design conformance function"
```

---

### Task 14: CLI — subcommand skeleton and svt import

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/Cargo.toml`

**Depends on:** Task 10 (load_into_store), Task 13 (evaluate_design)

**Step 1: Add dependencies to CLI Cargo.toml**

Add `serde_json` and `serde_yaml` to `crates/cli/Cargo.toml` dependencies:

```toml
serde_json = "1"
serde_yaml = "0.9"
```

**Step 2: Replace main.rs with subcommand skeleton and import command**

Replace `crates/cli/src/main.rs` entirely:

```rust
//! `svt` -- CLI for software-visualizer-tool.

#![warn(missing_docs)]

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::store::CozoStore;

/// Software Visualizer Tool -- analyze, model, and visualize software architecture.
#[derive(Parser, Debug)]
#[command(name = "svt", version, about)]
struct Cli {
    /// Store location (default: .svt/store)
    #[arg(long, default_value = ".svt/store")]
    store: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Import a design YAML/JSON file into the store.
    Import(ImportArgs),
    /// Run conformance checks on the current design.
    Check(CheckArgs),
}

#[derive(clap::Args, Debug)]
struct ImportArgs {
    /// Path to the YAML or JSON file to import.
    file: PathBuf,
}

#[derive(clap::Args, Debug)]
struct CheckArgs {
    /// Design version to check (default: latest).
    #[arg(long)]
    design: Option<u64>,

    /// Minimum severity to cause a non-zero exit code.
    #[arg(long, default_value = "error")]
    fail_on: String,

    /// Output format: human or json.
    #[arg(long, default_value = "human")]
    format: String,
}

fn open_or_create_store(path: &Path) -> Result<CozoStore> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating store directory {}", parent.display()))?;
    }
    CozoStore::new_persistent(path).map_err(|e| anyhow::anyhow!("{}", e))
}

fn open_store(path: &Path) -> Result<CozoStore> {
    if !path.exists() {
        bail!(
            "Store not found at {}. Run `svt import` first.",
            path.display()
        );
    }
    CozoStore::new_persistent(path).map_err(|e| anyhow::anyhow!("{}", e))
}

fn run_import(store_path: &Path, args: &ImportArgs) -> Result<()> {
    let content = std::fs::read_to_string(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;

    let ext = args
        .file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let doc = match ext {
        "yaml" | "yml" => interchange::parse_yaml(&content)
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        "json" => interchange::parse_json(&content)
            .map_err(|e| anyhow::anyhow!("{}", e))?,
        _ => bail!("Unsupported file format: .{ext}. Use .yaml, .yml, or .json"),
    };

    // Validate
    let warnings = interchange::validate_document(&doc)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    for w in &warnings {
        eprintln!("  WARN  {}: {}", w.path, w.message);
    }

    let mut store = open_or_create_store(store_path)?;
    let version = interchange_store::load_into_store(&mut store, &doc)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let node_count = store
        .get_all_nodes(version)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();
    let edge_count = store
        .get_all_edges(version, None)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();
    let constraint_count = store
        .get_constraints(version)
        .map_err(|e| anyhow::anyhow!("{}", e))?
        .len();

    println!("Imported {} as version {}", args.file.display(), version);
    println!(
        "  {} nodes, {} edges, {} constraints",
        node_count, edge_count, constraint_count
    );

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Import(args) => run_import(&cli.store, args),
        Commands::Check(args) => run_check(&cli.store, args),
    }
}

fn run_check(_store_path: &Path, _args: &CheckArgs) -> Result<()> {
    todo!("Implemented in Task 15")
}
```

**Step 3: Verify it compiles**

Run: `cargo build -p svt-cli`
Expected: compiles (run_check is `todo!()` but not called at compile time)

**Step 4: Commit**

```bash
git add crates/cli/src/main.rs crates/cli/Cargo.toml
git commit -m "Add CLI subcommand skeleton and svt import command"
```

---

### Task 15: CLI — svt check command

**Files:**
- Modify: `crates/cli/src/main.rs`

**Depends on:** Task 14

**Step 1: Implement run_check**

Replace the `run_check` stub in `main.rs`:

```rust
fn run_check(store_path: &Path, args: &CheckArgs) -> Result<()> {
    use svt_core::conformance::{self, ConstraintStatus, ConformanceReport};
    use svt_core::model::{Severity, SnapshotKind};
    use svt_core::store::GraphStore;

    let store = open_store(store_path)?;

    let version = match args.design {
        Some(v) => v,
        None => store
            .latest_version(SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No design versions found in store"))?,
    };

    let report = conformance::evaluate_design(&store, version)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if args.format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        );
    } else {
        print_human_report(&report);
    }

    // Determine exit code based on fail_on severity
    let fail_severity = match args.fail_on.as_str() {
        "warning" => Some(Severity::Warning),
        "info" => Some(Severity::Info),
        _ => Some(Severity::Error), // default: error
    };

    let has_failures = report.constraint_results.iter().any(|r| {
        r.status == ConstraintStatus::Fail
            && fail_severity
                .map(|s| severity_at_or_above(r.severity, s))
                .unwrap_or(false)
    });

    if has_failures {
        std::process::exit(1);
    }

    Ok(())
}

fn severity_at_or_above(actual: Severity, threshold: Severity) -> bool {
    severity_rank(actual) >= severity_rank(threshold)
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
    }
}

fn print_human_report(report: &conformance::ConformanceReport) {
    use svt_core::conformance::ConstraintStatus;

    println!("Checking design v{}...\n", report.design_version);

    for result in &report.constraint_results {
        let tag = match result.status {
            ConstraintStatus::Pass => "  PASS ",
            ConstraintStatus::Fail => "  FAIL ",
            ConstraintStatus::NotEvaluable => "  N/A  ",
        };
        println!("{} {}: {}", tag, result.constraint_name, result.message);

        for v in &result.violations {
            let target = v
                .target_path
                .as_deref()
                .map(|t| format!(" -> {}", t))
                .unwrap_or_default();
            println!("         {} {}{}", "-", v.source_path, target);
        }
    }

    println!();
    println!(
        "  {} passed, {} failed, {} warnings, {} not evaluable",
        report.summary.passed,
        report.summary.failed,
        report.summary.warned,
        report.summary.not_evaluable,
    );
}
```

Add the import at the top of the file:

```rust
use svt_core::conformance;
use svt_core::store::GraphStore;
```

Note: also add `Serialize, Deserialize` derives to `Severity` in the model if not already there (it should already have them from Milestone 1).

**Step 2: Verify it compiles**

Run: `cargo build -p svt-cli`
Expected: compiles

**Step 3: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "Add svt check command with human and JSON output"
```

---

### Task 16: Dog-food integration test

**Files:**
- Create: `crates/core/tests/dogfood.rs`

**Depends on:** Task 15

**Step 1: Write the integration test**

Create `crates/core/tests/dogfood.rs`:

```rust
//! Dog-food test: load design/architecture.yaml and run conformance checks.

use svt_core::conformance::{self, ConstraintStatus};
use svt_core::interchange;
use svt_core::interchange_store;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn dogfood_architecture_yaml_loads_successfully() {
    let yaml = include_str!("../../../design/architecture.yaml");
    let doc = interchange::parse_yaml(yaml).unwrap();

    let warnings = interchange::validate_document(&doc).unwrap();
    assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);

    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    let nodes = store.get_all_nodes(version).unwrap();
    assert!(
        nodes.len() >= 30,
        "expected at least 30 nodes, got {}",
        nodes.len()
    );

    let edges = store.get_all_edges(version, None).unwrap();
    assert!(
        edges.len() >= 20,
        "expected at least 20 edges, got {}",
        edges.len()
    );

    let constraints = store.get_constraints(version).unwrap();
    assert!(
        constraints.len() >= 5,
        "expected at least 5 constraints, got {}",
        constraints.len()
    );
}

#[test]
fn dogfood_conformance_all_must_not_depend_pass() {
    let yaml = include_str!("../../../design/architecture.yaml");
    let doc = interchange::parse_yaml(yaml).unwrap();
    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    let report = conformance::evaluate_design(&store, version).unwrap();

    // All must_not_depend constraints should pass
    let must_not_depend_results: Vec<_> = report
        .constraint_results
        .iter()
        .filter(|r| r.constraint_kind == "must_not_depend")
        .collect();

    assert!(
        !must_not_depend_results.is_empty(),
        "should have must_not_depend constraints"
    );

    for result in &must_not_depend_results {
        assert_eq!(
            result.status,
            ConstraintStatus::Pass,
            "constraint '{}' should pass but got {:?} with violations: {:?}",
            result.constraint_name,
            result.status,
            result.violations
        );
    }

    // Non-evaluable constraints should be marked as such
    let not_evaluable: Vec<_> = report
        .constraint_results
        .iter()
        .filter(|r| r.status == ConstraintStatus::NotEvaluable)
        .collect();

    // boundary, must_contain, max_fan_in are not evaluable in design-only mode
    assert!(
        !not_evaluable.is_empty(),
        "should have some not-evaluable constraints"
    );

    // No failures
    assert_eq!(report.summary.failed, 0);
}

#[test]
fn dogfood_conformance_report_serialises_to_json() {
    let yaml = include_str!("../../../design/architecture.yaml");
    let doc = interchange::parse_yaml(yaml).unwrap();
    let mut store = CozoStore::new_in_memory().unwrap();
    let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

    let report = conformance::evaluate_design(&store, version).unwrap();
    let json = serde_json::to_string_pretty(&report).unwrap();

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["design_version"].is_number());
    assert!(parsed["summary"]["passed"].is_number());
}
```

**Step 2: Run tests**

Run: `cargo test -p svt-core --test dogfood`
Expected: all 3 tests PASS

**Step 3: Commit**

```bash
git add crates/core/tests/dogfood.rs
git commit -m "Add dog-food integration tests for architecture.yaml"
```

---

### Task 17: Proptest round-trips

**Files:**
- Create: `crates/core/tests/proptest_interchange.rs`

**Depends on:** Task 11 (export)

**Step 1: Write proptest round-trip test**

Create `crates/core/tests/proptest_interchange.rs`:

```rust
//! Property-based tests for interchange round-trips.

use proptest::prelude::*;

use svt_core::interchange::{
    InterchangeConstraint, InterchangeDocument, InterchangeEdge, InterchangeNode,
};
use svt_core::interchange_store;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

fn arb_node_kind() -> impl Strategy<Value = NodeKind> {
    prop_oneof![
        Just(NodeKind::System),
        Just(NodeKind::Service),
        Just(NodeKind::Component),
        Just(NodeKind::Unit),
    ]
}

/// Generate a valid interchange document with N nodes in a flat hierarchy.
fn arb_document(max_nodes: usize) -> impl Strategy<Value = InterchangeDocument> {
    (1..=max_nodes)
        .prop_flat_map(|n| {
            proptest::collection::vec(arb_node_kind(), n).prop_map(move |kinds| {
                let mut nodes = Vec::new();
                let root_path = "/test".to_string();

                nodes.push(InterchangeNode {
                    canonical_path: root_path.clone(),
                    kind: NodeKind::System,
                    name: Some("test".to_string()),
                    sub_kind: Some("system".to_string()),
                    qualified_name: None,
                    language: None,
                    provenance: None,
                    source_ref: None,
                    metadata: None,
                    children: None,
                });

                let mut edges = vec![];

                for (i, kind) in kinds.iter().enumerate() {
                    let path = format!("/test/node-{}", i);
                    nodes.push(InterchangeNode {
                        canonical_path: path.clone(),
                        kind: *kind,
                        name: Some(format!("node-{}", i)),
                        sub_kind: Some("module".to_string()),
                        qualified_name: None,
                        language: None,
                        provenance: None,
                        source_ref: None,
                        metadata: None,
                        children: None,
                    });
                    edges.push(InterchangeEdge {
                        source: root_path.clone(),
                        target: path,
                        kind: EdgeKind::Contains,
                        metadata: None,
                    });
                }

                InterchangeDocument {
                    format: "svt/v1".to_string(),
                    kind: SnapshotKind::Design,
                    version: None,
                    metadata: None,
                    nodes,
                    edges,
                    constraints: vec![],
                }
            })
        })
}

proptest! {
    #[test]
    fn import_export_round_trip_preserves_node_count(doc in arb_document(10)) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

        let exported_yaml = interchange_store::export_yaml(&store, version).unwrap();
        let re_parsed = svt_core::interchange::parse_yaml(&exported_yaml).unwrap();

        prop_assert_eq!(re_parsed.nodes.len(), doc.nodes.len());
    }

    #[test]
    fn import_export_round_trip_preserves_edge_count(doc in arb_document(10)) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let version = interchange_store::load_into_store(&mut store, &doc).unwrap();

        let exported_yaml = interchange_store::export_yaml(&store, version).unwrap();
        let re_parsed = svt_core::interchange::parse_yaml(&exported_yaml).unwrap();

        prop_assert_eq!(re_parsed.edges.len(), doc.edges.len());
    }
}
```

**Step 2: Run tests**

Run: `cargo test -p svt-core --test proptest_interchange`
Expected: all proptest cases PASS

**Step 3: Commit**

```bash
git add crates/core/tests/proptest_interchange.rs
git commit -m "Add proptest round-trip tests for interchange"
```

---

### Task 18: Final cleanup — clippy, fmt, full test suite

**Files:**
- Potentially any file with clippy/fmt issues

**Depends on:** Task 16, Task 17

**Step 1: Run formatter**

Run: `cargo fmt --all`

**Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`

Fix any warnings.

**Step 3: Run full test suite**

Run: `cargo test --all`
Expected: all tests PASS

**Step 4: Verify doc builds**

Run: `cargo doc -p svt-core --no-deps`
Expected: no warnings about missing docs

**Step 5: Commit**

```bash
git add -A
git commit -m "Milestone 2: clippy clean, all tests passing"
```

---

## Final State

After all 19 tasks:

- `crates/core/src/lib.rs` — 4 always-compiled modules (`model`, `canonical`, `interchange`, + wired test support), 4 store-gated modules (`store`, `validation`, `interchange_store`, `conformance`)
- `crates/core/src/canonical.rs` — 5 public functions, all WASM-safe
- `crates/core/src/interchange.rs` — wire types + parse_yaml/parse_json/validate_document
- `crates/core/src/interchange_store.rs` — load_into_store/export_yaml/export_json
- `crates/core/src/conformance.rs` — types + evaluate_constraint_must_not_depend + evaluate_design
- `crates/cli/src/main.rs` — `svt import` and `svt check` subcommands
- `crates/core/tests/dogfood.rs` — 3 integration tests against architecture.yaml
- `crates/core/tests/proptest_interchange.rs` — property-based round-trip tests

End-to-end story works: `svt import design/architecture.yaml && svt check`
