# Known Gaps Cleanup Design

## Goal

Clean up three known gaps before proceeding to Milestone 16 (Dynamic Plugin Loading):
1. **Analyzer Wiring** -- refactor `analyze_project()` from hardcoded per-language phases to registry-based dispatch via a `LanguageOrchestrator` trait
2. **Warning Noise** -- aggregate the ~3,500 method-call warnings into a single summary warning per file
3. **PROGRESS.md Accuracy** -- update stale documentation to reflect current state

## Gap 1: Analyzer Wiring Refactor

### Problem

`analyze_project()` in `crates/analyzer/src/lib.rs` has 4 hardcoded language phases (~220 lines), each importing specific analyzer and discovery functions directly. Adding a new language requires modifying `lib.rs`, adding imports, and writing a new phase block. The `AnalyzerRegistry` exists (M10) but is unused in the pipeline.

### Design

Introduce a `LanguageOrchestrator` trait that bundles discovery, analysis, and post-processing per language. The existing `LanguageAnalyzer` trait remains unchanged (it handles parsing). The orchestrator handles everything else: discovery, top-level node emission, structural items, and post-processing.

#### New types

```rust
/// A discovered package/module/crate for any language.
pub struct LanguageUnit {
    pub name: String,
    pub language: String,
    pub root: PathBuf,
    pub source_root: PathBuf,
    pub source_files: Vec<PathBuf>,
    pub top_level_kind: NodeKind,
    pub top_level_sub_kind: String,
    pub source_ref: String,
}

/// Orchestrates discovery + analysis + post-processing for a language.
pub trait LanguageOrchestrator: Send + Sync {
    fn language_id(&self) -> &str;
    fn discover(&self, root: &Path) -> Vec<LanguageUnit>;
    fn analyze(&self, unit: &LanguageUnit) -> ParseResult;
    fn emit_structural_items(&self, _unit: &LanguageUnit) -> Vec<AnalysisItem> { vec![] }
    fn post_process(&self, _unit: &LanguageUnit, _result: &mut ParseResult) {}
}
```

#### Orchestrator implementations

- **`RustOrchestrator`**: Calls `discover_project()`, emits workspace root node + crate nodes, delegates parsing to `RustAnalyzer`. No post-processing.
- **`TypeScriptOrchestrator`**: Calls `discover_ts_packages()`, emits package nodes. Overrides `emit_structural_items()` for directory/file module emission. Overrides `post_process()` for item reparenting and relative import resolution.
- **`GoOrchestrator`**: Calls `discover_go_packages()`, emits module nodes. No post-processing.
- **`PythonOrchestrator`**: Calls `discover_python_packages()`, emits package nodes. No post-processing.

#### Registry

```rust
pub struct OrchestratorRegistry {
    orchestrators: Vec<Box<dyn LanguageOrchestrator>>,
}

impl OrchestratorRegistry {
    pub fn with_defaults() -> Self { /* register all 4 */ }
    pub fn register(&mut self, orchestrator: Box<dyn LanguageOrchestrator>) { ... }
    pub fn orchestrators(&self) -> &[Box<dyn LanguageOrchestrator>] { ... }
}
```

#### Simplified pipeline

```rust
pub fn analyze_project(...) -> Result<AnalysisSummary, AnalyzerError> {
    let registry = OrchestratorRegistry::with_defaults();
    let mut all_items = Vec::new();
    let mut all_relations = Vec::new();
    let mut all_warnings = Vec::new();
    let mut files_analyzed = 0;
    let mut language_counts: HashMap<String, usize> = HashMap::new();

    for orchestrator in registry.orchestrators() {
        let units = orchestrator.discover(project_root);
        *language_counts.entry(orchestrator.language_id().to_string()).or_default() += units.len();

        for unit in &units {
            // 1. Emit top-level node (common)
            all_items.push(AnalysisItem { /* from unit fields */ });

            // 2. Emit structural items (TS override, others return empty)
            all_items.extend(orchestrator.emit_structural_items(unit));

            // 3. Analyze files (common)
            let file_refs: Vec<&Path> = unit.source_files.iter().map(|p| p.as_path()).collect();
            files_analyzed += file_refs.len();
            let mut parse_result = orchestrator.analyze(unit);

            // 4. Post-process (TS override for reparenting + imports)
            orchestrator.post_process(unit, &mut parse_result);

            // 5. Accumulate
            all_items.extend(parse_result.items);
            all_relations.extend(parse_result.relations);
            all_warnings.extend(parse_result.warnings);
        }
    }

    // Map to graph + create snapshot (unchanged)
    ...
}
```

#### Rust workspace special case

`RustOrchestrator::discover()` returns `Vec<LanguageUnit>` for crates. The workspace root node is emitted as an extra item prepended to the first crate's discover results, or emitted separately before iteration. The workspace name detection stays in discovery.rs.

#### File organization

- `crates/analyzer/src/orchestrator.rs` -- `LanguageUnit`, `LanguageOrchestrator` trait, `OrchestratorRegistry`
- `crates/analyzer/src/orchestrator/rust.rs` -- `RustOrchestrator`
- `crates/analyzer/src/orchestrator/typescript.rs` -- `TypeScriptOrchestrator` (absorbs `emit_ts_module_items`, `file_to_module_qn`, `resolve_ts_import`)
- `crates/analyzer/src/orchestrator/go.rs` -- `GoOrchestrator`
- `crates/analyzer/src/orchestrator/python.rs` -- `PythonOrchestrator`

The existing `LanguageAnalyzer` trait and `AnalyzerRegistry` remain for the parsing-only concern. Orchestrators own their respective analyzers internally.

## Gap 2: Warning Noise Reduction

### Problem

The Rust analyzer emits one `AnalysisWarning` per method call it encounters (~3,500 during dog-food analysis). These are not actionable since tree-sitter cannot resolve receiver types.

### Design

In `visit_call_expressions()` in `rust.rs`, instead of pushing one warning per method call, track a count per file. After processing each file, emit a single aggregated warning:

```
"42 method calls could not be resolved (syntax-only analysis)"
```

This reduces ~3,500 warnings to ~one per file that has method calls. The `method_call_generates_warning` test is updated to verify the aggregated format.

Similarly, the `unresolvable relation` warnings in `mapping.rs` are deduplicated by tracking seen target names and emitting one summary warning per category at the end.

## Gap 3: PROGRESS.md Accuracy

Update after implementation:
- Mark "Analyzer Wiring" gap as RESOLVED
- Update "Analysis Depth" gap to reflect reduced warning noise
- Update test counts
- Ensure all resolved/partially-resolved tags are accurate
