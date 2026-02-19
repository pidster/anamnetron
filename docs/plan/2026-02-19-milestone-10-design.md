# Milestone 10: Plugin Foundations — Design

## Goal

Introduce trait-based registries for the three hardcoded extension points (constraint evaluators, export formats, language analyzers) so that new implementations can be added without modifying dispatch logic.

## Scope

- Define `ConstraintEvaluator`, `ExportFormat`, and extended `LanguageAnalyzer` traits
- Create `ConstraintRegistry`, `ExportRegistry`, and `AnalyzerRegistry` structs
- Refactor existing built-ins into trait implementations registered at startup
- Replace hardcoded `match` dispatch with registry lookups in CLI, server, and conformance engine
- No dynamic loading, no external plugin discovery — static registration via `with_defaults()` + `.register()`

## Architecture

### Trait Definitions

Three traits, one per extension point:

**ConstraintEvaluator** (in `crates/core/src/conformance.rs`):

```rust
pub trait ConstraintEvaluator: Send + Sync {
    fn kind(&self) -> &str;
    fn evaluate(
        &self,
        store: &dyn GraphStore,
        design_version: Version,
        analysis_version: Option<Version>,
        constraint: &Constraint,
    ) -> Result<ConstraintResult, StoreError>;
}
```

**ExportFormat** (in `crates/core/src/export/mod.rs`):

```rust
pub trait ExportFormat: Send + Sync {
    fn name(&self) -> &str;
    fn export(
        &self,
        store: &dyn GraphStore,
        version: Version,
    ) -> Result<String, StoreError>;
}
```

**LanguageAnalyzer** (in `crates/analyzer/src/languages/mod.rs`) — already exists as a trait. Extended with:

```rust
pub trait LanguageAnalyzer: Send + Sync {
    fn language_id(&self) -> &str;
    fn can_analyze(&self, project_path: &Path) -> bool;
    fn discover(&self, project_path: &Path) -> Result<Vec<AnalysisUnit>>;
    fn analyze_unit(&self, unit: &AnalysisUnit) -> ParseResult;
}
```

### Registry Structs

Each registry wraps a `HashMap<String, Box<dyn Trait>>`:

```rust
pub struct ConstraintRegistry {
    evaluators: HashMap<String, Box<dyn ConstraintEvaluator>>,
}

impl ConstraintRegistry {
    pub fn new() -> Self;
    pub fn with_defaults() -> Self;       // registers all 4 built-in evaluators
    pub fn register(&mut self, evaluator: Box<dyn ConstraintEvaluator>);
    pub fn get(&self, kind: &str) -> Option<&dyn ConstraintEvaluator>;
    pub fn kinds(&self) -> Vec<&str>;     // for CLI help text
}
```

Same pattern for `ExportRegistry` (2 built-ins: mermaid, json) and `AnalyzerRegistry` (2 built-ins: rust, typescript).

### Where Things Live

| Component | Crate | Location |
|-----------|-------|----------|
| `ConstraintEvaluator` trait | svt-core | `conformance.rs` |
| `ConstraintRegistry` | svt-core | `conformance.rs` |
| Built-in evaluator structs | svt-core | `conformance.rs` (extracted from existing functions) |
| `ExportFormat` trait | svt-core | `export/mod.rs` |
| `ExportRegistry` | svt-core | `export/mod.rs` |
| Built-in exporter structs | svt-core | `export/mermaid.rs`, `export/json.rs` |
| `LanguageAnalyzer` trait | svt-analyzer | `languages/mod.rs` (extended) |
| `AnalyzerRegistry` | svt-analyzer | `lib.rs` or new `registry.rs` |
| Built-in analyzer structs | svt-analyzer | `languages/rust.rs`, `languages/typescript.rs` |

### GraphStore as `dyn`

Trait methods that currently take `impl GraphStore` change to `&dyn GraphStore`. This is required for trait object compatibility. The `GraphStore` trait needs a `Send + Sync` bound added. This is a mechanical change — CozoStore already satisfies both bounds.

### Wiring

**CLI:** Creates registries with `with_defaults()` at startup, passes them to `run_check()`, `run_export()`, `run_analyze()`.

**Server:** `AppState` gains registry fields. Routes use registry lookups.

**Conformance engine:** `evaluate()` takes `&ConstraintRegistry` and looks up evaluators by `constraint.kind` instead of matching.

**Analyzer:** `analyze_project()` takes `&AnalyzerRegistry`. Iterates registered analyzers, calls `can_analyze()` to detect applicable languages, runs discovery + analysis for each.

### Error Handling

Unknown constraint kind → `ConstraintResult` with `NotEvaluable` status (same as today).
Unknown export format → `anyhow!("Unknown format: {name}. Available: {list}")`.
No applicable analyzer → warning logged, empty analysis (graceful degradation).

## Out of Scope

- Dynamic plugin loading (dlopen, WASM plugins)
- Plugin configuration files
- Plugin versioning / API stability guarantees
- New built-in analyzers, constraint types, or export formats
- REST API plugin routes
