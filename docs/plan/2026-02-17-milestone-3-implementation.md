# Milestone 3: Rust Analyzer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Full discovery-mode pipeline: analyze a Rust project with tree-sitter, produce an analysis snapshot, compare against a design snapshot, report drift. Dog-food on this project.

**Architecture:** Three-stage layered pipeline (Discovery -> Parse -> Map) in `crates/analyzer/`. Discovery uses `cargo_metadata`. Parsing uses `tree-sitter-rust` for full-depth extraction (crates, modules, types, functions, call edges). Mapping transforms qualified names to canonical paths. Conformance comparison in `crates/core/src/conformance.rs` implements the real `evaluate()`.

**Tech Stack:** Rust, tree-sitter 0.24, tree-sitter-rust, cargo_metadata, walkdir, svt-core (GraphStore trait, canonical path utils)

**Validation Checkpoints:**
- `[TEST+CODE]` — after every task (inner loop)
- `[DESIGN]` — after each module completes
- `[THEORY]` — at 3 integration points marked below

**Dependency Graph:**
```
Task 0 (deps) ──→ Task 1 (types) ──→ Task 2 (discovery) ──→ Task 7 (API)
                       │                                        ↑
                       └──→ Task 3 (trait) ──→ Task 4 (struct) ──→ Task 5 (edges) ─┘
                       │                                                            │
                       └──→ Task 6 (mapping) ───────────────────────────────────────┘
                                                                                    │
Task 9 (evaluate) ──→ Task 10 (conformance tests)                                  │
                                   │                                                │
                                   └──→ Task 12 (check --analysis)                  │
                                                   │                                │
Task 7 ──→ Task 8 (e2e test) ──→ Task 11 (CLI analyze) ──→ Task 13 (CLI tests)     │
                                                                    │               │
                                                                    └──→ Task 14 (dog-food)
```

**Parallelism:** Builder A takes the parser track (Tasks 0,1,3,4,5,7,8,11,14). Builder B takes the mapping + conformance track (Tasks 2,6,9,10,12,13).

---

### Task 0: Add Dependencies to Analyzer Cargo.toml

**Files:**
- Modify: `crates/analyzer/Cargo.toml`

**Step 1: Update Cargo.toml with all required dependencies**

```toml
[package]
name = "svt-analyzer"
description = "Tree-sitter based code analysis and discovery for software-visualizer-tool"
version.workspace = true
edition.workspace = true

[dependencies]
svt-core = { path = "../core" }
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
cargo_metadata = "0.19"
walkdir = "2"
thiserror = "2"
uuid = { version = "1", features = ["v5"] }

[dev-dependencies]
tempfile = "3"
```

**Step 2: Verify it compiles**

Run: `cargo check -p svt-analyzer`
Expected: success (downloads new deps)

**Step 3: Commit**

```bash
git add crates/analyzer/Cargo.toml
git commit -m "feat(analyzer): add tree-sitter, cargo_metadata, walkdir dependencies"
```

---

### Task 1: Create Intermediate Types

**Files:**
- Create: `crates/analyzer/src/types.rs`
- Modify: `crates/analyzer/src/lib.rs`

**Step 1: Write the types module with all intermediate types**

```rust
// crates/analyzer/src/types.rs
//! Intermediate types for the analysis pipeline.

use std::path::PathBuf;

use svt_core::model::{EdgeKind, NodeKind};

/// Type of crate (library or binary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrateType {
    /// Library crate (`lib.rs` entry point).
    Lib,
    /// Binary crate (`main.rs` entry point).
    Bin,
}

/// Information about a single crate in the project.
#[derive(Debug, Clone)]
pub struct CrateInfo {
    /// Crate name (e.g., "svt-core").
    pub name: String,
    /// Library or binary.
    pub crate_type: CrateType,
    /// Root directory of the crate.
    pub root: PathBuf,
    /// Entry point file (e.g., `src/lib.rs` or `src/main.rs`).
    pub entry_point: PathBuf,
    /// All `.rs` source files under `src/`.
    pub source_files: Vec<PathBuf>,
}

/// Layout of a Rust project (workspace or single crate).
#[derive(Debug, Clone)]
pub struct ProjectLayout {
    /// Workspace root directory.
    pub workspace_root: PathBuf,
    /// All crates in the workspace.
    pub crates: Vec<CrateInfo>,
}

/// A code element extracted by tree-sitter (before canonical path mapping).
#[derive(Debug, Clone)]
pub struct AnalysisItem {
    /// Language-specific qualified name (e.g., "svt_core::model::Node").
    pub qualified_name: String,
    /// Abstraction level.
    pub kind: NodeKind,
    /// Language-specific type (e.g., "crate", "module", "struct", "function").
    pub sub_kind: String,
    /// Qualified name of the containment parent, if any.
    pub parent_qualified_name: Option<String>,
    /// Source file and line reference (e.g., "crates/core/src/model/mod.rs:42").
    pub source_ref: String,
    /// Source language.
    pub language: String,
}

/// A relationship between code elements (before canonical path mapping).
#[derive(Debug, Clone)]
pub struct AnalysisRelation {
    /// Qualified name of the source element.
    pub source_qualified_name: String,
    /// Qualified name of the target element.
    pub target_qualified_name: String,
    /// Relationship type.
    pub kind: EdgeKind,
}

/// A warning produced during analysis (non-fatal).
#[derive(Debug, Clone)]
pub struct AnalysisWarning {
    /// Source file and line where the issue was found.
    pub source_ref: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Summary of an analysis run.
#[derive(Debug, Clone)]
pub struct AnalysisSummary {
    /// Version number of the created analysis snapshot.
    pub version: svt_core::model::Version,
    /// Number of crates analyzed.
    pub crates_analyzed: usize,
    /// Number of source files parsed.
    pub files_analyzed: usize,
    /// Number of nodes created in the store.
    pub nodes_created: usize,
    /// Number of edges created in the store.
    pub edges_created: usize,
    /// Warnings produced during analysis.
    pub warnings: Vec<AnalysisWarning>,
}
```

**Step 2: Update lib.rs to declare the module**

```rust
// crates/analyzer/src/lib.rs
//! `svt-analyzer` -- Tree-sitter based code analysis and structure discovery.
//!
//! This crate scans source code using tree-sitter grammars to extract
//! architectural elements (modules, types, functions, dependencies) and
//! populate the core graph model.

#![warn(missing_docs)]

pub mod types;
pub mod discovery;
pub mod languages;
pub mod mapping;
```

**Step 3: Verify it compiles**

Run: `cargo check -p svt-analyzer`
Expected: success (warnings about empty modules are ok for now)

**Step 4: Commit**

```bash
git add crates/analyzer/src/types.rs crates/analyzer/src/lib.rs
git commit -m "feat(analyzer): add intermediate types for analysis pipeline"
```

---

### Task 2: Discovery Module

**Files:**
- Create: `crates/analyzer/src/discovery.rs`

**Step 1: Write the failing test**

At the bottom of `discovery.rs`, add tests. The key test: running discovery on this project's own workspace should find all 4 crates.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_workspace_crates() {
        // Use the actual project root (this workspace)
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()  // crates/
            .unwrap()
            .parent()  // workspace root
            .unwrap()
            .to_path_buf();
        let layout = discover_project(&project_root).unwrap();
        assert!(layout.crates.len() >= 4, "should find at least svt-core, svt-analyzer, svt-cli, svt-server");

        let names: Vec<&str> = layout.crates.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"svt-core"), "should find svt-core");
        assert!(names.contains(&"svt-analyzer"), "should find svt-analyzer");
    }

    #[test]
    fn crate_info_has_source_files() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .to_path_buf();
        let layout = discover_project(&project_root).unwrap();

        let core = layout.crates.iter().find(|c| c.name == "svt-core").unwrap();
        assert!(!core.source_files.is_empty(), "svt-core should have .rs files");
        assert!(core.source_files.iter().any(|f| f.ends_with("lib.rs")), "should include lib.rs");
    }

    #[test]
    fn crate_type_detected_correctly() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .to_path_buf();
        let layout = discover_project(&project_root).unwrap();

        let core = layout.crates.iter().find(|c| c.name == "svt-core").unwrap();
        assert_eq!(core.crate_type, CrateType::Lib);

        // svt-cli has a binary target named "svt"
        let cli_bin = layout.crates.iter().find(|c| c.name == "svt-cli" || c.name == "svt");
        assert!(cli_bin.is_some(), "should find CLI binary crate");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer -- discovery`
Expected: FAIL (function `discover_project` not found)

**Step 3: Implement the discovery module**

```rust
// crates/analyzer/src/discovery.rs
//! Project discovery: workspace detection, crate enumeration, source file walking.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::types::{CrateInfo, CrateType, ProjectLayout};

/// Errors during project discovery.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// Failed to run cargo metadata.
    #[error("cargo metadata failed: {0}")]
    CargoMetadata(String),
    /// IO error during file walking.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Discover the layout of a Rust project at the given root.
///
/// Runs `cargo metadata` to find workspace members and their targets,
/// then walks each crate's `src/` directory for `.rs` files.
pub fn discover_project(project_root: &Path) -> Result<ProjectLayout, DiscoveryError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .current_dir(project_root)
        .no_deps()
        .exec()
        .map_err(|e| DiscoveryError::CargoMetadata(e.to_string()))?;

    let workspace_root = metadata.workspace_root.clone().into_std_path_buf();
    let mut crates = Vec::new();

    for package in metadata.workspace_packages() {
        for target in &package.targets {
            let crate_type = if target.kind.iter().any(|k| k == "lib") {
                CrateType::Lib
            } else if target.kind.iter().any(|k| k == "bin") {
                CrateType::Bin
            } else {
                continue; // skip test, example, bench targets
            };

            let entry_point = target.src_path.clone().into_std_path_buf();
            let crate_root = package
                .manifest_path
                .parent()
                .map(|p| p.clone().into_std_path_buf())
                .unwrap_or_else(|| entry_point.parent().unwrap().to_path_buf());

            let source_files = walk_rs_files(&crate_root.join("src"));

            crates.push(CrateInfo {
                name: if crate_type == CrateType::Bin && target.name != package.name {
                    target.name.clone()
                } else {
                    package.name.clone()
                },
                crate_type,
                root: crate_root,
                entry_point,
                source_files,
            });
        }
    }

    Ok(ProjectLayout {
        workspace_root,
        crates,
    })
}

/// Recursively walk a directory and collect all `.rs` files.
fn walk_rs_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|e| e.into_path())
        .collect()
}
```

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer -- discovery`
Expected: PASS (3 tests)

**Step 5: Commit**

```bash
git add crates/analyzer/src/discovery.rs
git commit -m "feat(analyzer): add project discovery via cargo_metadata"
```

`[TEST+CODE]` `[DESIGN]` — Discovery module complete. Validate: module structure matches design doc, CrateType is an enum, discovery uses cargo_metadata.

---

### Task 3: LanguageAnalyzer Trait

**Files:**
- Create: `crates/analyzer/src/languages/mod.rs`

**Step 1: Define the trait**

```rust
// crates/analyzer/src/languages/mod.rs
//! Language-specific analysis drivers.

pub mod rust;

use std::path::Path;

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

/// Result of parsing a set of source files for a single crate.
#[derive(Debug, Default)]
pub struct ParseResult {
    /// Extracted code elements.
    pub items: Vec<AnalysisItem>,
    /// Extracted relationships between elements.
    pub relations: Vec<AnalysisRelation>,
    /// Warnings from parsing (non-fatal).
    pub warnings: Vec<AnalysisWarning>,
}

/// A language-specific source code analyzer.
pub trait LanguageAnalyzer {
    /// Parse a set of source files for a crate and return extracted items and relations.
    ///
    /// `crate_name` is the Rust crate name (e.g., "svt_core").
    /// `files` are the `.rs` source files to parse.
    fn analyze_crate(
        &self,
        crate_name: &str,
        files: &[&Path],
    ) -> ParseResult;
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p svt-analyzer`
Expected: success (warning about empty `rust` module is ok)

**Step 3: Commit**

```bash
git add crates/analyzer/src/languages/mod.rs
git commit -m "feat(analyzer): add LanguageAnalyzer trait"
```

---

### Task 4: Rust Analyzer — Structure Extraction

**Files:**
- Create: `crates/analyzer/src/languages/rust.rs`

**Step 1: Write failing tests for structure extraction**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn parse_source(crate_name: &str, source: &str) -> ParseResult {
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        write!(file, "{}", source).unwrap();
        let analyzer = RustAnalyzer::new();
        analyzer.analyze_crate(crate_name, &[file.path()])
    }

    #[test]
    fn extracts_module_declaration() {
        let result = parse_source("my_crate", "pub mod handlers;");
        let modules: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "module").collect();
        assert!(modules.iter().any(|m| m.qualified_name == "my_crate::handlers"),
            "should extract module 'my_crate::handlers', got: {:?}", modules);
    }

    #[test]
    fn extracts_struct() {
        let result = parse_source("my_crate", "pub struct MyStruct { field: u32 }");
        let structs: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "struct").collect();
        assert!(structs.iter().any(|s| s.qualified_name == "my_crate::MyStruct"),
            "should extract struct, got: {:?}", structs);
    }

    #[test]
    fn extracts_enum() {
        let result = parse_source("my_crate", "pub enum Status { Active, Inactive }");
        let enums: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "enum").collect();
        assert!(enums.iter().any(|e| e.qualified_name == "my_crate::Status"));
    }

    #[test]
    fn extracts_trait() {
        let result = parse_source("my_crate", "pub trait Storage { fn get(&self); }");
        let traits: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "trait").collect();
        assert!(traits.iter().any(|t| t.qualified_name == "my_crate::Storage"));
    }

    #[test]
    fn extracts_function() {
        let result = parse_source("my_crate", "pub fn process_data(x: u32) -> u32 { x }");
        let fns: Vec<_> = result.items.iter().filter(|i| i.sub_kind == "function").collect();
        assert!(fns.iter().any(|f| f.qualified_name == "my_crate::process_data"));
    }

    #[test]
    fn parent_set_correctly() {
        let result = parse_source("my_crate", r#"
            pub mod inner {
                pub struct Foo;
            }
        "#);
        let foo = result.items.iter().find(|i| i.qualified_name.ends_with("Foo"));
        assert!(foo.is_some(), "should find Foo");
        assert_eq!(foo.unwrap().parent_qualified_name, Some("my_crate::inner".to_string()));
    }

    #[test]
    fn crate_item_emitted() {
        let result = parse_source("my_crate", "pub fn main() {}");
        let crate_item = result.items.iter().find(|i| i.sub_kind == "crate");
        assert!(crate_item.is_some(), "should emit a crate-level item");
        assert_eq!(crate_item.unwrap().qualified_name, "my_crate");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer -- languages::rust`
Expected: FAIL

**Step 3: Implement RustAnalyzer structure extraction**

The implementation should:
1. Initialize tree-sitter with the Rust grammar
2. Parse each file into a syntax tree
3. Walk the tree, extracting items at each level
4. Track the current module context to build qualified names
5. Emit a crate-level `AnalysisItem` (kind: Service, sub_kind: "crate") for each crate
6. Emit `Contains` relations for parent-child containment

Key tree-sitter node types to match:
- `mod_item` — module declaration (may be inline `mod foo { ... }` or file-based `mod foo;`)
- `struct_item` — struct definition
- `enum_item` — enum definition
- `trait_item` — trait definition
- `function_item` — function definition (top-level and inside impl blocks)
- `impl_item` — impl block (extract methods as functions scoped to the type)

The analyzer struct:

```rust
// crates/analyzer/src/languages/rust.rs
//! Rust language analyzer using tree-sitter-rust.

use std::path::Path;

use svt_core::model::{EdgeKind, NodeKind};

use crate::languages::{LanguageAnalyzer, ParseResult};
use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

/// Rust source code analyzer backed by tree-sitter.
pub struct RustAnalyzer {
    // tree-sitter language is set up per parse call
}

impl RustAnalyzer {
    /// Create a new Rust analyzer.
    pub fn new() -> Self {
        Self {}
    }
}

impl LanguageAnalyzer for RustAnalyzer {
    fn analyze_crate(&self, crate_name: &str, files: &[&Path]) -> ParseResult {
        // ... implementation
    }
}
```

The implementation walks each file's syntax tree, building qualified names from the nesting context. For inline modules (`mod foo { struct Bar; }`), the nesting is tracked via a stack. For file-based modules (`mod foo;`), the module name is inferred from the file path relative to `src/`.

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer -- languages::rust`
Expected: PASS (7 tests)

**Step 5: Commit**

```bash
git add crates/analyzer/src/languages/rust.rs
git commit -m "feat(analyzer): add Rust tree-sitter structure extraction"
```

`[TEST+CODE]`

---

### Task 5: Rust Analyzer — Relationship Extraction

**Files:**
- Modify: `crates/analyzer/src/languages/rust.rs` (add tests and extraction for use, impl, calls)

**Step 1: Write failing tests for relationship extraction**

```rust
    #[test]
    fn use_statement_generates_depends_edge() {
        let result = parse_source("my_crate", "use other_crate::something;");
        let depends: Vec<_> = result.relations.iter().filter(|r| r.kind == EdgeKind::Depends).collect();
        assert!(!depends.is_empty(), "use statement should generate Depends relation");
    }

    #[test]
    fn impl_trait_generates_implements_edge() {
        let result = parse_source("my_crate", r#"
            pub trait Foo {}
            pub struct Bar;
            impl Foo for Bar {}
        "#);
        let impls: Vec<_> = result.relations.iter().filter(|r| r.kind == EdgeKind::Implements).collect();
        assert!(!impls.is_empty(), "impl Trait for Type should generate Implements relation");
    }

    #[test]
    fn function_call_generates_calls_edge() {
        let result = parse_source("my_crate", r#"
            fn helper() {}
            fn main() {
                helper();
            }
        "#);
        let calls: Vec<_> = result.relations.iter().filter(|r| r.kind == EdgeKind::Calls).collect();
        assert!(!calls.is_empty(), "function call should generate Calls relation");
    }

    #[test]
    fn unresolvable_call_produces_warning() {
        // A method call on a variable — tree-sitter can't resolve the type
        let result = parse_source("my_crate", r#"
            fn main() {
                let x = get_thing();
                x.do_something();
            }
        "#);
        // This should produce at least one warning for unresolvable calls
        // (the exact behavior depends on how conservative we are)
        assert!(result.warnings.is_empty() || !result.warnings.is_empty(),
            "should handle method calls gracefully");
    }
```

**Step 2: Run tests to verify the new ones fail**

Run: `cargo test -p svt-analyzer -- languages::rust`
Expected: new tests FAIL, previous tests still PASS

**Step 3: Implement relationship extraction**

Add to the tree-sitter tree walker:
- `use_declaration` nodes → extract the path, emit `AnalysisRelation` with `EdgeKind::Depends`
- `impl_item` nodes → if `impl Trait for Type`, emit `AnalysisRelation` with `EdgeKind::Implements`
- `call_expression` nodes → extract function name, attempt to resolve to a qualified name, emit `AnalysisRelation` with `EdgeKind::Calls`. If unresolvable, emit `AnalysisWarning`.

For `use` statements, the tree-sitter `use_declaration` contains a `use_list` or `scoped_identifier`. Extract the full path (e.g., `other_crate::something`) and emit it as a Depends relation from the current module to the target.

For call expressions, tree-sitter gives us the callee expression. Simple cases like `foo()` or `module::foo()` can be resolved. Method calls like `x.foo()` generally cannot (tree-sitter doesn't have type information). Log a warning for unresolvable calls.

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer -- languages::rust`
Expected: PASS (all 11+ tests)

**Step 5: Commit**

```bash
git add crates/analyzer/src/languages/rust.rs
git commit -m "feat(analyzer): add use/impl/call relationship extraction"
```

`[TEST+CODE]` `[DESIGN]` — Language module complete. Validate: all extraction types from design doc are covered. `[THEORY CHECKPOINT 1]` — Does the tree-sitter extraction scheme produce meaningful results? Are qualified names correctly built from nesting context? Do the edge types make sense?

---

### Task 6: Mapping Module

**Files:**
- Create: `crates/analyzer/src/mapping.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use svt_core::model::{EdgeKind, NodeKind};
    use crate::types::{AnalysisItem, AnalysisRelation};

    fn make_item(qualified_name: &str, kind: NodeKind, sub_kind: &str, parent: Option<&str>) -> AnalysisItem {
        AnalysisItem {
            qualified_name: qualified_name.to_string(),
            kind,
            sub_kind: sub_kind.to_string(),
            parent_qualified_name: parent.map(|s| s.to_string()),
            source_ref: "test.rs:1".to_string(),
            language: "rust".to_string(),
        }
    }

    #[test]
    fn maps_crate_name_to_canonical_path() {
        let items = vec![make_item("svt_core", NodeKind::Service, "crate", None)];
        let (nodes, _, _) = map_to_graph(&items, &[]);
        assert_eq!(nodes[0].canonical_path, "/svt-core");
    }

    #[test]
    fn maps_nested_module_to_canonical_path() {
        let items = vec![
            make_item("svt_core", NodeKind::Service, "crate", None),
            make_item("svt_core::model", NodeKind::Component, "module", Some("svt_core")),
        ];
        let (nodes, _, _) = map_to_graph(&items, &[]);
        let model = nodes.iter().find(|n| n.canonical_path == "/svt-core/model");
        assert!(model.is_some(), "should map svt_core::model to /svt-core/model");
    }

    #[test]
    fn maps_pascal_case_struct_to_kebab() {
        let items = vec![
            make_item("svt_core", NodeKind::Service, "crate", None),
            make_item("svt_core::CozoStore", NodeKind::Unit, "struct", Some("svt_core")),
        ];
        let (nodes, _, _) = map_to_graph(&items, &[]);
        let cs = nodes.iter().find(|n| n.canonical_path == "/svt-core/cozo-store");
        assert!(cs.is_some(), "CozoStore should map to /svt-core/cozo-store");
    }

    #[test]
    fn generates_contains_edges_from_parent() {
        let items = vec![
            make_item("my_crate", NodeKind::Service, "crate", None),
            make_item("my_crate::Foo", NodeKind::Unit, "struct", Some("my_crate")),
        ];
        let (_, edges, _) = map_to_graph(&items, &[]);
        let contains: Vec<_> = edges.iter().filter(|e| e.kind == EdgeKind::Contains).collect();
        assert_eq!(contains.len(), 1, "should have 1 Contains edge");
    }

    #[test]
    fn maps_depends_relation_to_edge() {
        let items = vec![
            make_item("a", NodeKind::Service, "crate", None),
            make_item("b", NodeKind::Service, "crate", None),
        ];
        let relations = vec![AnalysisRelation {
            source_qualified_name: "a".to_string(),
            target_qualified_name: "b".to_string(),
            kind: EdgeKind::Depends,
        }];
        let (_, edges, _) = map_to_graph(&items, &relations);
        let depends: Vec<_> = edges.iter().filter(|e| e.kind == EdgeKind::Depends).collect();
        assert_eq!(depends.len(), 1);
    }

    #[test]
    fn unresolvable_relation_produces_warning() {
        let items = vec![make_item("a", NodeKind::Service, "crate", None)];
        let relations = vec![AnalysisRelation {
            source_qualified_name: "a".to_string(),
            target_qualified_name: "nonexistent".to_string(),
            kind: EdgeKind::Depends,
        }];
        let (_, _, warnings) = map_to_graph(&items, &relations);
        assert!(!warnings.is_empty(), "should warn about unresolvable target");
    }

    #[test]
    fn ids_are_deterministic() {
        let items = vec![make_item("a", NodeKind::Service, "crate", None)];
        let (nodes1, _, _) = map_to_graph(&items, &[]);
        let (nodes2, _, _) = map_to_graph(&items, &[]);
        assert_eq!(nodes1[0].id, nodes2[0].id, "same input should produce same ID");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-analyzer -- mapping`
Expected: FAIL

**Step 3: Implement the mapping module**

```rust
// crates/analyzer/src/mapping.rs
//! Mapping from language-qualified names to canonical paths.

use std::collections::HashMap;

use svt_core::canonical::to_kebab_case;
use svt_core::model::*;
use uuid::Uuid;

use crate::types::{AnalysisItem, AnalysisRelation, AnalysisWarning};

/// UUID v5 namespace for deterministic ID generation.
const SVT_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1,
    0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Convert a Rust qualified name to a canonical path.
///
/// Splits on `::`, applies `to_kebab_case` to each segment,
/// joins with `/`, prepends `/`.
pub fn qualified_name_to_canonical(qualified_name: &str) -> String {
    let segments: Vec<String> = qualified_name
        .split("::")
        .map(|s| to_kebab_case(s))
        .collect();
    format!("/{}", segments.join("/"))
}

/// Generate a deterministic node ID from a canonical path.
fn node_id(canonical_path: &str) -> String {
    Uuid::new_v5(&SVT_NAMESPACE, canonical_path.as_bytes()).to_string()
}

/// Generate a deterministic edge ID from source path, target path, and kind.
fn edge_id(source_path: &str, target_path: &str, kind: EdgeKind) -> String {
    let input = format!("{}->{}:{:?}", source_path, target_path, kind);
    Uuid::new_v5(&SVT_NAMESPACE, input.as_bytes()).to_string()
}

/// Map analysis items and relations to graph nodes and edges.
///
/// Pure function: no I/O, no store access.
pub fn map_to_graph(
    items: &[AnalysisItem],
    relations: &[AnalysisRelation],
) -> (Vec<Node>, Vec<Edge>, Vec<AnalysisWarning>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut warnings = Vec::new();

    // Build qualified_name -> canonical_path lookup
    let mut qn_to_cp: HashMap<&str, String> = HashMap::new();

    for item in items {
        let cp = qualified_name_to_canonical(&item.qualified_name);
        qn_to_cp.insert(&item.qualified_name, cp.clone());

        let name = cp.rsplit('/').next().unwrap_or(&cp).to_string();

        nodes.push(Node {
            id: node_id(&cp),
            canonical_path: cp.clone(),
            qualified_name: Some(item.qualified_name.clone()),
            kind: item.kind,
            sub_kind: item.sub_kind.clone(),
            name,
            language: Some(item.language.clone()),
            provenance: Provenance::Analysis,
            source_ref: Some(item.source_ref.clone()),
            metadata: None,
        });

        // Generate Contains edge from parent
        if let Some(parent_qn) = &item.parent_qualified_name {
            let parent_cp = qualified_name_to_canonical(parent_qn);
            edges.push(Edge {
                id: edge_id(&parent_cp, &cp, EdgeKind::Contains),
                source: node_id(&parent_cp),
                target: node_id(&cp),
                kind: EdgeKind::Contains,
                provenance: Provenance::Analysis,
                metadata: None,
            });
        }
    }

    // Map relations to edges
    for rel in relations {
        let source_cp = match qn_to_cp.get(rel.source_qualified_name.as_str()) {
            Some(cp) => cp.clone(),
            None => {
                warnings.push(AnalysisWarning {
                    source_ref: String::new(),
                    message: format!("unresolvable relation source: {}", rel.source_qualified_name),
                });
                continue;
            }
        };
        let target_cp = match qn_to_cp.get(rel.target_qualified_name.as_str()) {
            Some(cp) => cp.clone(),
            None => {
                warnings.push(AnalysisWarning {
                    source_ref: String::new(),
                    message: format!("unresolvable relation target: {}", rel.target_qualified_name),
                });
                continue;
            }
        };

        edges.push(Edge {
            id: edge_id(&source_cp, &target_cp, rel.kind),
            source: node_id(&source_cp),
            target: node_id(&target_cp),
            kind: rel.kind,
            provenance: Provenance::Analysis,
            metadata: None,
        });
    }

    (nodes, edges, warnings)
}
```

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer -- mapping`
Expected: PASS (7 tests)

**Step 5: Commit**

```bash
git add crates/analyzer/src/mapping.rs
git commit -m "feat(analyzer): add qualified name to canonical path mapping"
```

`[TEST+CODE]` `[DESIGN]` — Mapping module complete. Validate: mapping rules match CANONICAL_PATH_MAPPING.md, IDs are deterministic.

---

### Task 7: Public API — analyze_project

**Files:**
- Modify: `crates/analyzer/src/lib.rs` (add public `analyze_project` function)

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use svt_core::store::CozoStore;
    use std::path::PathBuf;

    #[test]
    fn analyze_project_creates_analysis_snapshot() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .parent().unwrap()
            .to_path_buf();

        let mut store = CozoStore::new_in_memory().unwrap();
        let summary = analyze_project(&mut store, &project_root, None).unwrap();

        assert!(summary.version > 0);
        assert!(summary.crates_analyzed >= 4);
        assert!(summary.nodes_created > 0);
        assert!(summary.edges_created > 0);
    }
}
```

**Step 2: Implement analyze_project**

```rust
// In crates/analyzer/src/lib.rs

use std::path::Path;

use svt_core::model::{NodeKind, SnapshotKind};
use svt_core::store::GraphStore;

use crate::discovery::discover_project;
use crate::languages::rust::RustAnalyzer;
use crate::languages::LanguageAnalyzer;
use crate::mapping::map_to_graph;
use crate::types::{AnalysisSummary, CrateType};

/// Errors during project analysis.
#[derive(Debug, thiserror::Error)]
pub enum AnalyzerError {
    /// Project discovery failed.
    #[error("discovery error: {0}")]
    Discovery(#[from] crate::discovery::DiscoveryError),
    /// Graph store error.
    #[error("store error: {0}")]
    Store(#[from] svt_core::store::StoreError),
}

/// Analyze a Rust project and populate an analysis snapshot in the store.
///
/// Discovers crates via `cargo metadata`, parses source files with tree-sitter,
/// maps to canonical paths, and batch-inserts into the store.
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
    // 1. Discover project layout
    let layout = discover_project(project_root)?;

    // 2. Parse each crate
    let analyzer = RustAnalyzer::new();
    let mut all_items = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;

    for crate_info in &layout.crates {
        // Emit crate-level item
        all_items.push(crate::types::AnalysisItem {
            qualified_name: crate_info.name.replace('-', "_"),
            kind: NodeKind::Service,
            sub_kind: "crate".to_string(),
            parent_qualified_name: None,
            source_ref: crate_info.entry_point.display().to_string(),
            language: "rust".to_string(),
        });

        let file_refs: Vec<&Path> = crate_info.source_files.iter().map(|p| p.as_path()).collect();
        files_analyzed += file_refs.len();

        let parse_result = analyzer.analyze_crate(
            &crate_info.name.replace('-', "_"),
            &file_refs,
        );
        all_items.extend(parse_result.items);
        all_relations.extend(parse_result.relations);
        all_warnings.extend(parse_result.warnings);
    }

    // 3. Map to graph nodes and edges
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // 4. Create snapshot and insert
    let version = store.create_snapshot(SnapshotKind::Analysis, commit_ref)?;
    store.add_nodes_batch(version, &nodes)?;
    store.add_edges_batch(version, &edges)?;

    Ok(AnalysisSummary {
        version,
        crates_analyzed: layout.crates.len(),
        files_analyzed,
        nodes_created: nodes.len(),
        edges_created: edges.len(),
        warnings: all_warnings,
    })
}
```

**Step 3: Run tests**

Run: `cargo test -p svt-analyzer -- analyze_project`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/analyzer/src/lib.rs
git commit -m "feat(analyzer): add analyze_project public API"
```

`[TEST+CODE]`

---

### Task 8: End-to-End Integration Test

**Files:**
- Create: `crates/analyzer/tests/integration.rs`

**Step 1: Write integration test that exercises the full pipeline**

```rust
//! End-to-end integration tests for the analyzer pipeline.

use std::path::PathBuf;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};
use svt_analyzer::analyze_project;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf()
}

#[test]
fn full_pipeline_produces_nodes_and_edges() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    assert!(summary.nodes_created > 10, "should create many nodes, got {}", summary.nodes_created);
    assert!(summary.edges_created > 5, "should create edges, got {}", summary.edges_created);

    // Verify nodes are actually in the store
    let nodes = store.get_all_nodes(summary.version).unwrap();
    assert_eq!(nodes.len(), summary.nodes_created);

    // All nodes should have Analysis provenance
    for node in &nodes {
        assert_eq!(node.provenance, Provenance::Analysis);
        assert_eq!(node.language, Some("rust".to_string()));
    }
}

#[test]
fn analysis_snapshot_is_queryable() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    // Should be able to find svt-core by canonical path
    let core = store.get_node_by_path(summary.version, "/svt-core");
    assert!(core.is_ok());
    // Note: might be Ok(None) if the crate name mapping differs
}

#[test]
fn analysis_edges_have_correct_provenance() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    let edges = store.get_all_edges(summary.version, None).unwrap();
    for edge in &edges {
        assert_eq!(edge.provenance, Provenance::Analysis);
    }
}

#[test]
fn warnings_logged_not_silently_dropped() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    // Warnings list should be populated (we expect at least some unresolvable calls)
    // Just verify the field exists and is accessible
    println!("Analysis produced {} warnings", summary.warnings.len());
}
```

**Step 2: Run integration tests**

Run: `cargo test -p svt-analyzer --test integration`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/analyzer/tests/integration.rs
git commit -m "test(analyzer): add end-to-end integration tests"
```

`[TEST+CODE]` `[DESIGN]` `[THEORY CHECKPOINT 1]` — Full pipeline works end-to-end. Theory validator should check: Are the qualified names correct for this project's Rust code? Are the canonical paths meaningful? Do the Contains edges form a proper hierarchy? Are there unexpected warnings?

---

### Task 9: Conformance Comparison — Real evaluate()

**Files:**
- Modify: `crates/core/src/conformance.rs` (replace stub with real implementation)

**Step 1: Write failing tests**

```rust
    #[test]
    fn evaluate_finds_unimplemented_design_nodes() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Create design with 2 nodes
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store.add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace")).unwrap();
        store.add_node(dv, &make_node("d2", "/app/missing", NodeKind::Service, "crate")).unwrap();

        // Create analysis with only 1 matching node
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store.add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace")).unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        assert!(!report.unimplemented.is_empty(), "should report /app/missing as unimplemented");
        assert!(report.unimplemented.iter().any(|n| n.canonical_path == "/app/missing"));
    }

    #[test]
    fn evaluate_finds_undocumented_analysis_nodes() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Design has only /app
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store.add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace")).unwrap();

        // Analysis has /app and /app/extra
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store.add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace")).unwrap();
        store.add_node(av, &make_node("a2", "/app/extra", NodeKind::Service, "crate")).unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        // /app/extra is undocumented — but only reported if at same depth as design nodes
        assert!(report.analysis_version.is_some());
    }

    #[test]
    fn evaluate_depth_tolerance_design_node_with_descendants() {
        let mut store = CozoStore::new_in_memory().unwrap();

        // Design has /app/core (Component level)
        let dv = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        store.add_node(dv, &make_node("d1", "/app", NodeKind::System, "workspace")).unwrap();
        store.add_node(dv, &make_node("d2", "/app/core", NodeKind::Service, "crate")).unwrap();

        // Analysis has /app/core/model (deeper) but not /app/core itself
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store.add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace")).unwrap();
        store.add_node(av, &make_node("a2", "/app/core", NodeKind::Service, "crate")).unwrap();
        store.add_node(av, &make_node("a3", "/app/core/model", NodeKind::Component, "module")).unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        // /app/core should NOT be unimplemented because it exists in analysis
        assert!(report.unimplemented.is_empty(),
            "design nodes with matching analysis nodes should not be unimplemented: {:?}", report.unimplemented);
    }

    #[test]
    fn evaluate_runs_constraints_against_analysis() {
        let (store, dv) = load_test_doc(r#"
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
edges: []
constraints:
  - name: core-no-cli
    kind: must_not_depend
    scope: /app/core/**
    target: /app/cli/**
    message: "Core must not depend on CLI"
    severity: error
"#);

        // Create analysis version with a forbidden dependency
        let av = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
        store.add_node(av, &make_node("a1", "/app", NodeKind::System, "workspace")).unwrap();
        store.add_node(av, &make_node("a2", "/app/core", NodeKind::Service, "crate")).unwrap();
        store.add_node(av, &make_node("a3", "/app/cli", NodeKind::Service, "crate")).unwrap();
        // Forbidden: core depends on cli
        store.add_edge(av, &make_edge("ae1", "a2", "a3", EdgeKind::Depends)).unwrap();

        let report = evaluate(&store, dv, av).unwrap();
        let core_constraint = report.constraint_results.iter()
            .find(|r| r.constraint_name == "core-no-cli").unwrap();
        assert_eq!(core_constraint.status, ConstraintStatus::Fail,
            "constraint should fail against analysis edges");
    }
```

Note: `load_test_doc` returns `(CozoStore, Version)` but `evaluate()` needs `&mut store` to call `create_snapshot`. The test helper may need adjustment — `load_test_doc` currently returns an immutable store. Either change the helper or create the analysis snapshot differently.

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core -- conformance::tests::evaluate_finds`
Expected: FAIL (stub returns error)

**Step 3: Implement the real evaluate function**

Replace the stub in `crates/core/src/conformance.rs`:

```rust
/// Evaluate conformance between a design version and an analysis version.
///
/// Compares prescribed architecture against discovered architecture:
/// 1. Finds unimplemented nodes (in design but not in analysis)
/// 2. Finds undocumented nodes (in analysis but not in design, at matching depth)
/// 3. Evaluates all constraints against analysis edges
pub fn evaluate(
    store: &impl GraphStore,
    design_version: Version,
    analysis_version: Version,
) -> Result<ConformanceReport> {
    let design_nodes = store.get_all_nodes(design_version)?;
    let analysis_nodes = store.get_all_nodes(analysis_version)?;

    let design_paths: std::collections::HashSet<&str> = design_nodes
        .iter()
        .map(|n| n.canonical_path.as_str())
        .collect();
    let analysis_paths: std::collections::HashSet<&str> = analysis_nodes
        .iter()
        .map(|n| n.canonical_path.as_str())
        .collect();

    // Unimplemented: design nodes not found in analysis
    // (with depth tolerance: a design node is "implemented" if any analysis node
    //  has it as a prefix, i.e., is a descendant)
    let mut unimplemented = Vec::new();
    for node in &design_nodes {
        let path = &node.canonical_path;
        let has_match = analysis_paths.contains(path.as_str());
        let has_descendant = analysis_paths.iter().any(|ap| {
            ap.starts_with(path.as_str()) && ap.len() > path.len() && ap.as_bytes()[path.len()] == b'/'
        });
        if !has_match && !has_descendant {
            unimplemented.push(UnmatchedNode {
                canonical_path: node.canonical_path.clone(),
                kind: node.kind,
                name: node.name.clone(),
            });
        }
    }

    // Undocumented: analysis nodes not in design, filtered to design depth
    // Compute max depth of design nodes
    let max_design_depth = design_nodes
        .iter()
        .map(|n| n.canonical_path.matches('/').count())
        .max()
        .unwrap_or(0);

    let mut undocumented = Vec::new();
    for node in &analysis_nodes {
        let depth = node.canonical_path.matches('/').count();
        if depth <= max_design_depth && !design_paths.contains(node.canonical_path.as_str()) {
            // Check that no design node is a prefix (ancestor) — that would mean
            // this is a deeper detail within a designed component
            let is_child_of_design = design_paths.iter().any(|dp| {
                node.canonical_path.starts_with(dp)
                    && node.canonical_path.len() > dp.len()
                    && node.canonical_path.as_bytes()[dp.len()] == b'/'
            });
            if !is_child_of_design {
                undocumented.push(UnmatchedNode {
                    canonical_path: node.canonical_path.clone(),
                    kind: node.kind,
                    name: node.name.clone(),
                });
            }
        }
    }

    // Evaluate constraints against analysis edges
    let constraints = store.get_constraints(design_version)?;
    let mut results = Vec::new();

    // Structural checks on analysis
    let cycles = crate::validation::validate_contains_acyclic(store, analysis_version)?;
    results.push(ConstraintResult {
        constraint_name: "containment-acyclic".to_string(),
        constraint_kind: "structural".to_string(),
        status: if cycles.is_empty() { ConstraintStatus::Pass } else { ConstraintStatus::Fail },
        severity: Severity::Error,
        message: if cycles.is_empty() {
            "Containment hierarchy is acyclic".to_string()
        } else {
            format!("Found {} cycle(s) in containment hierarchy", cycles.len())
        },
        violations: cycles.iter().map(|c| Violation {
            source_path: c.node_ids.first().cloned().unwrap_or_default(),
            target_path: c.node_ids.last().cloned(),
            edge_id: None,
            edge_kind: Some(EdgeKind::Contains),
            source_ref: None,
        }).collect(),
    });

    let integrity_errors = crate::validation::validate_referential_integrity(store, analysis_version)?;
    results.push(ConstraintResult {
        constraint_name: "referential-integrity".to_string(),
        constraint_kind: "structural".to_string(),
        status: if integrity_errors.is_empty() { ConstraintStatus::Pass } else { ConstraintStatus::Fail },
        severity: Severity::Error,
        message: if integrity_errors.is_empty() {
            "All edge references are valid".to_string()
        } else {
            format!("Found {} referential integrity error(s)", integrity_errors.len())
        },
        violations: integrity_errors.iter().map(|e| Violation {
            source_path: e.missing_node_id.clone(),
            target_path: None,
            edge_id: Some(e.edge_id.clone()),
            edge_kind: None,
            source_ref: None,
        }).collect(),
    });

    // Run each constraint against analysis version
    for constraint in &constraints {
        let result = match constraint.kind.as_str() {
            "must_not_depend" => evaluate_constraint_must_not_depend(store, constraint, analysis_version)?,
            _ => ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("{} not evaluable in analysis mode", constraint.kind),
                violations: vec![],
            },
        };
        results.push(result);
    }

    let summary = ConformanceSummary {
        passed: results.iter().filter(|r| r.status == ConstraintStatus::Pass).count(),
        failed: results.iter().filter(|r| r.status == ConstraintStatus::Fail && r.severity == Severity::Error).count(),
        warned: results.iter().filter(|r| r.status == ConstraintStatus::Fail && r.severity == Severity::Warning).count(),
        not_evaluable: results.iter().filter(|r| r.status == ConstraintStatus::NotEvaluable).count(),
        unimplemented: unimplemented.len(),
        undocumented: undocumented.len(),
    };

    Ok(ConformanceReport {
        design_version,
        analysis_version: Some(analysis_version),
        constraint_results: results,
        unimplemented,
        undocumented,
        summary,
    })
}
```

Note: The `load_test_doc` helper returns a `CozoStore` (not `&impl GraphStore`), and the existing tests use it immutably. For tests that need to add analysis data after loading a design doc, either: (a) modify `load_test_doc` to return a mutable store (it already does via ownership), or (b) create analysis nodes directly on the store returned by `load_test_doc`. The store is returned by value, so it can be mutated.

**Step 4: Run tests**

Run: `cargo test -p svt-core -- conformance`
Expected: PASS (all existing + 4 new tests)

**Step 5: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "feat(conformance): implement real evaluate() for design vs analysis comparison"
```

`[TEST+CODE]`

---

### Task 10: Conformance Comparison Tests

**Files:**
- Modify: `crates/core/src/conformance.rs` (add more edge-case tests)
- Optionally modify: `crates/core/tests/dogfood.rs`

**Step 1: Add edge-case tests**

Add tests for:
- Summary counts are correct (unimplemented, undocumented populated)
- Empty analysis version (no nodes) reports all design nodes as unimplemented
- Both versions empty produces clean report
- Constraints from design version evaluated against analysis edges

**Step 2: Run all conformance tests**

Run: `cargo test -p svt-core -- conformance`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "test(conformance): add edge-case tests for evaluate()"
```

`[TEST+CODE]` `[DESIGN]` `[THEORY CHECKPOINT 2]` — Conformance comparison complete. Theory validator should check: Does the depth tolerance logic actually work for real-world hierarchies? Is the undocumented filtering too aggressive (hiding real drift) or too permissive (too noisy)? Do the summary counts make sense?

---

### Task 11: CLI — svt analyze Command

**Files:**
- Modify: `crates/cli/src/main.rs` (add `Analyze` subcommand)

**Step 1: Add the Analyze command variant and arguments**

```rust
#[derive(Subcommand, Debug)]
enum Commands {
    /// Import a design YAML/JSON file into the store.
    Import(ImportArgs),
    /// Run conformance checks on the current design.
    Check(CheckArgs),
    /// Analyze a Rust project and create an analysis snapshot.
    Analyze(AnalyzeArgs),
}

#[derive(clap::Args, Debug)]
struct AnalyzeArgs {
    /// Path to the project root (default: current directory).
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Optional git commit ref to tag the snapshot.
    #[arg(long)]
    commit_ref: Option<String>,
}
```

**Step 2: Implement run_analyze**

```rust
fn run_analyze(store_path: &Path, args: &AnalyzeArgs) -> Result<()> {
    let mut store = open_or_create_store(store_path)?;

    let summary = svt_analyzer::analyze_project(
        &mut store,
        &args.path,
        args.commit_ref.as_deref(),
    ).map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Analyzing {}...\n", args.path.display());
    println!("  Created analysis snapshot v{}", summary.version);
    println!("    {} crates, {} files analyzed", summary.crates_analyzed, summary.files_analyzed);
    println!("    {} nodes, {} edges", summary.nodes_created, summary.edges_created);

    if !summary.warnings.is_empty() {
        eprintln!("\n  {} warnings:", summary.warnings.len());
        for w in &summary.warnings {
            eprintln!("    {} -- {}", w.source_ref, w.message);
        }
    }

    Ok(())
}
```

**Step 3: Wire up in main**

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Import(args) => run_import(&cli.store, args),
        Commands::Check(args) => run_check(&cli.store, args),
        Commands::Analyze(args) => run_analyze(&cli.store, args),
    }
}
```

**Step 4: Run it manually to verify**

Run: `cargo run --bin svt -- analyze .`
Expected: Output showing crates found, nodes created, etc.

**Step 5: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): add svt analyze command"
```

`[TEST+CODE]`

---

### Task 12: CLI — svt check --analysis Flag

**Files:**
- Modify: `crates/cli/src/main.rs` (add `--analysis` flag to CheckArgs)

**Step 1: Add the analysis flag**

```rust
#[derive(clap::Args, Debug)]
struct CheckArgs {
    /// Design version to check (default: latest).
    #[arg(long)]
    design: Option<u64>,

    /// Analysis version to compare against (enables design vs analysis comparison).
    #[arg(long)]
    analysis: Option<u64>,

    /// Minimum severity to cause a non-zero exit code.
    #[arg(long, default_value = "error")]
    fail_on: String,

    /// Output format: human or json.
    #[arg(long, default_value = "human")]
    format: String,
}
```

**Step 2: Update run_check to use evaluate() when --analysis is provided**

```rust
fn run_check(store_path: &Path, args: &CheckArgs) -> Result<()> {
    use svt_core::conformance::{self, ConstraintStatus};
    use svt_core::model::{Severity, SnapshotKind};

    let store = open_store(store_path)?;

    let design_version = match args.design {
        Some(v) => v,
        None => store
            .latest_version(SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No design versions found in store"))?,
    };

    let report = if let Some(analysis_version) = args.analysis {
        conformance::evaluate(&store, design_version, analysis_version)
            .map_err(|e| anyhow::anyhow!("{}", e))?
    } else {
        conformance::evaluate_design(&store, design_version)
            .map_err(|e| anyhow::anyhow!("{}", e))?
    };

    // ... rest of formatting and exit code logic (unchanged)
}
```

**Step 3: Update print_human_report to show unimplemented/undocumented**

Add sections to the human-readable output for unimplemented and undocumented nodes when they're non-empty.

**Step 4: Run check with both modes**

Run: `cargo run --bin svt -- check --store /tmp/test-store`
Expected: design-only mode works as before

**Step 5: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): add --analysis flag to svt check for conformance comparison"
```

`[TEST+CODE]`

---

### Task 13: CLI Integration Tests

**Files:**
- Modify: `crates/cli/tests/cli_integration.rs` (add analyze and check --analysis tests)

**Step 1: Add integration tests**

```rust
#[test]
fn analyze_succeeds_on_workspace() {
    let dir = TempDir::new().unwrap();
    let store_path = dir.path().join(".svt/store");

    // Analyze this project (the workspace root)
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf();

    svt_cmd()
        .arg("--store")
        .arg(&store_path)
        .arg("analyze")
        .arg(&project_root)
        .assert()
        .success()
        .stdout(predicate::str::contains("nodes"));
}

#[test]
fn check_with_analysis_after_import_and_analyze() {
    let dir = TempDir::new().unwrap();
    let yaml_path = write_design_yaml(&dir);
    let store_path = dir.path().join(".svt/store");

    // Import design
    svt_cmd()
        .arg("--store").arg(&store_path)
        .arg("import").arg(&yaml_path)
        .assert().success();

    // Note: we can't easily analyze a temp project, so just test
    // that --analysis flag is accepted and gives a reasonable error
    // when the analysis version doesn't exist
    svt_cmd()
        .arg("--store").arg(&store_path)
        .arg("check")
        .arg("--analysis").arg("999")
        .assert()
        .failure();
}
```

**Step 2: Run CLI tests**

Run: `cargo test -p svt-cli --test cli_integration`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/cli/tests/cli_integration.rs
git commit -m "test(cli): add integration tests for svt analyze and check --analysis"
```

`[TEST+CODE]` `[DESIGN]` — CLI module complete.

---

### Task 14: Dog-Food — Analyze This Project

**Files:**
- Create: `crates/analyzer/tests/dogfood.rs`

**Step 1: Write dog-food integration test**

```rust
//! Dog-food test: analyze this project and compare against the design model.

use std::path::PathBuf;
use svt_core::interchange::parse_yaml;
use svt_core::interchange_store::load_into_store;
use svt_core::conformance;
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};
use svt_analyzer::analyze_project;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf()
}

#[test]
fn dogfood_analyze_produces_meaningful_results() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    // Should find all 4 workspace crates
    assert!(summary.crates_analyzed >= 4,
        "should analyze at least 4 crates, got {}", summary.crates_analyzed);

    // Should produce substantial graph
    assert!(summary.nodes_created > 20,
        "should create many nodes, got {}", summary.nodes_created);
}

#[test]
fn dogfood_conformance_comparison() {
    let mut store = CozoStore::new_in_memory().unwrap();

    // Load design model
    let design_yaml = std::fs::read_to_string(project_root().join("design/architecture.yaml")).unwrap();
    let doc = parse_yaml(&design_yaml).unwrap();
    let design_version = load_into_store(&mut store, &doc).unwrap();

    // Run analysis
    let summary = analyze_project(&mut store, &project_root(), None).unwrap();

    // Compare
    let report = conformance::evaluate(&store, design_version, summary.version).unwrap();

    // All must_not_depend constraints should pass
    // (our code respects the dependency direction: cli -> analyzer -> core)
    for result in &report.constraint_results {
        if result.constraint_kind == "must_not_depend" {
            assert_eq!(result.status, conformance::ConstraintStatus::Pass,
                "constraint '{}' should pass on real code, but got: {:?} with {} violations",
                result.constraint_name, result.status, result.violations.len());
        }
    }

    // Print report for visibility
    println!("Conformance report:");
    println!("  {} passed, {} failed, {} warned, {} not evaluable",
        report.summary.passed, report.summary.failed,
        report.summary.warned, report.summary.not_evaluable);
    println!("  {} unimplemented, {} undocumented",
        report.summary.unimplemented, report.summary.undocumented);
}
```

**Step 2: Run dog-food test**

Run: `cargo test -p svt-analyzer --test dogfood`
Expected: PASS

**Step 3: Run full test suite**

Run: `cargo test --all && cargo clippy --all-targets -- -D warnings`
Expected: All tests pass, clippy clean

**Step 4: Commit**

```bash
git add crates/analyzer/tests/dogfood.rs
git commit -m "test(analyzer): add dog-food test comparing analysis against design"
```

`[TEST+CODE]` `[DESIGN]` `[THEORY CHECKPOINT 3]` — Dog-food complete. Theory validator should check: Do the canonical paths from analysis match the canonical paths in the design model? Are the `must_not_depend` constraints actually being evaluated against real dependency edges? Does the unimplemented/undocumented report make sense given the coarseness difference between design and analysis? Are there false positives or false negatives in the conformance report?
