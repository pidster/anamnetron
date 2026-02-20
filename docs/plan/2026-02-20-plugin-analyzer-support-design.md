# M18: Plugin Analyzer Support — Design

## Goal

Allow external plugins to contribute language analyzers via a descriptor + parser API, and refactor the built-in Go, Python, and TypeScript analyzers to use the same mechanism.

## Architecture

Approach A: **Descriptor + Parser in svt-core.** Plugins provide a `LanguageDescriptor` (data describing how to discover project units) and a `LanguageParser` (trait for parsing source files). The host handles discovery, file walking, and orchestration via a generic `DescriptorOrchestrator` in svt-analyzer.

This keeps the plugin API simple (plugins only parse), standardises discovery across languages, and dog-foods the API by using it for built-in languages.

## Types Moving to svt-core

Four types move from `crates/analyzer/src/types.rs` and `crates/analyzer/src/languages/mod.rs` to a new `crates/core/src/analysis.rs` module. They only depend on `NodeKind` and `EdgeKind` (already in svt-core).

```rust
// crates/core/src/analysis.rs

use std::path::{Path, PathBuf};
use crate::model::{EdgeKind, NodeKind};

/// A code element extracted by static analysis.
#[derive(Debug, Clone)]
pub struct AnalysisItem {
    pub qualified_name: String,
    pub kind: NodeKind,
    pub sub_kind: String,
    pub parent_qualified_name: Option<String>,
    pub source_ref: String,
    pub language: String,
}

/// A relationship between code elements.
#[derive(Debug, Clone)]
pub struct AnalysisRelation {
    pub source_qualified_name: String,
    pub target_qualified_name: String,
    pub kind: EdgeKind,
}

/// A non-fatal warning from analysis.
#[derive(Debug, Clone)]
pub struct AnalysisWarning {
    pub source_ref: String,
    pub message: String,
}

/// Result of parsing source files for a single language unit.
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub items: Vec<AnalysisItem>,
    pub relations: Vec<AnalysisRelation>,
    pub warnings: Vec<AnalysisWarning>,
}
```

`crates/analyzer/src/types.rs` re-exports these from core to avoid breaking internal code. Language-specific discovery structs (`CrateInfo`, `TsPackageInfo`, etc.) stay in analyzer — they're internal implementation details.

## New Plugin API Types in svt-core

### LanguageDescriptor

A plain data struct describing how to discover and collect source files for a language. Discovery is data-driven, not behaviour-driven — the host implements walking/matching once.

```rust
/// Describes how to discover project units for a language.
#[derive(Debug, Clone)]
pub struct LanguageDescriptor {
    /// Unique language identifier (e.g., "rust", "go", "java").
    pub language_id: String,
    /// Manifest filenames that indicate a project unit
    /// (e.g., ["go.mod"], ["package.json"], ["pyproject.toml", "setup.py"]).
    pub manifest_files: Vec<String>,
    /// Source file extensions to collect (e.g., [".go"], [".py"]).
    pub source_extensions: Vec<String>,
    /// Directories to skip during walking (e.g., ["vendor", "node_modules"]).
    pub skip_directories: Vec<String>,
    /// The NodeKind for top-level units (typically NodeKind::Service).
    pub top_level_kind: NodeKind,
    /// Sub-kind label for top-level units (e.g., "module", "package").
    pub top_level_sub_kind: String,
}
```

### LanguageParser

Trait for parsing source files into analysis items and relations. Plugin authors implement this.

```rust
/// Parse source files for a language.
pub trait LanguageParser: Send + Sync {
    /// Parse source files for a single project unit.
    fn parse(&self, unit_name: &str, files: &[&Path]) -> ParseResult;

    /// Optional: emit additional structural items (e.g., directory-based modules).
    fn emit_structural_items(
        &self,
        _source_root: &Path,
        _unit_name: &str,
        _source_files: &[PathBuf],
    ) -> Vec<AnalysisItem> {
        vec![]
    }

    /// Optional: post-process parse results (e.g., reparenting items).
    fn post_process(
        &self,
        _source_root: &Path,
        _unit_name: &str,
        _result: &mut ParseResult,
    ) {}
}
```

### SvtPlugin Extension

```rust
pub trait SvtPlugin: Send + Sync {
    // ... existing methods ...

    /// Language parsers contributed by this plugin.
    fn language_parsers(&self) -> Vec<(LanguageDescriptor, Box<dyn LanguageParser>)> {
        Vec::new()
    }
}
```

## Host-Side: DescriptorOrchestrator

New file `crates/analyzer/src/orchestrator/descriptor.rs`. A generic orchestrator that wraps any `(LanguageDescriptor, Box<dyn LanguageParser>)` pair.

```rust
pub struct DescriptorOrchestrator {
    descriptor: LanguageDescriptor,
    parser: Box<dyn LanguageParser>,
}

impl LanguageOrchestrator for DescriptorOrchestrator {
    fn language_id(&self) -> &str { &self.descriptor.language_id }

    fn discover(&self, root: &Path) -> Vec<LanguageUnit> {
        discover_by_descriptor(root, &self.descriptor)
    }

    fn analyze(&self, unit: &LanguageUnit) -> ParseResult {
        let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
        self.parser.parse(&unit.name, &file_refs)
    }

    fn emit_structural_items(&self, unit: &LanguageUnit) -> Vec<AnalysisItem> {
        self.parser.emit_structural_items(&unit.source_root, &unit.name, &unit.source_files)
    }

    fn post_process(&self, unit: &LanguageUnit, result: &mut ParseResult) {
        self.parser.post_process(&unit.source_root, &unit.name, result)
    }
}
```

### discover_by_descriptor() Algorithm

1. Walk `root` recursively, skipping `descriptor.skip_directories`
2. When a `descriptor.manifest_files` match is found, that directory is a project unit
3. Derive unit name from the manifest:
   - Try JSON: extract `"name"` field
   - Try TOML: extract `[project].name` or `[package].name`
   - For `go.mod`: regex `^module (.+)$`, take last path segment
   - Fallback: parent directory name
4. Collect all files with `descriptor.source_extensions` under that directory
5. Build `LanguageUnit` with descriptor's `top_level_kind` / `top_level_sub_kind`

## Built-in Refactoring

### Go, Python, TypeScript → Descriptor + Parser

Each language provides a `descriptor()` and `parser()` constructor. The custom orchestrator files are replaced by `DescriptorOrchestrator` wrappers.

| Language | Manifest files | Extensions | Skip dirs | Escape hatches |
|----------|---------------|-----------|-----------|----------------|
| Go | `go.mod` | `.go` | `vendor` | None |
| Python | `pyproject.toml`, `setup.py` | `.py` | `venv`, `.venv`, `__pycache__` | None |
| TypeScript | `package.json` | `.ts`, `.tsx`, `.svelte` | `node_modules`, `dist` | `emit_structural_items` + `post_process` |

### Rust — Keeps Custom Orchestrator

Rust stays as `RustOrchestrator` implementing `LanguageOrchestrator` directly because:
- Discovery uses `cargo metadata` (not manifest file walking)
- Workspace-aware qualified name logic
- `extra_items()` for workspace root node

`RustAnalyzer` implements `LanguageParser` so the parsing interface is uniform. Rust bypasses `DescriptorOrchestrator` only for discovery.

### OrchestratorRegistry::with_defaults()

```rust
// 1 custom + 3 descriptor-based
registry.register(Box::new(RustOrchestrator::new()));
registry.register(Box::new(DescriptorOrchestrator::new(
    TypeScriptAnalyzer::descriptor(), TypeScriptAnalyzer::parser()
)));
registry.register(Box::new(DescriptorOrchestrator::new(
    GoAnalyzer::descriptor(), GoAnalyzer::parser()
)));
registry.register(Box::new(DescriptorOrchestrator::new(
    PythonAnalyzer::descriptor(), PythonAnalyzer::parser()
)));
```

## CLI Wiring

`run_analyze()` in `crates/cli/src/main.rs` gains plugin support:

```rust
// After building OrchestratorRegistry with defaults...
for plugin in loader.plugins() {
    for (descriptor, parser) in plugin.language_parsers() {
        registry.register(Box::new(DescriptorOrchestrator::new(descriptor, parser)));
    }
}
```

## Error Handling

- **Discovery errors are non-fatal.** Walk failures produce warnings, not errors. One language failing doesn't block others.
- **Manifest name extraction** falls back through JSON → TOML → go.mod regex → directory name. If all fail, skip with warning.
- **Duplicate language IDs.** If a plugin registers a `language_id` matching a built-in, warn and skip. First registered wins.
- **Empty source files.** If a manifest is found but no matching source files exist, skip silently.
- **WASM compatibility.** All new types in svt-core have no platform-specific dependencies.

## Testing Strategy

### Core types (`crates/core/src/analysis.rs`)
- `ParseResult` construction and field access
- Mock `LanguageParser` returning fixed items/relations
- Default `emit_structural_items` and `post_process` are no-ops

### DescriptorOrchestrator (`crates/analyzer/src/orchestrator/descriptor.rs`)
- `discover_by_descriptor` with tempdir: manifest + source files → correct `LanguageUnit`
- Skip directories respected
- Multiple manifest files (pyproject.toml OR setup.py) both detected
- Name extraction: JSON, TOML, go.mod, directory fallback
- Empty directory → empty vec
- Nested projects discovered correctly

### Built-in regression
- All existing orchestrator/analyzer tests pass (test behaviour, not implementation)
- Remove custom Go/Python/TypeScript orchestrator files (replaced by descriptor)
- Rust orchestrator tests unchanged
- Dog-food: `svt analyze .` produces same node/edge counts

### Plugin integration (`crates/cli/tests/plugin_cli.rs`)
- Mock plugin contributing a language parser shows in `svt plugin list`

### Key regression check
- 366 Rust tests + 22 vitest tests all pass
- Dog-food conformance: 12 passed, 0 failed, 0 warned, 0 not evaluable
