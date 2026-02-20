# M18: Plugin Analyzer Support — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow external plugins to contribute language analyzers via a descriptor + parser API, refactoring built-in Go/Python/TypeScript to use the same mechanism.

**Architecture:** Move analysis pipeline types (`AnalysisItem`, `AnalysisRelation`, `AnalysisWarning`, `ParseResult`) from svt-analyzer to svt-core. Add `LanguageDescriptor` struct and `LanguageParser` trait to svt-core. Implement a generic `DescriptorOrchestrator` in svt-analyzer that pairs any descriptor+parser. Refactor Go/Python/TypeScript to use descriptors. Extend `SvtPlugin` with `language_parsers()`. Wire into CLI.

**Tech Stack:** Rust 2021, svt-core (WASM-compatible), svt-analyzer (tree-sitter), svt-cli (libloading), walkdir, toml (new dep for manifest parsing)

**Design doc:** `docs/plan/2026-02-20-plugin-analyzer-support-design.md`

---

### Task 1: Move Analysis Types to svt-core

**Files:**
- Create: `crates/core/src/analysis.rs`
- Modify: `crates/core/src/lib.rs:1-44` (add module declaration)

**Step 1: Write the failing test**

Add to the bottom of the new `crates/core/src/analysis.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, NodeKind};

    #[test]
    fn parse_result_collects_items_relations_warnings() {
        let result = ParseResult {
            items: vec![AnalysisItem {
                qualified_name: "my_crate::Foo".to_string(),
                kind: NodeKind::Unit,
                sub_kind: "struct".to_string(),
                parent_qualified_name: Some("my_crate".to_string()),
                source_ref: "src/lib.rs:10".to_string(),
                language: "rust".to_string(),
            }],
            relations: vec![AnalysisRelation {
                source_qualified_name: "my_crate::Foo".to_string(),
                target_qualified_name: "my_crate::Bar".to_string(),
                kind: EdgeKind::DependsOn,
            }],
            warnings: vec![AnalysisWarning {
                source_ref: "src/lib.rs:20".to_string(),
                message: "unresolved import".to_string(),
            }],
        };
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qualified_name, "my_crate::Foo");
        assert_eq!(result.relations.len(), 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn parse_result_default_is_empty() {
        let result = ParseResult::default();
        assert!(result.items.is_empty());
        assert!(result.relations.is_empty());
        assert!(result.warnings.is_empty());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core analysis::tests --no-run 2>&1`
Expected: FAIL — module `analysis` doesn't exist yet

**Step 3: Write the implementation**

Create `crates/core/src/analysis.rs`:

```rust
//! Analysis pipeline types shared between svt-core and svt-analyzer.
//!
//! These types are the interchange format between language parsers (which
//! produce them) and the mapping/insertion pipeline (which consumes them).
//! They live in svt-core so that plugin authors can implement
//! [`LanguageParser`] without depending on svt-analyzer.

use std::path::{Path, PathBuf};

use crate::model::{EdgeKind, NodeKind};

/// A code element extracted by static analysis.
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

/// A relationship between code elements.
#[derive(Debug, Clone)]
pub struct AnalysisRelation {
    /// Qualified name of the source element.
    pub source_qualified_name: String,
    /// Qualified name of the target element.
    pub target_qualified_name: String,
    /// Relationship type.
    pub kind: EdgeKind,
}

/// A non-fatal warning from analysis.
#[derive(Debug, Clone)]
pub struct AnalysisWarning {
    /// Source file and line where the issue was found.
    pub source_ref: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Result of parsing source files for a single language unit.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    /// Extracted code elements.
    pub items: Vec<AnalysisItem>,
    /// Extracted relationships between elements.
    pub relations: Vec<AnalysisRelation>,
    /// Warnings from parsing (non-fatal).
    pub warnings: Vec<AnalysisWarning>,
}
```

Add to `crates/core/src/lib.rs` (after the `pub mod model;` line, before any `#[cfg(feature = "store")]` block):

```rust
/// Analysis pipeline types: items, relations, warnings, parse results.
pub mod analysis;
```

Note: This module is NOT gated behind `#[cfg(feature = "store")]` because it only depends on `model` types and must be available to all consumers including WASM.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core analysis::tests -v`
Expected: 2 tests PASS

**Step 5: Commit**

```bash
git add crates/core/src/analysis.rs crates/core/src/lib.rs
git commit -m "feat(core): add analysis pipeline types (AnalysisItem, ParseResult, etc.)"
```

---

### Task 2: Add LanguageDescriptor and LanguageParser to svt-core

**Files:**
- Modify: `crates/core/src/analysis.rs` (append new types and trait)

**Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` block in `crates/core/src/analysis.rs`:

```rust
    #[test]
    fn language_descriptor_fields_accessible() {
        let desc = LanguageDescriptor {
            language_id: "java".to_string(),
            manifest_files: vec!["pom.xml".to_string()],
            source_extensions: vec![".java".to_string()],
            skip_directories: vec!["target".to_string(), ".git".to_string()],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        };
        assert_eq!(desc.language_id, "java");
        assert_eq!(desc.manifest_files, vec!["pom.xml"]);
        assert_eq!(desc.source_extensions, vec![".java"]);
    }

    /// A mock parser for testing the LanguageParser trait.
    struct MockParser;

    impl LanguageParser for MockParser {
        fn parse(&self, unit_name: &str, _files: &[&Path]) -> ParseResult {
            ParseResult {
                items: vec![AnalysisItem {
                    qualified_name: format!("{unit_name}::Main"),
                    kind: NodeKind::Unit,
                    sub_kind: "class".to_string(),
                    parent_qualified_name: Some(unit_name.to_string()),
                    source_ref: "src/Main.java:1".to_string(),
                    language: "java".to_string(),
                }],
                relations: vec![],
                warnings: vec![],
            }
        }
    }

    #[test]
    fn mock_parser_returns_items() {
        let parser = MockParser;
        let result = parser.parse("my-app", &[]);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qualified_name, "my-app::Main");
    }

    #[test]
    fn language_parser_default_hooks_are_noops() {
        let parser = MockParser;
        let root = Path::new("/tmp");
        assert!(parser.emit_structural_items(root, "pkg", &[]).is_empty());
        let mut result = ParseResult::default();
        parser.post_process(root, "pkg", &mut result);
        assert!(result.items.is_empty());
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core analysis::tests --no-run 2>&1`
Expected: FAIL — `LanguageDescriptor` and `LanguageParser` not defined

**Step 3: Write the implementation**

Append to `crates/core/src/analysis.rs` (before the `#[cfg(test)]` block):

```rust
/// Describes how to discover project units for a language.
///
/// The host uses this to walk the project directory, find manifest files,
/// derive package names, and collect source files — without the plugin
/// needing to implement any discovery logic.
#[derive(Debug, Clone)]
pub struct LanguageDescriptor {
    /// Unique language identifier (e.g., "rust", "go", "java").
    pub language_id: String,
    /// Manifest filenames that indicate a project unit
    /// (e.g., `["go.mod"]`, `["package.json"]`, `["pyproject.toml", "setup.py"]`).
    pub manifest_files: Vec<String>,
    /// Source file extensions to collect (e.g., `[".go"]`, `[".py"]`).
    pub source_extensions: Vec<String>,
    /// Directories to skip during walking (e.g., `["vendor", "node_modules"]`).
    pub skip_directories: Vec<String>,
    /// The [`NodeKind`] for top-level units (typically `NodeKind::Service`).
    pub top_level_kind: NodeKind,
    /// Sub-kind label for top-level units (e.g., "module", "package", "crate").
    pub top_level_sub_kind: String,
}

/// Trait for parsing source files into analysis items and relations.
///
/// Plugin authors implement this to add support for a new language.
/// The host handles discovery, file walking, and orchestration —
/// the parser only needs to extract structure from source code.
pub trait LanguageParser: Send + Sync {
    /// Parse source files for a single project unit.
    ///
    /// `unit_name` is the package/module name derived from the manifest.
    /// `files` are all source files collected by the host based on the descriptor.
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult;

    /// Emit additional structural items beyond what parsing finds.
    ///
    /// For example, TypeScript emits directory-based module nodes.
    /// Default: no additional items.
    fn emit_structural_items(
        &self,
        _source_root: &Path,
        _unit_name: &str,
        _source_files: &[PathBuf],
    ) -> Vec<AnalysisItem> {
        vec![]
    }

    /// Post-process parse results (e.g., reparenting items, resolving imports).
    ///
    /// Default: no post-processing.
    fn post_process(
        &self,
        _source_root: &Path,
        _unit_name: &str,
        _result: &mut ParseResult,
    ) {
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core analysis::tests -v`
Expected: 5 tests PASS (2 from Task 1 + 3 new)

**Step 5: Commit**

```bash
git add crates/core/src/analysis.rs
git commit -m "feat(core): add LanguageDescriptor and LanguageParser trait for plugin analyzers"
```

---

### Task 3: Extend SvtPlugin with language_parsers()

**Files:**
- Modify: `crates/core/src/plugin.rs:53-77` (add method to trait)
- Modify: `crates/core/src/plugin.rs` (tests section — update mock plugins)

**Step 1: Write the failing test**

Add a new test in `crates/core/src/plugin.rs` `mod tests`:

```rust
    #[test]
    fn mock_plugin_default_language_parsers_is_empty() {
        let plugin = MockPlugin;
        assert!(
            plugin.language_parsers().is_empty(),
            "default language_parsers() should return an empty vec"
        );
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core plugin::tests::mock_plugin_default_language_parsers_is_empty --no-run 2>&1`
Expected: FAIL — `language_parsers` method doesn't exist on `SvtPlugin`

**Step 3: Write the implementation**

Add the import at the top of `crates/core/src/plugin.rs`:

```rust
use crate::analysis::{LanguageDescriptor, LanguageParser};
```

Add a new method to the `SvtPlugin` trait (after `export_formats`):

```rust
    /// Language parsers contributed by this plugin.
    ///
    /// Each entry pairs a [`LanguageDescriptor`] (discovery configuration) with
    /// a [`LanguageParser`] (source code parser). The host uses the descriptor
    /// to find project units and the parser to extract structure.
    ///
    /// Returns an empty vec by default.
    fn language_parsers(&self) -> Vec<(LanguageDescriptor, Box<dyn LanguageParser>)> {
        Vec::new()
    }
```

Update the `SvtPlugin` trait doc comment to mention language parsers:

```rust
/// A plugin provides metadata (name, version, API version) and may contribute
/// additional [`ConstraintEvaluator`]s, [`ExportFormat`]s, and
/// [`LanguageParser`]s to the host application.
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core plugin::tests -v`
Expected: 8 tests PASS (7 existing + 1 new)

**Step 5: Commit**

```bash
git add crates/core/src/plugin.rs
git commit -m "feat(core): extend SvtPlugin trait with language_parsers() method"
```

---

### Task 4: Re-export Core Analysis Types from svt-analyzer

**Files:**
- Modify: `crates/analyzer/src/types.rs:1-133` (replace local types with re-exports)
- Modify: `crates/analyzer/src/languages/mod.rs:1-22` (re-export ParseResult from core)

**Step 1: Run all analyzer tests as baseline**

Run: `cargo test -p svt-analyzer 2>&1 | tail -5`
Expected: 114 tests PASS (confirm baseline)

**Step 2: Replace local types in analyzer/types.rs with re-exports**

In `crates/analyzer/src/types.rs`, replace the `AnalysisItem`, `AnalysisRelation`, `AnalysisWarning` struct definitions with re-exports from svt-core. Keep the language-specific structs (`CrateInfo`, `ProjectLayout`, `TsPackageInfo`, `GoPackageInfo`, `GoPackage`, `PythonPackageInfo`, `AnalysisSummary`) unchanged.

Replace lines 98-133 (the four analysis types) with:

```rust
// Re-export analysis pipeline types from svt-core.
// These were moved to core so plugin authors can use them.
pub use svt_core::analysis::{AnalysisItem, AnalysisRelation, AnalysisWarning};
```

**Step 3: Replace local ParseResult in languages/mod.rs with re-export**

In `crates/analyzer/src/languages/mod.rs`, replace the `ParseResult` struct definition (lines 13-22) and its imports (line 11) with:

```rust
use crate::types::AnalysisItem;

// Re-export ParseResult from svt-core for use by language analyzers.
pub use svt_core::analysis::ParseResult;
```

Remove the `AnalysisRelation` and `AnalysisWarning` imports from line 11 (they're no longer used directly in this file — they come through `ParseResult`).

**Step 4: Run all tests to verify the re-export is transparent**

Run: `cargo test -p svt-analyzer -v 2>&1 | tail -5`
Expected: 114 tests PASS — no behavioural change

Run: `cargo test --workspace 2>&1 | grep "^test result:"` to check everything still compiles.
Expected: All test suites pass

**Step 5: Commit**

```bash
git add crates/analyzer/src/types.rs crates/analyzer/src/languages/mod.rs
git commit -m "refactor(analyzer): re-export analysis types from svt-core"
```

---

### Task 5: Implement DescriptorOrchestrator

**Files:**
- Create: `crates/analyzer/src/orchestrator/descriptor.rs`
- Modify: `crates/analyzer/src/orchestrator/mod.rs:1-11` (add `pub mod descriptor;`)

**Step 1: Write the failing tests**

Create `crates/analyzer/src/orchestrator/descriptor.rs` with:

```rust
//! Generic orchestrator driven by a [`LanguageDescriptor`] and [`LanguageParser`].

use std::path::{Path, PathBuf};

use svt_core::analysis::{
    AnalysisItem, LanguageDescriptor, LanguageParser, ParseResult,
};
use svt_core::model::NodeKind;

use super::{LanguageOrchestrator, LanguageUnit};

/// A generic orchestrator that wraps a [`LanguageDescriptor`] and [`LanguageParser`].
///
/// Discovery is data-driven using the descriptor. Parsing delegates to the parser.
pub struct DescriptorOrchestrator {
    descriptor: LanguageDescriptor,
    parser: Box<dyn LanguageParser>,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubParser;

    impl LanguageParser for StubParser {
        fn parse(&self, unit_name: &str, _files: &[&Path]) -> ParseResult {
            ParseResult {
                items: vec![AnalysisItem {
                    qualified_name: format!("{unit_name}::Stub"),
                    kind: NodeKind::Unit,
                    sub_kind: "class".to_string(),
                    parent_qualified_name: Some(unit_name.to_string()),
                    source_ref: "stub.java:1".to_string(),
                    language: "java".to_string(),
                }],
                relations: vec![],
                warnings: vec![],
            }
        }
    }

    fn java_descriptor() -> LanguageDescriptor {
        LanguageDescriptor {
            language_id: "java".to_string(),
            manifest_files: vec!["pom.xml".to_string()],
            source_extensions: vec![".java".to_string()],
            skip_directories: vec!["target".to_string(), ".git".to_string()],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        }
    }

    #[test]
    fn descriptor_orchestrator_language_id() {
        let orch = DescriptorOrchestrator::new(java_descriptor(), Box::new(StubParser));
        assert_eq!(orch.language_id(), "java");
    }

    #[test]
    fn discover_finds_manifest_and_collects_sources() {
        let dir = tempfile::tempdir().unwrap();
        // Create pom.xml with a name
        std::fs::write(
            dir.path().join("pom.xml"),
            "<project><name>my-app</name></project>",
        )
        .unwrap();
        // Create source files
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("Main.java"), "class Main {}").unwrap();
        std::fs::write(src.join("readme.txt"), "not java").unwrap();

        let orch = DescriptorOrchestrator::new(java_descriptor(), Box::new(StubParser));
        let units = orch.discover(dir.path());

        assert_eq!(units.len(), 1, "should discover one unit from pom.xml");
        assert_eq!(units[0].language, "java");
        assert_eq!(
            units[0].source_files.len(),
            1,
            "should only collect .java files"
        );
        assert_eq!(units[0].top_level_sub_kind, "module");
    }

    #[test]
    fn discover_skips_configured_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pom.xml"), "{}").unwrap();
        let target_dir = dir.path().join("target");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join("Generated.java"), "class Gen {}").unwrap();
        // Source in root
        std::fs::write(dir.path().join("App.java"), "class App {}").unwrap();

        let orch = DescriptorOrchestrator::new(java_descriptor(), Box::new(StubParser));
        let units = orch.discover(dir.path());

        assert_eq!(units.len(), 1);
        // Should NOT include target/Generated.java
        for f in &units[0].source_files {
            assert!(
                !f.to_string_lossy().contains("target"),
                "should skip target/ directory, found: {}",
                f.display()
            );
        }
    }

    #[test]
    fn discover_empty_directory_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let orch = DescriptorOrchestrator::new(java_descriptor(), Box::new(StubParser));
        let units = orch.discover(dir.path());
        assert!(units.is_empty(), "no manifest → no units");
    }

    #[test]
    fn discover_extracts_name_from_json_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let desc = LanguageDescriptor {
            language_id: "test".to_string(),
            manifest_files: vec!["package.json".to_string()],
            source_extensions: vec![".ts".to_string()],
            skip_directories: vec![],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
        };
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name": "my-pkg"}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("index.ts"), "export {}").unwrap();

        let orch = DescriptorOrchestrator::new(desc, Box::new(StubParser));
        let units = orch.discover(dir.path());

        assert_eq!(units.len(), 1);
        assert_eq!(units[0].name, "my-pkg");
    }

    #[test]
    fn discover_extracts_name_from_toml_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let desc = LanguageDescriptor {
            language_id: "test".to_string(),
            manifest_files: vec!["pyproject.toml".to_string()],
            source_extensions: vec![".py".to_string()],
            skip_directories: vec![],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "package".to_string(),
        };
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nname = \"my-tool\"\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("main.py"), "pass").unwrap();

        let orch = DescriptorOrchestrator::new(desc, Box::new(StubParser));
        let units = orch.discover(dir.path());

        assert_eq!(units.len(), 1);
        assert_eq!(units[0].name, "my-tool");
    }

    #[test]
    fn discover_extracts_name_from_go_mod() {
        let dir = tempfile::tempdir().unwrap();
        let desc = LanguageDescriptor {
            language_id: "test".to_string(),
            manifest_files: vec!["go.mod".to_string()],
            source_extensions: vec![".go".to_string()],
            skip_directories: vec![],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        };
        std::fs::write(
            dir.path().join("go.mod"),
            "module github.com/user/myrepo\n\ngo 1.21\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("main.go"), "package main").unwrap();

        let orch = DescriptorOrchestrator::new(desc, Box::new(StubParser));
        let units = orch.discover(dir.path());

        assert_eq!(units.len(), 1);
        assert_eq!(units[0].name, "myrepo");
    }

    #[test]
    fn discover_falls_back_to_directory_name() {
        let dir = tempfile::tempdir().unwrap();
        // pom.xml with no parseable name
        std::fs::write(dir.path().join("pom.xml"), "<project/>").unwrap();
        std::fs::write(dir.path().join("App.java"), "class App {}").unwrap();

        let orch = DescriptorOrchestrator::new(java_descriptor(), Box::new(StubParser));
        let units = orch.discover(dir.path());

        assert_eq!(units.len(), 1);
        // Falls back to directory name
        assert!(
            !units[0].name.is_empty(),
            "should fall back to directory name"
        );
    }

    #[test]
    fn analyze_delegates_to_parser() {
        let orch = DescriptorOrchestrator::new(java_descriptor(), Box::new(StubParser));
        let unit = LanguageUnit {
            name: "test-pkg".to_string(),
            language: "java".to_string(),
            root: PathBuf::from("/tmp"),
            source_root: PathBuf::from("/tmp"),
            source_files: vec![],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
            source_ref: "/tmp/pom.xml".to_string(),
            parent_qualified_name: None,
        };
        let result = orch.analyze(&unit);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qualified_name, "test-pkg::Stub");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer orchestrator::descriptor::tests --no-run 2>&1`
Expected: FAIL — module `descriptor` doesn't exist, `DescriptorOrchestrator::new` not defined, `discover_by_descriptor` not defined

**Step 3: Write the implementation**

Add `pub mod descriptor;` to `crates/analyzer/src/orchestrator/mod.rs` (after `pub mod typescript;`).

Add `toml` to `crates/analyzer/Cargo.toml` dependencies:

```toml
toml = "0.8"
```

Complete the implementation in `crates/analyzer/src/orchestrator/descriptor.rs`:

```rust
impl DescriptorOrchestrator {
    /// Create a new descriptor-driven orchestrator.
    pub fn new(descriptor: LanguageDescriptor, parser: Box<dyn LanguageParser>) -> Self {
        Self { descriptor, parser }
    }
}

impl LanguageOrchestrator for DescriptorOrchestrator {
    fn language_id(&self) -> &str {
        &self.descriptor.language_id
    }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_by_descriptor(root, &self.descriptor)
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.parser.parse(&unit.name, &file_refs)
    }

    fn emit_structural_items(&self, unit: &LanguageUnit) -> Vec<AnalysisItem> {
        self.parser
            .emit_structural_items(&unit.source_root, &unit.name, &unit.source_files)
    }

    fn post_process(&self, unit: &LanguageUnit, result: &mut ParseResult) {
        self.parser
            .post_process(&unit.source_root, &unit.name, result)
    }
}

/// Discover project units by walking the directory tree looking for manifest files.
fn discover_by_descriptor(root: &Path, descriptor: &LanguageDescriptor) -> Vec<LanguageUnit> {
    let mut units = Vec::new();

    let walker = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            if entry.file_type().is_dir() {
                let name = entry.file_name().to_string_lossy();
                !descriptor.skip_directories.iter().any(|s| s == name.as_ref())
            } else {
                true
            }
        });

    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy();
        if !descriptor.manifest_files.iter().any(|m| m == file_name.as_ref()) {
            continue;
        }

        let manifest_path = entry.path();
        let unit_root = manifest_path.parent().unwrap_or(root);

        // Extract name from manifest
        let name = extract_name_from_manifest(manifest_path)
            .unwrap_or_else(|| {
                unit_root
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            });

        // Collect source files
        let source_files = collect_source_files(unit_root, descriptor);

        units.push(LanguageUnit {
            name,
            language: descriptor.language_id.clone(),
            root: unit_root.to_path_buf(),
            source_root: unit_root.to_path_buf(),
            source_files,
            top_level_kind: descriptor.top_level_kind.clone(),
            top_level_sub_kind: descriptor.top_level_sub_kind.clone(),
            source_ref: manifest_path.display().to_string(),
            parent_qualified_name: None,
        });
    }

    units
}

/// Try to extract a package/project name from a manifest file.
fn extract_name_from_manifest(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let file_name = path.file_name()?.to_string_lossy();

    // go.mod: "module github.com/user/repo"
    if file_name == "go.mod" {
        return extract_go_module_name(&content);
    }

    // Try JSON: {"name": "..."}
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
            return Some(name.to_string());
        }
    }

    // Try TOML: [project].name or [package].name
    if let Ok(toml) = content.parse::<toml::Table>() {
        if let Some(name) = toml
            .get("project")
            .or_else(|| toml.get("package"))
            .and_then(|t| t.get("name"))
            .and_then(|v| v.as_str())
        {
            return Some(name.to_string());
        }
    }

    None
}

/// Extract module name from go.mod content.
fn extract_go_module_name(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(module_path) = trimmed.strip_prefix("module ") {
            let module_path = module_path.trim();
            return Some(
                module_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(module_path)
                    .to_string(),
            );
        }
    }
    None
}

/// Collect source files matching the descriptor's extensions, skipping configured dirs.
fn collect_source_files(root: &Path, descriptor: &LanguageDescriptor) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let walker = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            if entry.file_type().is_dir() {
                let name = entry.file_name().to_string_lossy();
                !descriptor.skip_directories.iter().any(|s| s == name.as_ref())
            } else {
                true
            }
        });

    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if let Some(ext_with_dot) = path.extension().map(|e| format!(".{}", e.to_string_lossy())) {
            if descriptor.source_extensions.iter().any(|se| se == &ext_with_dot) {
                files.push(path.to_path_buf());
            }
        }
    }

    files
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-analyzer orchestrator::descriptor::tests -v`
Expected: 9 tests PASS

Run: `cargo test -p svt-analyzer -v 2>&1 | tail -5`
Expected: All 114+ tests still pass

**Step 5: Commit**

```bash
git add crates/analyzer/src/orchestrator/descriptor.rs crates/analyzer/src/orchestrator/mod.rs crates/analyzer/Cargo.toml
git commit -m "feat(analyzer): add DescriptorOrchestrator with manifest-driven discovery"
```

---

### Task 6: Refactor Go Orchestrator to Descriptor + Parser

**Files:**
- Modify: `crates/analyzer/src/languages/go.rs` (implement `LanguageParser`)
- Modify: `crates/analyzer/src/orchestrator/go.rs` (replace with descriptor construction)
- Modify: `crates/analyzer/src/orchestrator/mod.rs:102-108` (update `with_defaults`)

**Step 1: Run Go-specific tests as baseline**

Run: `cargo test -p svt-analyzer go -v 2>&1`
Expected: Note test names and pass count (7 analyzer + 1 orchestrator + discovery tests)

**Step 2: Implement LanguageParser on GoAnalyzer**

In `crates/analyzer/src/languages/go.rs`, add `LanguageParser` impl:

```rust
use svt_core::analysis::{LanguageDescriptor, LanguageParser};
use svt_core::model::NodeKind;

impl LanguageParser for GoAnalyzer {
    fn parse(&self, unit_name: &str, files: &[&Path]) -> crate::languages::ParseResult {
        self.analyze_crate(unit_name, files)
    }
}

impl GoAnalyzer {
    /// Language descriptor for Go projects.
    pub fn descriptor() -> LanguageDescriptor {
        LanguageDescriptor {
            language_id: "go".to_string(),
            manifest_files: vec!["go.mod".to_string()],
            source_extensions: vec![".go".to_string()],
            skip_directories: vec![
                "vendor".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "dist".to_string(),
            ],
            top_level_kind: NodeKind::Service,
            top_level_sub_kind: "module".to_string(),
        }
    }

    /// Create a boxed LanguageParser for use with DescriptorOrchestrator.
    pub fn parser() -> Box<dyn LanguageParser> {
        Box::new(GoAnalyzer::new())
    }
}
```

**Step 3: Update orchestrator/go.rs to use DescriptorOrchestrator**

Replace `crates/analyzer/src/orchestrator/go.rs` with:

```rust
//! Go orchestrator — delegates to DescriptorOrchestrator.

use super::descriptor::DescriptorOrchestrator;
use crate::languages::go::GoAnalyzer;

/// Create a Go orchestrator using the descriptor + parser pattern.
pub fn orchestrator() -> DescriptorOrchestrator {
    DescriptorOrchestrator::new(GoAnalyzer::descriptor(), GoAnalyzer::parser())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::LanguageOrchestrator;

    #[test]
    fn go_orchestrator_language_id() {
        let orch = orchestrator();
        assert_eq!(orch.language_id(), "go");
    }
}
```

**Step 4: Update with_defaults() in orchestrator/mod.rs**

Change the Go line from:

```rust
registry.register(Box::new(go::GoOrchestrator::new()));
```

to:

```rust
registry.register(Box::new(go::orchestrator()));
```

**Step 5: Run tests to verify regression-free**

Run: `cargo test -p svt-analyzer -v 2>&1`
Expected: All tests PASS (orchestrator test count may change slightly but all Go behaviour preserved)

Run: `cargo test --workspace 2>&1 | grep "^test result:"`
Expected: All suites pass

**Step 6: Commit**

```bash
git add crates/analyzer/src/languages/go.rs crates/analyzer/src/orchestrator/go.rs crates/analyzer/src/orchestrator/mod.rs
git commit -m "refactor(analyzer): Go orchestrator uses DescriptorOrchestrator"
```

---

### Task 7: Refactor Python Orchestrator to Descriptor + Parser

**Files:**
- Modify: `crates/analyzer/src/languages/python.rs`
- Modify: `crates/analyzer/src/orchestrator/python.rs`
- Modify: `crates/analyzer/src/orchestrator/mod.rs`

Same pattern as Task 6. Key differences:

- `manifest_files: vec!["pyproject.toml".to_string(), "setup.py".to_string()]`
- `source_extensions: vec![".py".to_string()]`
- `skip_directories: vec!["venv".to_string(), ".venv".to_string(), "__pycache__".to_string(), "node_modules".to_string(), "target".to_string(), ".git".to_string()]`
- `top_level_sub_kind: "package".to_string()`

Follow the exact same steps as Task 6 but for Python.

**Commit:**

```bash
git add crates/analyzer/src/languages/python.rs crates/analyzer/src/orchestrator/python.rs crates/analyzer/src/orchestrator/mod.rs
git commit -m "refactor(analyzer): Python orchestrator uses DescriptorOrchestrator"
```

---

### Task 8: Refactor TypeScript Orchestrator to Descriptor + Parser

**Files:**
- Modify: `crates/analyzer/src/languages/typescript.rs`
- Modify: `crates/analyzer/src/orchestrator/typescript.rs`
- Modify: `crates/analyzer/src/orchestrator/mod.rs`

Same pattern as Task 6 but TypeScript uses the escape hatches:

- `manifest_files: vec!["package.json".to_string()]`
- `source_extensions: vec![".ts".to_string(), ".tsx".to_string(), ".svelte".to_string()]`
- `skip_directories: vec!["node_modules".to_string(), "dist".to_string(), "build".to_string(), ".svt".to_string(), "target".to_string(), ".git".to_string()]`
- `top_level_sub_kind: "package".to_string()`

TypeScript's `LanguageParser` impl overrides `emit_structural_items` and `post_process` to handle directory-based module reparenting and import resolution. These methods already exist on the `TypeScriptOrchestrator` — move their bodies into the `LanguageParser` impl.

The existing `emit_ts_module_items()` and `file_to_module_qn()` helper functions stay in `crates/analyzer/src/orchestrator/typescript.rs` — they're called by the `LanguageParser` impl.

**Important:** The TypeScript orchestrator tests (`typescript_orchestrator_discovers_packages` and `typescript_orchestrator_emits_structural_items`) must still pass. These are the most complex tests and validate the escape hatches work.

**Commit:**

```bash
git add crates/analyzer/src/languages/typescript.rs crates/analyzer/src/orchestrator/typescript.rs crates/analyzer/src/orchestrator/mod.rs
git commit -m "refactor(analyzer): TypeScript orchestrator uses DescriptorOrchestrator"
```

---

### Task 9: Refactor RustAnalyzer to Implement LanguageParser

**Files:**
- Modify: `crates/analyzer/src/languages/rust.rs` (add `LanguageParser` impl)
- Modify: `crates/analyzer/src/orchestrator/rust.rs` (use parser for analyze())

Rust keeps its custom orchestrator (cargo metadata discovery, workspace-aware naming, extra_items). But `RustAnalyzer` also implements `LanguageParser` so the parsing interface is uniform.

**Step 1: Add LanguageParser impl to RustAnalyzer**

In `crates/analyzer/src/languages/rust.rs`:

```rust
use svt_core::analysis::LanguageParser;

impl LanguageParser for RustAnalyzer {
    fn parse(&self, unit_name: &str, files: &[&Path]) -> crate::languages::ParseResult {
        self.analyze_crate(unit_name, files)
    }
}
```

**Step 2: Update RustOrchestrator::analyze() to delegate through LanguageParser**

This is optional since the behaviour is identical, but ensures uniformity. The current `analyze()` body is:

```rust
fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
    let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
    self.analyzer.analyze_crate(&unit.name, &file_refs)
}
```

No change needed — `analyze_crate` is already what `LanguageParser::parse` delegates to.

**Step 3: Run tests**

Run: `cargo test -p svt-analyzer rust -v 2>&1`
Expected: All Rust analyzer and orchestrator tests pass

**Step 4: Commit**

```bash
git add crates/analyzer/src/languages/rust.rs
git commit -m "refactor(analyzer): RustAnalyzer implements LanguageParser trait"
```

---

### Task 10: Wire Plugin Language Parsers into CLI

**Files:**
- Modify: `crates/analyzer/src/lib.rs:48-68` (accept optional extra orchestrators)
- Modify: `crates/cli/src/main.rs:347-387` (pass plugin parsers to analyze)
- Modify: `crates/cli/src/plugin.rs` (add `register_language_parsers` method)

**Step 1: Modify `analyze_project()` to accept an `OrchestratorRegistry`**

Change the signature from:

```rust
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
```

to:

```rust
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
```

Actually, the cleanest approach: add a new `analyze_project_with_registry()` function that takes an `OrchestratorRegistry`, and have `analyze_project()` call it with defaults:

```rust
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
) -> Result<AnalysisSummary, AnalyzerError> {
    let registry = OrchestratorRegistry::with_defaults();
    analyze_project_with_registry(store, project_root, commit_ref, registry)
}

pub fn analyze_project_with_registry(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
    registry: OrchestratorRegistry,
) -> Result<AnalysisSummary, AnalyzerError> {
    // ... existing body, but using the passed-in registry ...
}
```

**Step 2: Add `register_language_parsers` to PluginLoader**

In `crates/cli/src/plugin.rs`:

```rust
use svt_analyzer::orchestrator::descriptor::DescriptorOrchestrator;
use svt_analyzer::orchestrator::OrchestratorRegistry;

/// Register all loaded plugins' language parsers into the given registry.
pub fn register_language_parsers(&self, registry: &mut OrchestratorRegistry) {
    for plugin in &self.plugins {
        for (descriptor, parser) in plugin.language_parsers() {
            registry.register(Box::new(DescriptorOrchestrator::new(descriptor, parser)));
        }
    }
}
```

**Step 3: Update `run_analyze()` in main.rs**

```rust
fn run_analyze(store_path: &Path, args: &AnalyzeArgs, loader: &plugin::PluginLoader) -> Result<()> {
    let mut store = open_or_create_store(store_path)?;

    let commit_ref = args.commit_ref.clone().or_else(|| detect_git_head(&args.path));

    let mut registry = svt_analyzer::orchestrator::OrchestratorRegistry::with_defaults();
    loader.register_language_parsers(&mut registry);

    let summary = svt_analyzer::analyze_project_with_registry(
        &mut store, &args.path, commit_ref.as_deref(), registry,
    ).map_err(|e| anyhow::anyhow!("{}", e))?;

    // ... rest unchanged ...
}
```

Update the call site in `main()` to pass `&loader` to `run_analyze()`.

**Step 4: Run all tests**

Run: `cargo test --workspace 2>&1 | grep "^test result:"`
Expected: All suites pass

**Step 5: Commit**

```bash
git add crates/analyzer/src/lib.rs crates/cli/src/main.rs crates/cli/src/plugin.rs
git commit -m "feat(cli): wire plugin language parsers into analysis pipeline"
```

---

### Task 11: Update Plugin List to Show Language Contributions

**Files:**
- Modify: `crates/cli/src/main.rs` (the `run_plugin_list` function)

**Step 1: Update `run_plugin_list()` to show language parsers**

Find the section that lists plugin contributions and add language parsers:

```rust
let parsers = plugin.language_parsers();
if !parsers.is_empty() {
    let langs: Vec<&str> = parsers.iter().map(|(d, _)| d.language_id.as_str()).collect();
    println!("    Languages: {}", langs.join(", "));
}
```

**Step 2: Run tests**

Run: `cargo test -p svt-cli plugin -v`
Expected: All plugin tests pass

**Step 3: Commit**

```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): show plugin language contributions in svt plugin list"
```

---

### Task 12: Dog-food Regression Verification

**Files:**
- No code changes — verification only

**Step 1: Run full test suite**

Run: `cargo test --workspace 2>&1`
Expected: All tests pass (366+ Rust tests)

**Step 2: Run clippy**

Run: `cargo clippy --workspace 2>&1`
Expected: No warnings (or only the existing `register_all` dead_code allow)

**Step 3: Run format check**

Run: `cargo fmt --check`
Expected: Clean

**Step 4: Dog-food analysis**

Run: `cargo run --bin svt -- import design/architecture.yaml && cargo run --bin svt -- analyze . && cargo run --bin svt -- check --analysis`
Expected:
- Import succeeds
- Analyze produces nodes and edges for all 4 languages
- Conformance: 12 passed, 0 failed, 0 warned, 0 not evaluable

**Step 5: Commit PROGRESS.md update**

Update `docs/plan/PROGRESS.md`:
- Add M18 row to completed milestones table
- Update test count
- Mark "Plugin Analyzer Support" known gap as RESOLVED
- Add M18 completion details section

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 18 (plugin analyzer support) complete"
```
