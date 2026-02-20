# Known Gaps Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor `analyze_project()` to use registry-based dispatch via `LanguageOrchestrator` trait, aggregate noisy warnings, and update PROGRESS.md.

**Architecture:** Extract the 4 hardcoded language phases into a `LanguageOrchestrator` trait with `discover()`, `analyze()`, `emit_structural_items()`, and `post_process()` methods. Each language gets its own orchestrator implementation. The `analyze_project()` function iterates over registered orchestrators instead of hardcoding phases. Warning aggregation happens in the Rust analyzer's `visit_call_expressions()` function, replacing per-method-call warnings with a per-file count.

**Tech Stack:** Rust 2021, tree-sitter, svt-core, svt-analyzer crate

---

### Task 1: Create `LanguageUnit` type and `LanguageOrchestrator` trait

**Files:**
- Create: `crates/analyzer/src/orchestrator.rs`
- Modify: `crates/analyzer/src/lib.rs` (add `pub mod orchestrator;`)

**Step 1: Write the failing test**

Add to the bottom of `crates/analyzer/src/orchestrator.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn language_unit_has_required_fields() {
        let unit = LanguageUnit {
            name: "test-pkg".to_string(),
            language: "test".to_string(),
            root: PathBuf::from("/tmp"),
            source_root: PathBuf::from("/tmp/src"),
            source_files: vec![PathBuf::from("/tmp/src/main.rs")],
            top_level_kind: svt_core::model::NodeKind::Service,
            top_level_sub_kind: "crate".to_string(),
            source_ref: "/tmp/src/main.rs".to_string(),
        };
        assert_eq!(unit.name, "test-pkg");
        assert_eq!(unit.language, "test");
        assert_eq!(unit.source_files.len(), 1);
    }

    #[test]
    fn orchestrator_registry_with_defaults_has_all_languages() {
        let registry = OrchestratorRegistry::with_defaults();
        let mut ids: Vec<&str> = registry.orchestrators().iter().map(|o| o.language_id()).collect();
        ids.sort();
        assert_eq!(ids, vec!["go", "python", "rust", "typescript"]);
    }

    #[test]
    fn orchestrator_registry_register_adds_orchestrator() {
        let mut registry = OrchestratorRegistry::new();
        assert!(registry.orchestrators().is_empty());
        registry.register(Box::new(super::rust::RustOrchestrator::new()));
        assert_eq!(registry.orchestrators().len(), 1);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer orchestrator::tests -- --no-run 2>&1`
Expected: FAIL — module `orchestrator` doesn't exist yet

**Step 3: Write minimal implementation**

Create `crates/analyzer/src/orchestrator.rs`:

```rust
//! Language orchestrators that bundle discovery, analysis, and post-processing.

pub mod go;
pub mod python;
pub mod rust;
pub mod typescript;

use std::path::Path;

use svt_core::model::NodeKind;

use crate::languages::ParseResult;
use crate::types::AnalysisItem;

/// A discovered package/module/crate for any language.
#[derive(Debug, Clone)]
pub struct LanguageUnit {
    /// Package/crate/module name.
    pub name: String,
    /// Language identifier (e.g., "rust", "go").
    pub language: String,
    /// Root directory of the unit.
    pub root: std::path::PathBuf,
    /// Source root directory.
    pub source_root: std::path::PathBuf,
    /// All source files.
    pub source_files: Vec<std::path::PathBuf>,
    /// NodeKind for the top-level item.
    pub top_level_kind: NodeKind,
    /// Sub-kind for the top-level item (e.g., "crate", "package", "module").
    pub top_level_sub_kind: String,
    /// Source reference for the top-level item.
    pub source_ref: String,
}

/// Orchestrates discovery, analysis, and post-processing for a language.
///
/// Each language implements this trait to participate in the analysis pipeline.
/// The pipeline calls `discover()` first, then for each returned unit:
/// 1. Emits a top-level node from unit fields (common)
/// 2. Calls `emit_structural_items()` for additional structure (e.g., TS modules)
/// 3. Calls `analyze()` to parse source files
/// 4. Calls `post_process()` for language-specific fixups (e.g., TS reparenting)
pub trait LanguageOrchestrator: Send + Sync {
    /// Unique language identifier.
    fn language_id(&self) -> &str;

    /// Discover packages/crates/modules in the project root.
    fn discover(&self, root: &Path) -> Vec<LanguageUnit>;

    /// Analyze source files for a single unit.
    fn analyze(&self, unit: &LanguageUnit) -> ParseResult;

    /// Emit additional structural items (e.g., directory/file modules for TypeScript).
    ///
    /// Default: no additional items.
    fn emit_structural_items(&self, _unit: &LanguageUnit) -> Vec<AnalysisItem> {
        vec![]
    }

    /// Post-process analysis results (e.g., reparent items, resolve imports).
    ///
    /// Default: no post-processing.
    fn post_process(&self, _unit: &LanguageUnit, _result: &mut ParseResult) {}
}

/// Registry of language orchestrators.
pub struct OrchestratorRegistry {
    orchestrators: Vec<Box<dyn LanguageOrchestrator>>,
}

impl OrchestratorRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            orchestrators: Vec::new(),
        }
    }

    /// Create a registry with all built-in orchestrators pre-registered.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(rust::RustOrchestrator::new()));
        registry.register(Box::new(typescript::TypeScriptOrchestrator::new()));
        registry.register(Box::new(go::GoOrchestrator::new()));
        registry.register(Box::new(python::PythonOrchestrator::new()));
        registry
    }

    /// Register an orchestrator.
    pub fn register(&mut self, orchestrator: Box<dyn LanguageOrchestrator>) {
        self.orchestrators.push(orchestrator);
    }

    /// Get all registered orchestrators.
    #[must_use]
    pub fn orchestrators(&self) -> &[Box<dyn LanguageOrchestrator>] {
        &self.orchestrators
    }
}

impl Default for OrchestratorRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
```

Also add `pub mod orchestrator;` to `crates/analyzer/src/lib.rs` (after the existing module declarations).

Note: The sub-modules (`go`, `python`, `rust`, `typescript`) are created in Tasks 2-5. For now, create minimal stub files so the module compiles. Each stub file:

`crates/analyzer/src/orchestrator/go.rs`:
```rust
//! Go language orchestrator.
// Full implementation in Task 4.
```

`crates/analyzer/src/orchestrator/python.rs`:
```rust
//! Python language orchestrator.
// Full implementation in Task 5.
```

`crates/analyzer/src/orchestrator/rust.rs`:
```rust
//! Rust language orchestrator.
// Full implementation in Task 2.
```

`crates/analyzer/src/orchestrator/typescript.rs`:
```rust
//! TypeScript language orchestrator.
// Full implementation in Task 3.
```

Wait — the tests reference the orchestrator types. So each stub needs at least the struct + `new()`:

`crates/analyzer/src/orchestrator/rust.rs`:
```rust
//! Rust language orchestrator.

use std::path::Path;
use crate::languages::ParseResult;
use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Rust projects.
#[derive(Debug)]
pub struct RustOrchestrator {
    _private: (),
}

impl RustOrchestrator {
    /// Create a new `RustOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for RustOrchestrator {
    fn default() -> Self { Self::new() }
}

impl LanguageOrchestrator for RustOrchestrator {
    fn language_id(&self) -> &str { "rust" }
    fn discover(&self, _root: &Path) -> Vec<LanguageUnit> { vec![] }
    fn analyze(&self, _unit: &LanguageUnit) -> ParseResult { ParseResult::default() }
}
```

Same pattern for `go.rs` ("go", `GoOrchestrator`), `python.rs` ("python", `PythonOrchestrator`), `typescript.rs` ("typescript", `TypeScriptOrchestrator`).

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer orchestrator::tests -v`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add crates/analyzer/src/orchestrator.rs crates/analyzer/src/orchestrator/ crates/analyzer/src/lib.rs
git commit -m "feat(analyzer): add LanguageOrchestrator trait and OrchestratorRegistry"
```

---

### Task 2: Implement `RustOrchestrator`

**Files:**
- Modify: `crates/analyzer/src/orchestrator/rust.rs`

**Step 1: Write the failing test**

Add to the bottom of `crates/analyzer/src/orchestrator/rust.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_orchestrator_language_id() {
        let orch = RustOrchestrator::new();
        assert_eq!(orch.language_id(), "rust");
    }

    #[test]
    fn rust_orchestrator_discovers_workspace_crates() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap().parent().unwrap().to_path_buf();
        let orch = RustOrchestrator::new();
        let units = orch.discover(&project_root);
        // Should find at least svt-core, svt-analyzer, svt-cli, svt-server
        assert!(units.len() >= 4, "should discover at least 4 crates, got {}", units.len());
        assert!(units.iter().all(|u| u.language == "rust"));
        assert!(units.iter().all(|u| u.top_level_sub_kind == "crate"));
    }

    #[test]
    fn rust_orchestrator_emits_workspace_root_as_extra_item() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap().parent().unwrap().to_path_buf();
        let orch = RustOrchestrator::new();
        let extra = orch.extra_items(&project_root);
        // Should have a workspace root node
        assert!(extra.len() >= 1, "should emit workspace root item");
        assert_eq!(extra[0].sub_kind, "workspace");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer orchestrator::rust::tests -v`
Expected: FAIL — `discover` returns empty, `extra_items` doesn't exist

**Step 3: Write implementation**

Replace the stub in `crates/analyzer/src/orchestrator/rust.rs` with:

```rust
//! Rust language orchestrator.
//!
//! Discovers Rust crates via `cargo metadata` and delegates parsing to
//! `RustAnalyzer`. Handles workspace detection and qualified name mapping.

use std::path::Path;

use svt_core::model::NodeKind;

use crate::discovery::discover_project;
use crate::languages::rust::RustAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};
use crate::types::AnalysisItem;

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Rust projects.
#[derive(Debug)]
pub struct RustOrchestrator {
    analyzer: RustAnalyzer,
}

impl RustOrchestrator {
    /// Create a new `RustOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: RustAnalyzer::new(),
        }
    }

    /// Return extra items that don't belong to any single unit (e.g., workspace root).
    ///
    /// Called once per project, before iterating over units.
    pub fn extra_items(&self, project_root: &Path) -> Vec<AnalysisItem> {
        let layout = match discover_project(project_root) {
            Ok(l) => l,
            Err(_) => return vec![],
        };
        let mut items = Vec::new();
        if let Some(ref ws_name) = layout.workspace_name {
            items.push(AnalysisItem {
                qualified_name: ws_name.replace('-', "_"),
                kind: NodeKind::System,
                sub_kind: "workspace".to_string(),
                parent_qualified_name: None,
                source_ref: layout.workspace_root.display().to_string(),
                language: "rust".to_string(),
            });
        }
        items
    }
}

impl Default for RustOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguageOrchestrator for RustOrchestrator {
    fn language_id(&self) -> &str {
        "rust"
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        let layout = match discover_project(root) {
            Ok(l) => l,
            Err(_) => return vec![],
        };

        layout
            .crates
            .iter()
            .map(|c| {
                let qn = workspace_qualified_name(&c.name, layout.workspace_name.as_deref());
                LanguageUnit {
                    name: qn,
                    language: "rust".to_string(),
                    root: c.root.clone(),
                    source_root: c.root.join("src"),
                    source_files: c.source_files.clone(),
                    top_level_kind: NodeKind::Service,
                    top_level_sub_kind: "crate".to_string(),
                    source_ref: c.entry_point.display().to_string(),
                }
            })
            .collect()
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.analyzer.analyze_crate(&unit.name, &file_refs)
    }
}

/// Convert a package name to a qualified name, splitting workspace prefix.
fn workspace_qualified_name(package_name: &str, workspace_name: Option<&str>) -> String {
    if let Some(ws) = workspace_name {
        let prefix = format!("{ws}-");
        if let Some(suffix) = package_name.strip_prefix(&prefix) {
            return format!("{}::{}", ws.replace('-', "_"), suffix.replace('-', "_"));
        }
    }
    package_name.replace('-', "_")
}
```

Note: `workspace_qualified_name` is moved here from `lib.rs`. It will need the `parent_qualified_name` for crate items to be set. The `LanguageUnit` doesn't carry parent info — that's set by the caller when it knows the workspace context. We handle this by adding an optional `parent_qualified_name` field to `LanguageUnit`, or by having `RustOrchestrator::discover()` set a parent on each unit.

Add a `parent_qualified_name` field to `LanguageUnit`:

```rust
pub struct LanguageUnit {
    // ... existing fields ...
    /// Parent qualified name for the top-level item (e.g., workspace name for crates).
    pub parent_qualified_name: Option<String>,
}
```

And in the Rust orchestrator's `discover()`, set the parent:

```rust
parent_qualified_name: layout.workspace_name.as_ref().map(|ws| ws.replace('-', "_")),
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer orchestrator::rust::tests -v`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add crates/analyzer/src/orchestrator/rust.rs crates/analyzer/src/orchestrator.rs
git commit -m "feat(analyzer): implement RustOrchestrator"
```

---

### Task 3: Implement `TypeScriptOrchestrator`

**Files:**
- Modify: `crates/analyzer/src/orchestrator/typescript.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_orchestrator_language_id() {
        let orch = TypeScriptOrchestrator::new();
        assert_eq!(orch.language_id(), "typescript");
    }

    #[test]
    fn typescript_orchestrator_discovers_packages() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap().parent().unwrap().to_path_buf();
        let orch = TypeScriptOrchestrator::new();
        let units = orch.discover(&project_root);
        assert!(units.len() >= 1, "should discover at least 1 TS package");
        assert!(units.iter().all(|u| u.language == "typescript"));
    }

    #[test]
    fn typescript_orchestrator_emits_structural_items() {
        let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap().parent().unwrap().to_path_buf();
        let orch = TypeScriptOrchestrator::new();
        let units = orch.discover(&project_root);
        assert!(!units.is_empty());
        let items = orch.emit_structural_items(&units[0]);
        // Should emit directory and file-level modules
        assert!(!items.is_empty(), "should emit structural module items");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer orchestrator::typescript::tests -v`
Expected: FAIL — stub returns empty

**Step 3: Write implementation**

Replace the stub in `crates/analyzer/src/orchestrator/typescript.rs`. This file absorbs:
- `emit_ts_module_items()` from `lib.rs` (lines 257-318)
- `file_to_module_qn()` from `lib.rs` (lines 321-345)
- `resolve_ts_import()` from `lib.rs` (lines 348-362)

```rust
//! TypeScript/Svelte language orchestrator.
//!
//! Discovers TypeScript packages via `package.json`, emits directory and
//! file-level module items, reparents parsed items, and resolves relative
//! import paths.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use svt_core::model::NodeKind;

use crate::discovery::discover_ts_packages;
use crate::languages::typescript::TypeScriptAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};
use crate::types::{AnalysisItem, TsPackageInfo};

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for TypeScript/Svelte projects.
#[derive(Debug)]
pub struct TypeScriptOrchestrator {
    analyzer: TypeScriptAnalyzer,
}

impl TypeScriptOrchestrator {
    /// Create a new `TypeScriptOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: TypeScriptAnalyzer::new(),
        }
    }
}

impl Default for TypeScriptOrchestrator {
    fn default() -> Self { Self::new() }
}

impl LanguageOrchestrator for TypeScriptOrchestrator {
    fn language_id(&self) -> &str { "typescript" }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        let packages = discover_ts_packages(root).unwrap_or_default();
        packages.into_iter().map(|pkg| LanguageUnit {
            name: pkg.name.clone(),
            language: "typescript".to_string(),
            root: pkg.root.clone(),
            source_root: pkg.source_root.clone(),
            source_files: pkg.source_files.clone(),
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
            source_ref: pkg.root.join("package.json").display().to_string(),
            parent_qualified_name: None,
        }).collect()
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.analyzer.analyze_crate(&unit.name, &file_refs)
    }

    fn emit_structural_items(&self, unit: &LanguageUnit) -> Vec<AnalysisItem> {
        emit_ts_module_items(&unit.source_root, &unit.name, &unit.source_files)
    }

    fn post_process(&self, unit: &LanguageUnit, result: &mut ParseResult) {
        // Reparent items to their file-level module qualified names
        for item in &mut result.items {
            if let Some(file_module_qn) =
                file_to_module_qn(&unit.source_root, &item.source_ref, &unit.name)
            {
                let item_name = item.qualified_name
                    .rsplit("::")
                    .next()
                    .unwrap_or("")
                    .to_string();
                item.qualified_name = format!("{file_module_qn}::{item_name}");
                item.parent_qualified_name = Some(file_module_qn);
            }
        }

        // Resolve relative import paths
        result.relations.retain_mut(|rel| {
            if rel.target_qualified_name.starts_with("./")
                || rel.target_qualified_name.starts_with("../")
            {
                if let Some(resolved) = resolve_ts_import(&rel.target_qualified_name, &unit.name) {
                    rel.target_qualified_name = resolved;
                    true
                } else {
                    false
                }
            } else {
                true
            }
        });
    }
}

// Move emit_ts_module_items, file_to_module_qn, resolve_ts_import here
// (exact code from lib.rs lines 257-362, adapted to return Vec instead of mutating)
```

The `emit_ts_module_items` function changes signature from `fn(..., items: &mut Vec<AnalysisItem>)` to `fn(...) -> Vec<AnalysisItem>`.

The `resolve_ts_import` function changes from taking `&TsPackageInfo` to taking `package_name: &str` (that's all it uses).

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer orchestrator::typescript::tests -v`
Expected: 3 tests PASS

**Step 5: Commit**

```bash
git add crates/analyzer/src/orchestrator/typescript.rs
git commit -m "feat(analyzer): implement TypeScriptOrchestrator with reparenting and import resolution"
```

---

### Task 4: Implement `GoOrchestrator`

**Files:**
- Modify: `crates/analyzer/src/orchestrator/go.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_orchestrator_language_id() {
        let orch = GoOrchestrator::new();
        assert_eq!(orch.language_id(), "go");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer orchestrator::go::tests -v`
Expected: FAIL — stub has no real implementation

**Step 3: Write implementation**

```rust
//! Go language orchestrator.

use std::path::Path;

use svt_core::model::NodeKind;

use crate::discovery::discover_go_packages;
use crate::languages::go::GoAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Go projects.
#[derive(Debug)]
pub struct GoOrchestrator {
    analyzer: GoAnalyzer,
}

impl GoOrchestrator {
    /// Create a new `GoOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self { analyzer: GoAnalyzer::new() }
    }
}

impl Default for GoOrchestrator {
    fn default() -> Self { Self::new() }
}

impl LanguageOrchestrator for GoOrchestrator {
    fn language_id(&self) -> &str { "go" }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_go_packages(root).unwrap_or_default().into_iter().map(|pkg| LanguageUnit {
            name: pkg.name.clone(),
            language: "go".to_string(),
            root: pkg.root.clone(),
            source_root: pkg.root.clone(),
            source_files: pkg.source_files.clone(),
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
            source_ref: pkg.root.join("go.mod").display().to_string(),
            parent_qualified_name: None,
        }).collect()
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.analyzer.analyze_crate(&unit.name, &file_refs)
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer orchestrator::go::tests -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/analyzer/src/orchestrator/go.rs
git commit -m "feat(analyzer): implement GoOrchestrator"
```

---

### Task 5: Implement `PythonOrchestrator`

**Files:**
- Modify: `crates/analyzer/src/orchestrator/python.rs`

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_orchestrator_language_id() {
        let orch = PythonOrchestrator::new();
        assert_eq!(orch.language_id(), "python");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer orchestrator::python::tests -v`
Expected: FAIL — stub has no real implementation

**Step 3: Write implementation**

```rust
//! Python language orchestrator.

use std::path::Path;

use svt_core::model::NodeKind;

use crate::discovery::discover_python_packages;
use crate::languages::python::PythonAnalyzer;
use crate::languages::{LanguageAnalyzer, ParseResult};

use super::{LanguageOrchestrator, LanguageUnit};

/// Orchestrator for Python projects.
#[derive(Debug)]
pub struct PythonOrchestrator {
    analyzer: PythonAnalyzer,
}

impl PythonOrchestrator {
    /// Create a new `PythonOrchestrator`.
    #[must_use]
    pub fn new() -> Self {
        Self { analyzer: PythonAnalyzer::new() }
    }
}

impl Default for PythonOrchestrator {
    fn default() -> Self { Self::new() }
}

impl LanguageOrchestrator for PythonOrchestrator {
    fn language_id(&self) -> &str { "python" }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_python_packages(root).unwrap_or_default().into_iter().map(|pkg| LanguageUnit {
            name: pkg.name.clone(),
            language: "python".to_string(),
            root: pkg.root.clone(),
            source_root: pkg.source_root.clone(),
            source_files: pkg.source_files.clone(),
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
            source_ref: pkg.root.display().to_string(),
            parent_qualified_name: None,
        }).collect()
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.analyzer.analyze_crate(&unit.name, &file_refs)
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer orchestrator::python::tests -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/analyzer/src/orchestrator/python.rs
git commit -m "feat(analyzer): implement PythonOrchestrator"
```

---

### Task 6: Rewrite `analyze_project()` to use `OrchestratorRegistry`

**Files:**
- Modify: `crates/analyzer/src/lib.rs` (major rewrite of `analyze_project()`)

**Step 1: Verify existing tests pass before refactoring**

Run: `cargo test -p svt-analyzer -v`
Expected: All 100+ tests PASS (baseline)

**Step 2: Rewrite `analyze_project()`**

Replace the entire function body (lines 54-240) and remove the now-unused imports and helper functions (`emit_ts_module_items`, `file_to_module_qn`, `resolve_ts_import`, `workspace_qualified_name`). The new `analyze_project()`:

```rust
use crate::orchestrator::{LanguageUnit, OrchestratorRegistry};

pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
    let registry = OrchestratorRegistry::with_defaults();
    let mut all_items = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;
    let mut language_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for orchestrator in registry.orchestrators() {
        // Emit extra items (e.g., Rust workspace root)
        // This is handled by RustOrchestrator::extra_items() specifically.
        // We use a downcast-free approach: if the orchestrator is Rust,
        // we check for workspace items. Actually, better: add extra_items()
        // as a default method on the trait.
        all_items.extend(orchestrator.extra_items(project_root));

        let units = orchestrator.discover(project_root);
        *language_counts
            .entry(orchestrator.language_id().to_string())
            .or_default() += units.len();

        for unit in &units {
            // 1. Emit top-level node (common for all languages)
            all_items.push(AnalysisItem {
                qualified_name: unit.name.clone(),
                kind: unit.top_level_kind,
                sub_kind: unit.top_level_sub_kind.clone(),
                parent_qualified_name: unit.parent_qualified_name.clone(),
                source_ref: unit.source_ref.clone(),
                language: unit.language.clone(),
            });

            // 2. Emit structural items (TypeScript modules, etc.)
            all_items.extend(orchestrator.emit_structural_items(unit));

            // 3. Analyze source files
            let file_refs: Vec<&Path> =
                unit.source_files.iter().map(|p| p.as_path()).collect();
            files_analyzed += file_refs.len();
            let mut parse_result = orchestrator.analyze(unit);

            // 4. Post-process (TypeScript reparenting + import resolution)
            orchestrator.post_process(unit, &mut parse_result);

            // 5. Accumulate results
            all_items.extend(parse_result.items);
            all_relations.extend(parse_result.relations);
            all_warnings.extend(parse_result.warnings);
        }
    }

    // Map to graph nodes and edges
    let (nodes, edges, mapping_warnings) = map_to_graph(&all_items, &all_relations);
    all_warnings.extend(mapping_warnings);

    // Create snapshot and insert
    let version = store.create_snapshot(SnapshotKind::Analysis, commit_ref)?;
    store.add_nodes_batch(version, &nodes)?;
    store.add_edges_batch(version, &edges)?;

    Ok(AnalysisSummary {
        version,
        crates_analyzed: *language_counts.get("rust").unwrap_or(&0),
        ts_packages_analyzed: *language_counts.get("typescript").unwrap_or(&0),
        go_packages_analyzed: *language_counts.get("go").unwrap_or(&0),
        python_packages_analyzed: *language_counts.get("python").unwrap_or(&0),
        files_analyzed,
        nodes_created: nodes.len(),
        edges_created: edges.len(),
        warnings: all_warnings,
    })
}
```

Add `extra_items()` as a default method on `LanguageOrchestrator`:

```rust
/// Emit project-level items that don't belong to any single unit (e.g., workspace root).
///
/// Default: no extra items.
fn extra_items(&self, _root: &Path) -> Vec<AnalysisItem> {
    vec![]
}
```

And override it in `RustOrchestrator`.

Remove from `lib.rs`:
- `use crate::discovery::{discover_go_packages, discover_project, discover_python_packages, discover_ts_packages};`
- `use crate::languages::go::GoAnalyzer;`
- `use crate::languages::python::PythonAnalyzer;`
- `use crate::languages::rust::RustAnalyzer;`
- `use crate::languages::typescript::TypeScriptAnalyzer;`
- `use crate::languages::LanguageAnalyzer;`
- `use crate::types::TsPackageInfo;`
- `use std::collections::HashSet;`
- `fn workspace_qualified_name()`
- `fn emit_ts_module_items()`
- `fn file_to_module_qn()`
- `fn resolve_ts_import()`
- The `workspace_qualified_name_*` tests (move to orchestrator::rust::tests)

**Step 3: Run all tests to verify refactor is behavior-preserving**

Run: `cargo test -p svt-analyzer -v`
Expected: All tests PASS (same count as before minus moved tests, plus new orchestrator tests)

Also run: `cargo test` (full workspace)
Expected: All 340+ tests PASS

**Step 4: Commit**

```bash
git add crates/analyzer/src/lib.rs crates/analyzer/src/orchestrator.rs crates/analyzer/src/orchestrator/
git commit -m "refactor(analyzer): rewrite analyze_project to use OrchestratorRegistry dispatch"
```

---

### Task 7: Aggregate method call warnings

**Files:**
- Modify: `crates/analyzer/src/languages/rust.rs` (lines 464-525, `visit_call_expressions`)
- Modify: `crates/analyzer/src/languages/rust.rs` (tests around line 1006)

**Step 1: Update the test to expect aggregated warning**

In `crates/analyzer/src/languages/rust.rs`, update the `method_call_generates_warning` test:

```rust
#[test]
fn method_call_generates_warning() {
    let result = parse_source(
        "my_crate",
        r#"
        fn main() {
            let x = String::new();
            x.push_str("hello");
            x.push_str("world");
        }
    "#,
    );
    // Method calls produce an aggregated warning per parse, not per call.
    let method_warnings: Vec<_> = result
        .warnings
        .iter()
        .filter(|w| w.message.contains("method call"))
        .collect();
    assert_eq!(method_warnings.len(), 1, "should produce exactly 1 aggregated warning");
    assert!(method_warnings[0].message.contains("2"), "should mention the count");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer method_call_generates_warning -v`
Expected: FAIL — currently produces 2 separate warnings

**Step 3: Modify `visit_call_expressions`**

Change the function signature to track a count instead of emitting warnings immediately:

```rust
fn visit_call_expressions(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    file_path: &Path,
    module_context: &[String],
    relations: &mut Vec<AnalysisRelation>,
    unresolved_method_count: &mut usize, // Changed from warnings
) {
    // ... same recursion logic ...
    // Instead of warnings.push(...) for field_expression:
    "field_expression" => {
        *unresolved_method_count += 1;
    }
    // ... recurse with unresolved_method_count ...
}
```

In the caller (`parse_rust_file`), after calling `visit_call_expressions`, emit one aggregated warning if count > 0:

```rust
let mut unresolved_method_count = 0;
visit_call_expressions(root, source, file, &module_context, &mut result.relations, &mut unresolved_method_count);
if unresolved_method_count > 0 {
    result.warnings.push(AnalysisWarning {
        source_ref: file.display().to_string(),
        message: format!(
            "{unresolved_method_count} method call(s) could not be resolved (syntax-only analysis)"
        ),
    });
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer -v`
Expected: All tests PASS. The `method_call_handled_gracefully` test still passes because it checks relations, not warnings.

Also run: `cargo test` (full workspace)
Expected: All tests PASS. Warning count in dogfood tests should be significantly lower.

**Step 5: Commit**

```bash
git add crates/analyzer/src/languages/rust.rs
git commit -m "refactor(analyzer): aggregate method call warnings into per-file summary"
```

---

### Task 8: Update PROGRESS.md and final verification

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

Run: `cargo clippy --all-targets`
Expected: No warnings

Run: `cargo fmt --check`
Expected: No formatting issues

**Step 2: Update PROGRESS.md**

- Mark "Analyzer Wiring" gap as RESOLVED
- Update "Analysis Depth" to note reduced warning noise
- Update test counts
- Add the cleanup to the completed milestones table (or as a sub-entry under M15)
- Add this plan document to the plan documents table

**Step 3: Commit**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark known gaps cleanup complete, update progress"
```
