# Milestone 10: Plugin Foundations — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Introduce trait-based registries for constraint evaluators, export formats, and language analyzers, replacing all hardcoded dispatch with registry lookups.

**Architecture:** Three registries, each following the same pattern: a trait defining the extension point, built-in structs implementing it, and a `Registry` struct with `register()`/`get()`/`with_defaults()`. Public conformance/export APIs change from `impl GraphStore` to `&dyn GraphStore` for trait object compatibility. The `evaluate_constraints()` match statement, `run_export()` match statement, and `analyze_project()` hardcoded analyzer construction are all replaced by registry lookups.

**Tech Stack:** Rust traits, `HashMap<String, Box<dyn Trait>>`, existing svt-core and svt-analyzer crates.

---

### Task 1: Define ConstraintEvaluator Trait and Extract Built-in Structs

**Files:**
- Modify: `crates/core/src/conformance.rs`

This task introduces the `ConstraintEvaluator` trait and wraps the 4 existing evaluator functions (`evaluate_constraint_must_not_depend`, `evaluate_constraint_boundary`, `evaluate_constraint_must_contain`, `evaluate_constraint_max_fan_in`) in structs that implement the trait. The functions themselves stay as-is for now — the structs delegate to them. Signatures change from `&impl GraphStore` to `&dyn GraphStore`.

**Step 1: Write the failing test**

Add to the `tests` module in `crates/core/src/conformance.rs`:

```rust
#[test]
fn constraint_evaluator_trait_returns_correct_kind() {
    let evaluator = MustNotDependEvaluator;
    assert_eq!(evaluator.kind(), "must_not_depend");

    let evaluator = BoundaryEvaluator;
    assert_eq!(evaluator.kind(), "boundary");

    let evaluator = MustContainEvaluator;
    assert_eq!(evaluator.kind(), "must_contain");

    let evaluator = MaxFanInEvaluator;
    assert_eq!(evaluator.kind(), "max_fan_in");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core constraint_evaluator_trait_returns_correct_kind`
Expected: FAIL — `MustNotDependEvaluator` not found.

**Step 3: Define the trait and 4 structs**

Add to `crates/core/src/conformance.rs` (after the existing types, before the functions):

```rust
/// Extension point for constraint evaluation.
///
/// Each constraint kind (e.g., "must_not_depend", "boundary") is implemented
/// as a struct that implements this trait. The registry dispatches to the
/// correct evaluator based on `kind()`.
pub trait ConstraintEvaluator: Send + Sync {
    /// The constraint kind string this evaluator handles (e.g., "must_not_depend").
    fn kind(&self) -> &str;

    /// Evaluate a constraint against the store data.
    ///
    /// `design_version` is the version containing constraints.
    /// `analysis_version` is `Some(v)` for design-vs-analysis mode, `None` for design-only.
    /// `eval_version` is the version to evaluate against (may differ from design_version).
    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult>;
}

/// Built-in evaluator for `must_not_depend` constraints.
#[derive(Debug)]
pub struct MustNotDependEvaluator;

impl ConstraintEvaluator for MustNotDependEvaluator {
    fn kind(&self) -> &str { "must_not_depend" }

    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_must_not_depend(store, constraint, eval_version)
    }
}

/// Built-in evaluator for `boundary` constraints.
#[derive(Debug)]
pub struct BoundaryEvaluator;

impl ConstraintEvaluator for BoundaryEvaluator {
    fn kind(&self) -> &str { "boundary" }

    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_boundary(store, constraint, eval_version)
    }
}

/// Built-in evaluator for `must_contain` constraints.
#[derive(Debug)]
pub struct MustContainEvaluator;

impl ConstraintEvaluator for MustContainEvaluator {
    fn kind(&self) -> &str { "must_contain" }

    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_must_contain(store, constraint, eval_version)
    }
}

/// Built-in evaluator for `max_fan_in` constraints.
#[derive(Debug)]
pub struct MaxFanInEvaluator;

impl ConstraintEvaluator for MaxFanInEvaluator {
    fn kind(&self) -> &str { "max_fan_in" }

    fn evaluate(
        &self,
        store: &dyn GraphStore,
        constraint: &Constraint,
        eval_version: Version,
    ) -> Result<ConstraintResult> {
        evaluate_constraint_max_fan_in(store, constraint, eval_version)
    }
}
```

Also change the 4 existing evaluator function signatures from `store: &impl GraphStore` to `store: &dyn GraphStore`. These are the functions at lines 104, 182, 252, 327:

- `evaluate_constraint_must_not_depend(store: &dyn GraphStore, ...)`
- `evaluate_constraint_boundary(store: &dyn GraphStore, ...)`
- `evaluate_constraint_must_contain(store: &dyn GraphStore, ...)`
- `evaluate_constraint_max_fan_in(store: &dyn GraphStore, ...)`

Also change the helper functions:
- `structural_checks(store: &dyn GraphStore, ...)`
- `evaluate_constraints(store: &dyn GraphStore, ...)`

And the public API:
- `evaluate_design(store: &dyn GraphStore, ...)`
- `evaluate(store: &dyn GraphStore, ...)`

**Step 4: Run all tests to verify nothing broke**

Run: `cargo test -p svt-core`
Expected: All 80 unit tests + integration tests pass. The `&dyn GraphStore` change is transparent since `CozoStore` implements `GraphStore`.

**Step 5: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "feat(core): add ConstraintEvaluator trait and 4 built-in evaluator structs"
```

---

### Task 2: Add ConstraintRegistry

**Files:**
- Modify: `crates/core/src/conformance.rs`

**Step 1: Write the failing test**

Add to the `tests` module in `crates/core/src/conformance.rs`:

```rust
#[test]
fn constraint_registry_with_defaults_has_all_built_ins() {
    let registry = ConstraintRegistry::with_defaults();
    assert!(registry.get("must_not_depend").is_some());
    assert!(registry.get("boundary").is_some());
    assert!(registry.get("must_contain").is_some());
    assert!(registry.get("max_fan_in").is_some());
    assert!(registry.get("unknown_kind").is_none());

    let mut kinds = registry.kinds();
    kinds.sort();
    assert_eq!(kinds, vec!["boundary", "max_fan_in", "must_contain", "must_not_depend"]);
}

#[test]
fn constraint_registry_register_adds_evaluator() {
    let mut registry = ConstraintRegistry::new();
    assert!(registry.get("must_not_depend").is_none());
    registry.register(Box::new(MustNotDependEvaluator));
    assert!(registry.get("must_not_depend").is_some());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core constraint_registry`
Expected: FAIL — `ConstraintRegistry` not found.

**Step 3: Implement ConstraintRegistry**

Add to `crates/core/src/conformance.rs` (after the evaluator structs):

```rust
/// Registry of constraint evaluators, keyed by kind string.
///
/// Use `with_defaults()` to get a registry pre-loaded with all built-in
/// evaluators. Use `register()` to add custom evaluators.
pub struct ConstraintRegistry {
    evaluators: std::collections::HashMap<String, Box<dyn ConstraintEvaluator>>,
}

impl ConstraintRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            evaluators: std::collections::HashMap::new(),
        }
    }

    /// Create a registry with all built-in evaluators pre-registered.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(MustNotDependEvaluator));
        registry.register(Box::new(BoundaryEvaluator));
        registry.register(Box::new(MustContainEvaluator));
        registry.register(Box::new(MaxFanInEvaluator));
        registry
    }

    /// Register a constraint evaluator. Replaces any existing evaluator for the same kind.
    pub fn register(&mut self, evaluator: Box<dyn ConstraintEvaluator>) {
        self.evaluators.insert(evaluator.kind().to_string(), evaluator);
    }

    /// Look up an evaluator by constraint kind.
    pub fn get(&self, kind: &str) -> Option<&dyn ConstraintEvaluator> {
        self.evaluators.get(kind).map(|e| e.as_ref())
    }

    /// List all registered constraint kinds.
    pub fn kinds(&self) -> Vec<&str> {
        self.evaluators.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for ConstraintRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core constraint_registry`
Expected: Both new tests pass.

**Step 5: Commit**

```bash
git add crates/core/src/conformance.rs
git commit -m "feat(core): add ConstraintRegistry with register/get/with_defaults"
```

---

### Task 3: Wire ConstraintRegistry into Conformance Engine

**Files:**
- Modify: `crates/core/src/conformance.rs`

Replace the hardcoded `match` in `evaluate_constraints()` with a registry lookup. The `evaluate_design()` and `evaluate()` public functions gain a `&ConstraintRegistry` parameter.

**Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn evaluate_design_with_registry_works() {
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
    let registry = ConstraintRegistry::with_defaults();
    let report = evaluate_design(&store, version, &registry).unwrap();
    assert_eq!(report.summary.passed, 3); // 2 structural + 1 must_not_depend
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core evaluate_design_with_registry_works`
Expected: FAIL — `evaluate_design` doesn't accept a registry parameter yet.

**Step 3: Update evaluate_constraints, evaluate_design, and evaluate**

Change `evaluate_constraints()` (the private helper) to use the registry:

```rust
fn evaluate_constraints(
    store: &dyn GraphStore,
    constraint_version: Version,
    eval_version: Version,
    registry: &ConstraintRegistry,
) -> Result<Vec<ConstraintResult>> {
    let mut results = Vec::new();
    let constraints = store.get_constraints(constraint_version)?;
    for constraint in &constraints {
        let result = match registry.get(&constraint.kind) {
            Some(evaluator) => evaluator.evaluate(store, constraint, eval_version)?,
            None => ConstraintResult {
                constraint_name: constraint.name.clone(),
                constraint_kind: constraint.kind.clone(),
                status: ConstraintStatus::NotEvaluable,
                severity: constraint.severity,
                message: format!("{} not evaluable", constraint.kind),
                violations: vec![],
            },
        };
        results.push(result);
    }
    Ok(results)
}
```

Update `evaluate_design`:

```rust
pub fn evaluate_design(
    store: &dyn GraphStore,
    version: Version,
    registry: &ConstraintRegistry,
) -> Result<ConformanceReport> {
    let mut results = structural_checks(store, version)?;
    results.extend(evaluate_constraints(store, version, version, registry)?);
    let summary = compute_summary(&results, 0, 0);
    // ... rest unchanged
}
```

Update `evaluate`:

```rust
pub fn evaluate(
    store: &dyn GraphStore,
    design_version: Version,
    analysis_version: Version,
    registry: &ConstraintRegistry,
) -> Result<ConformanceReport> {
    // ... same body but pass registry to evaluate_constraints
    results.extend(evaluate_constraints(store, design_version, analysis_version, registry)?);
    // ...
}
```

Update ALL existing tests that call `evaluate_design()` or `evaluate()` to pass `&ConstraintRegistry::with_defaults()`. This is mechanical — add `&ConstraintRegistry::with_defaults()` as the last argument. There are approximately 12 test functions that need updating.

**Step 4: Run all tests to verify everything passes**

Run: `cargo test -p svt-core`
Expected: All tests pass. The registry dispatches to the same evaluators as before.

**Step 5: Run full workspace tests to catch callers**

Run: `cargo test`
Expected: Compilation errors in `crates/cli/src/main.rs` (lines 178-181) and `crates/server/src/routes/conformance.rs` (lines 27, 36) because `evaluate_design` and `evaluate` now require a registry parameter. Fix these by adding `&ConstraintRegistry::with_defaults()`:

In `crates/cli/src/main.rs`:
```rust
use svt_core::conformance::ConstraintRegistry;
// ...
let registry = ConstraintRegistry::with_defaults();
// In run_check, change:
conformance::evaluate(&store, design_version, analysis_version, &registry)
// and:
conformance::evaluate_design(&store, design_version, &registry)
```

In `crates/server/src/routes/conformance.rs`:
```rust
use svt_core::conformance::ConstraintRegistry;
// ...
let registry = ConstraintRegistry::with_defaults();
let report = conformance::evaluate_design(&state.store, version, &registry)?;
// and:
let report = conformance::evaluate(&state.store, params.design, params.analysis, &registry)?;
```

**Step 6: Run full workspace tests again**

Run: `cargo test`
Expected: All 282 tests pass (277 Rust + 5 vitest).

**Step 7: Commit**

```bash
git add crates/core/src/conformance.rs crates/cli/src/main.rs crates/server/src/routes/conformance.rs
git commit -m "refactor(core): wire ConstraintRegistry into conformance engine"
```

---

### Task 4: Define ExportFormat Trait and Extract Built-in Structs

**Files:**
- Modify: `crates/core/src/export/mod.rs`
- Modify: `crates/core/src/export/mermaid.rs`

**Step 1: Write the failing test**

Add a test module to `crates/core/src/export/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_trait_returns_correct_name() {
        let mermaid = MermaidExporter;
        assert_eq!(mermaid.name(), "mermaid");

        let json = JsonExporter;
        assert_eq!(json.name(), "json");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core export_format_trait_returns_correct_name`
Expected: FAIL — `MermaidExporter` not found.

**Step 3: Define the trait and 2 structs**

In `crates/core/src/export/mod.rs`:

```rust
//! Export graph data in various formats.

pub mod mermaid;

use crate::model::Version;
use crate::store::{GraphStore, Result};

/// Extension point for export formats.
///
/// Each format (e.g., "mermaid", "json") is implemented as a struct
/// that implements this trait.
pub trait ExportFormat: Send + Sync {
    /// The format name (used in CLI `--format` flag).
    fn name(&self) -> &str;

    /// Export graph data for the given version as a string.
    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String>;
}

/// Built-in Mermaid flowchart exporter.
#[derive(Debug)]
pub struct MermaidExporter;

impl ExportFormat for MermaidExporter {
    fn name(&self) -> &str { "mermaid" }

    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        mermaid::to_mermaid(store, version)
    }
}

/// Built-in JSON interchange exporter.
#[derive(Debug)]
pub struct JsonExporter;

impl ExportFormat for JsonExporter {
    fn name(&self) -> &str { "json" }

    fn export(&self, store: &dyn GraphStore, version: Version) -> Result<String> {
        crate::interchange_store::export_json(store, version)
    }
}
```

Change `mermaid::to_mermaid` signature from `store: &impl GraphStore` to `store: &dyn GraphStore`.

Check `interchange_store::export_json` — if it uses `impl GraphStore`, change it to `&dyn GraphStore` as well.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p svt-core`
Expected: All tests pass, including the new one.

**Step 5: Commit**

```bash
git add crates/core/src/export/mod.rs crates/core/src/export/mermaid.rs crates/core/src/interchange_store.rs
git commit -m "feat(core): add ExportFormat trait with MermaidExporter and JsonExporter"
```

---

### Task 5: Add ExportRegistry and Wire into CLI

**Files:**
- Modify: `crates/core/src/export/mod.rs`
- Modify: `crates/cli/src/main.rs`

**Step 1: Write the failing test**

Add to the test module in `crates/core/src/export/mod.rs`:

```rust
#[test]
fn export_registry_with_defaults_has_all_built_ins() {
    let registry = ExportRegistry::with_defaults();
    assert!(registry.get("mermaid").is_some());
    assert!(registry.get("json").is_some());
    assert!(registry.get("unknown").is_none());

    let mut names = registry.names();
    names.sort();
    assert_eq!(names, vec!["json", "mermaid"]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-core export_registry_with_defaults`
Expected: FAIL — `ExportRegistry` not found.

**Step 3: Implement ExportRegistry**

Add to `crates/core/src/export/mod.rs`:

```rust
/// Registry of export formats, keyed by format name.
pub struct ExportRegistry {
    formats: std::collections::HashMap<String, Box<dyn ExportFormat>>,
}

impl ExportRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            formats: std::collections::HashMap::new(),
        }
    }

    /// Create a registry with all built-in formats pre-registered.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(MermaidExporter));
        registry.register(Box::new(JsonExporter));
        registry
    }

    /// Register an export format. Replaces any existing format with the same name.
    pub fn register(&mut self, format: Box<dyn ExportFormat>) {
        self.formats.insert(format.name().to_string(), format);
    }

    /// Look up a format by name.
    pub fn get(&self, name: &str) -> Option<&dyn ExportFormat> {
        self.formats.get(name).map(|f| f.as_ref())
    }

    /// List all registered format names.
    pub fn names(&self) -> Vec<&str> {
        self.formats.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for ExportRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
```

**Step 4: Run tests to verify registry tests pass**

Run: `cargo test -p svt-core export_registry`
Expected: PASS.

**Step 5: Wire ExportRegistry into CLI**

In `crates/cli/src/main.rs`, update `run_export()`:

```rust
use svt_core::export::ExportRegistry;

fn run_export(store_path: &Path, args: &ExportArgs) -> Result<()> {
    use svt_core::model::SnapshotKind;

    let store = open_store(store_path)?;
    let export_registry = ExportRegistry::with_defaults();

    let version = match args.version {
        Some(v) => v,
        None => store
            .latest_version(SnapshotKind::Design)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| anyhow::anyhow!("No design versions found in store"))?,
    };

    let exporter = export_registry.get(&args.format).ok_or_else(|| {
        let available = export_registry.names().join(", ");
        anyhow::anyhow!("Unknown format: '{}'. Available: {}", args.format, available)
    })?;

    let content = exporter
        .export(&store, version)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &content)
            .with_context(|| format!("writing to {}", output_path.display()))?;
        println!("Exported to {}", output_path.display());
    } else {
        print!("{content}");
    }

    Ok(())
}
```

**Step 6: Run full workspace tests**

Run: `cargo test`
Expected: All tests pass. CLI integration tests for export still work.

**Step 7: Commit**

```bash
git add crates/core/src/export/mod.rs crates/cli/src/main.rs
git commit -m "feat(core): add ExportRegistry and wire into CLI export command"
```

---

### Task 6: Extend LanguageAnalyzer Trait

**Files:**
- Modify: `crates/analyzer/src/languages/mod.rs`
- Modify: `crates/analyzer/src/languages/rust.rs`
- Modify: `crates/analyzer/src/languages/typescript.rs`

The current `LanguageAnalyzer` trait only has `analyze_crate()`. We need to extend it with `language_id()` for registry keying. The discovery and file routing will be handled separately in the registry.

**Step 1: Write the failing test**

Add to `crates/analyzer/src/languages/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_analyzer_has_correct_language_id() {
        let analyzer = rust::RustAnalyzer::new();
        assert_eq!(analyzer.language_id(), "rust");
    }

    #[test]
    fn typescript_analyzer_has_correct_language_id() {
        let analyzer = typescript::TypeScriptAnalyzer::new();
        assert_eq!(analyzer.language_id(), "typescript");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer rust_analyzer_has_correct_language_id`
Expected: FAIL — no method `language_id` on `RustAnalyzer`.

**Step 3: Add language_id() to LanguageAnalyzer trait**

In `crates/analyzer/src/languages/mod.rs`:

```rust
/// A language-specific source code analyzer.
pub trait LanguageAnalyzer: Send + Sync {
    /// Unique identifier for this language (e.g., "rust", "typescript").
    fn language_id(&self) -> &str;

    /// Parse a set of source files for a crate and return extracted items and relations.
    fn analyze_crate(&self, crate_name: &str, files: &[&Path]) -> ParseResult;
}
```

In `crates/analyzer/src/languages/rust.rs`, add to the existing `impl LanguageAnalyzer for RustAnalyzer`:

```rust
fn language_id(&self) -> &str { "rust" }
```

In `crates/analyzer/src/languages/typescript.rs`, add to the existing `impl LanguageAnalyzer for TypeScriptAnalyzer`:

```rust
fn language_id(&self) -> &str { "typescript" }
```

**Step 4: Run tests**

Run: `cargo test -p svt-analyzer`
Expected: All 75 tests pass (66 unit + 3 dogfood + 6 integration).

**Step 5: Commit**

```bash
git add crates/analyzer/src/languages/mod.rs crates/analyzer/src/languages/rust.rs crates/analyzer/src/languages/typescript.rs
git commit -m "feat(analyzer): add language_id() to LanguageAnalyzer trait"
```

---

### Task 7: Add AnalyzerRegistry and Wire into analyze_project

**Files:**
- Modify: `crates/analyzer/src/languages/mod.rs`
- Modify: `crates/analyzer/src/lib.rs`

**Step 1: Write the failing test**

Add to the test module in `crates/analyzer/src/languages/mod.rs`:

```rust
#[test]
fn analyzer_registry_with_defaults_has_all_built_ins() {
    let registry = AnalyzerRegistry::with_defaults();
    assert!(registry.get("rust").is_some());
    assert!(registry.get("typescript").is_some());
    assert!(registry.get("python").is_none());

    let mut ids = registry.language_ids();
    ids.sort();
    assert_eq!(ids, vec!["rust", "typescript"]);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p svt-analyzer analyzer_registry_with_defaults`
Expected: FAIL — `AnalyzerRegistry` not found.

**Step 3: Implement AnalyzerRegistry**

Add to `crates/analyzer/src/languages/mod.rs`:

```rust
/// Registry of language analyzers, keyed by language ID.
pub struct AnalyzerRegistry {
    analyzers: std::collections::HashMap<String, Box<dyn LanguageAnalyzer>>,
}

impl AnalyzerRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            analyzers: std::collections::HashMap::new(),
        }
    }

    /// Create a registry with all built-in analyzers pre-registered.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(rust::RustAnalyzer::new()));
        registry.register(Box::new(typescript::TypeScriptAnalyzer::new()));
        registry
    }

    /// Register a language analyzer. Replaces any existing analyzer for the same language.
    pub fn register(&mut self, analyzer: Box<dyn LanguageAnalyzer>) {
        self.analyzers.insert(analyzer.language_id().to_string(), analyzer);
    }

    /// Look up an analyzer by language ID.
    pub fn get(&self, language_id: &str) -> Option<&dyn LanguageAnalyzer> {
        self.analyzers.get(language_id).map(|a| a.as_ref())
    }

    /// List all registered language IDs.
    pub fn language_ids(&self) -> Vec<&str> {
        self.analyzers.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for AnalyzerRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
```

**Step 4: Run tests to verify registry tests pass**

Run: `cargo test -p svt-analyzer analyzer_registry`
Expected: PASS.

**Step 5: Wire AnalyzerRegistry into analyze_project()**

In `crates/analyzer/src/lib.rs`, change `analyze_project()` to accept a registry and use it instead of hardcoded constructors:

```rust
pub fn analyze_project(
    store: &mut impl GraphStore,
    project_root: &Path,
    commit_ref: Option<&str>,
    analyzer_registry: &AnalyzerRegistry,
) -> Result<AnalysisSummary, AnalyzerError> {
    // ...

    // Phase 1: Rust analysis
    let layout = discover_project(project_root)?;
    let rust_analyzer = analyzer_registry.get("rust");

    if let Some(rust_analyzer) = rust_analyzer {
        for crate_info in &layout.crates {
            // ... same as before, use rust_analyzer.analyze_crate(...)
        }
    }

    // Phase 2: TypeScript/Svelte analysis
    let ts_packages = discover_ts_packages(project_root).unwrap_or_default();
    let ts_analyzer = analyzer_registry.get("typescript");

    if let Some(ts_analyzer) = ts_analyzer {
        for package in &ts_packages {
            // ... same as before, use ts_analyzer.analyze_crate(...)
        }
    }

    // ... rest unchanged
}
```

Remove the direct imports of `RustAnalyzer` and `TypeScriptAnalyzer` from the function body (they're now accessed through the registry).

Re-export `AnalyzerRegistry` from `lib.rs`:

```rust
pub use crate::languages::AnalyzerRegistry;
```

**Step 6: Update callers**

In `crates/cli/src/main.rs`, update `run_analyze()`:

```rust
use svt_analyzer::AnalyzerRegistry;

fn run_analyze(store_path: &Path, args: &AnalyzeArgs) -> Result<()> {
    let mut store = open_or_create_store(store_path)?;
    let analyzer_registry = AnalyzerRegistry::with_defaults();

    let summary = svt_analyzer::analyze_project(
        &mut store, &args.path, args.commit_ref.as_deref(), &analyzer_registry
    ).map_err(|e| anyhow::anyhow!("{}", e))?;
    // ... rest unchanged
}
```

Update `crates/server/src/main.rs` if it calls `analyze_project()` directly (it likely passes the store at startup).

Update test files that call `analyze_project()` — add `&AnalyzerRegistry::with_defaults()` as the last argument.

**Step 7: Run full workspace tests**

Run: `cargo test`
Expected: All tests pass.

**Step 8: Commit**

```bash
git add crates/analyzer/src/languages/mod.rs crates/analyzer/src/lib.rs crates/cli/src/main.rs crates/server/
git commit -m "feat(analyzer): add AnalyzerRegistry and wire into analyze_project"
```

---

### Task 8: Final Verification and Documentation

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Run full verification**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo audit
wasm-pack build crates/wasm --target web
cd web && npm test && cd ..
```

Expected: All pass. Clippy should be clean — the `&dyn GraphStore` change and new types should not introduce warnings.

**Step 2: Verify dogfood conformance**

```bash
cargo run --bin svt -- import design/architecture.yaml
cargo run --bin svt -- analyze .
cargo run --bin svt -- check --analysis
```

Expected: Conformance check runs, results are reasonable (passes + some not_evaluable for must_contain constraints).

**Step 3: Update PROGRESS.md**

Update the milestone table to mark M10 as complete. Add to the completed milestones table:

```
| **10** | Plugin Foundations | 2026-02-19 | [test count] | ConstraintEvaluator/ExportFormat/LanguageAnalyzer traits, ConstraintRegistry/ExportRegistry/AnalyzerRegistry, all built-ins refactored to trait impls, registry dispatch replaces hardcoded match |
```

Update "Current state" with the new test count.

Move M10 from "Suggested Next Milestones" to completed. Remove the "Plugin API" item from "Not Yet Built" (mark as done).

Add to plan documents table:

```
| `2026-02-19-milestone-10-design.md` | M10 design (COMPLETE) |
| `2026-02-19-milestone-10-implementation.md` | M10 implementation plan (COMPLETE) |
```

**Step 4: Commit**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 10 as complete with plugin foundations"
```
