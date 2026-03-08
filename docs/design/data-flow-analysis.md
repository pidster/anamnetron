# Data Flow Analysis: Type-Traced Data Movement Through Call Chains

## Status: Draft
## Date: 2026-03-08

## Problem

The Flow View (see `docs/design/flow-view.md`) visualizes `calls` and `depends` edges but produces **zero `data_flow` edges**. Users see control flow (which function calls which) but cannot answer the question: *how does data move through the system, and how does it change shape along the way?*

Data flow is fundamentally different from control flow. A request may enter as raw bytes at a network listener, be deserialized into a domain message type, wrapped into a consensus proposal, transformed into a log entry, and finally persisted as a storage record. Each transformation boundary is architecturally significant — it reveals coupling, abstraction layers, and the true data pipeline of a system.

### Concrete Example

In a distributed system like aeon-rs:

```
network::server           → receives bytes
  ↓ deserializes to
client-message (enum)     → domain input type
  ↓ wrapped by
protocol::message-payload → protocol wrapper
  ↓ proposed to
multi-raft::raft-proposal → consensus input
  ↓ committed as
wal::log-entry            → persistence format
  ↓ applied to
storage::state-machine    → terminal sink
```

The analyzer already extracts the nodes (modules, types, functions) and `calls` edges between them. What is missing is the **data transformation pipeline** — the chain of type conversions that traces how data changes shape as it flows through the call graph.

### Why This Matters

- **Architecture comprehension**: Data flow paths reveal the true architecture of a system more clearly than call graphs. A module that transforms `HttpRequest` into `DomainCommand` is an anti-corruption layer, whether or not it was designed as one.
- **Impact analysis**: Changing a type's shape affects every downstream consumer in the data flow chain. `calls` edges alone cannot surface this.
- **Conformance**: Design-mode data flow constraints (e.g., "raw network types must not reach the storage layer") require `data_flow` and `transforms` edges to evaluate.

## Proposed Solution

### Overview

Add a **type-flow analysis pass** that runs after the existing per-file tree-sitter parsing. This pass examines function signatures (parameter types and return types), `From`/`Into` implementations, and constructor patterns to infer `data_flow` and `transforms` edges. These edges are then available in the graph for the Flow View to visualize.

The pass operates on the `ParseResult` output from language parsers — it does not require a second tree-sitter parse. It chains existing `calls` edges with type information to derive data movement.

### Architecture

```
                    Per-file parsing (existing)
                    ├── Items: modules, structs, enums, functions, traits
                    ├── Relations: contains, depends, calls, implements
                    └── Metadata: param types, return types (NEW)
                              │
                              ▼
                    Type-Flow Analysis Pass (NEW)
                    ├── Build type signature index
                    ├── Walk call chains across module boundaries
                    ├── Detect type transformations at each boundary
                    ├── Identify entry points and terminal sinks
                    └── Emit: data_flow edges, transforms edges
                              │
                              ▼
                    Orchestrator inserts into GraphStore
                    ├── Existing: nodes + contains/depends/calls/implements/exports
                    └── New: data_flow + transforms edges
```

## New Graph Schema Additions

### New Edge Kind: `transforms`

Represents a type being converted to another type. The source is the input type node; the target is the output type node.

| Field | Value |
|-------|-------|
| kind | `transforms` |
| source | Node ID of the input type (e.g., `ClientMessage`) |
| target | Node ID of the output type (e.g., `RaftProposal`) |
| provenance | `analysis` or `inferred` |

#### Metadata

| Key | Type | Description |
|-----|------|-------------|
| `mechanism` | string | How the transform occurs: `from_impl`, `into_impl`, `constructor`, `serde`, `manual` |
| `via_function` | string | Canonical path of the function performing the transform |
| `is_fallible` | bool | Whether the conversion can fail (`TryFrom`, `Result` return) |

### New Edge Kind: `data_flow`

Represents data moving between architectural elements (modules, components, services), potentially changing form. This is a higher-level edge — it aggregates one or more `transforms` edges into a module-to-module or component-to-component data movement.

| Field | Value |
|-------|-------|
| kind | `data_flow` |
| source | Node ID of the source module/component |
| target | Node ID of the target module/component |
| provenance | `analysis` or `inferred` |

#### Metadata

| Key | Type | Description |
|-----|------|-------------|
| `source_type` | string | Canonical path of the type leaving the source |
| `target_type` | string | Canonical path of the type entering the target |
| `transforms_chain` | string[] | Ordered list of `transforms` edge IDs in the chain |
| `direction` | string | `push` (source calls target), `pull` (target calls source), `channel` (async boundary) |
| `protocol` | string | `direct`, `channel`, `queue`, `rpc`, `http` (when inferrable) |

### EdgeKind Enum Update

In `crates/core/src/model/mod.rs`, the `EdgeKind` enum already has `DataFlow`. Add `Transforms`:

```rust
pub enum EdgeKind {
    Contains,
    Depends,
    Calls,
    Implements,
    Extends,
    DataFlow,
    Exports,
    Transforms,  // NEW
}
```

### AnalysisRelation Extension

The existing `AnalysisRelation` in `crates/core/src/analysis.rs` carries `source_qualified_name`, `target_qualified_name`, and `kind`. No structural change is needed — `transforms` and `data_flow` relations use the same shape. Edge metadata is passed through the existing `metadata` field on `Edge` after mapping.

### New Metadata on AnalysisItem

Function and method `AnalysisItem` nodes gain additional metadata fields to carry type signature information extracted during parsing:

| Key | Type | Description |
|-----|------|-------------|
| `param_types` | `[{"name": "req", "type": "HttpRequest"}]` | Parameter names and their resolved type qualified names |
| `return_type` | string | Resolved return type qualified name (stripped of `Result`, `Option` wrappers) |
| `is_entry_point` | bool | Heuristic flag: network listener, API handler, main function |
| `is_sink` | bool | Heuristic flag: storage writer, logger, serializer to external |

## Analysis Algorithm

### Phase 1: Enrich Function Signatures (per-file, during existing parse)

Extend each language parser to capture function parameter types and return types as metadata on function/method `AnalysisItem` nodes. This builds on existing infrastructure:

- **Rust**: `extract_param_types()` and `extract_type_from_annotation()` already resolve parameter types in `crates/analyzer/src/languages/rust.rs`. Extend to also capture the return type from `-> Type` annotations.
- **Go**: Extract parameter and return types from `func` declarations. Go's explicit types make this straightforward.
- **TypeScript**: Extract from type annotations (`: Type`) on parameters and return values. Generics are resolved to their base type.
- **Python**: Extract from type hints (`param: Type`, `-> Type`). Untyped parameters are skipped.
- **Java**: Extract from method declarations. Java's mandatory type annotations give complete coverage.

#### Rust-Specific: Return Type Extraction

Add return type extraction alongside the existing parameter type extraction. In `rust.rs`, after `extract_param_types()` is called for a function node, also extract the return type:

```rust
// Existing: parameter types
let param_types = extract_param_types(params, source, module_context, &use_aliases);

// New: return type
let return_type = node
    .child_by_field_name("return_type")
    .and_then(|rt| rt.child_by_field_name("type"))
    .and_then(|t| extract_type_from_annotation(t, source, module_context, &use_aliases));
```

Store both in the item's metadata:

```json
{
  "param_types": [{"name": "msg", "type": "svt_core::model::Node"}],
  "return_type": "svt_core::model::Edge"
}
```

### Phase 2: Build Type Signature Index (post-parse, pre-insertion)

After all files are parsed, build an in-memory index from the `ParseResult`:

```
TypeSignatureIndex:
  functions: Map<QualifiedName, FunctionSignature>
  type_conversions: Map<(SourceType, TargetType), Vec<ConversionPath>>
  from_impls: Map<TargetType, Vec<SourceType>>  // From<S> for T
  into_impls: Map<SourceType, Vec<TargetType>>  // Into<T> for S
```

Where:

```rust
struct FunctionSignature {
    qualified_name: String,
    param_types: Vec<(String, String)>,  // (param_name, type_qn)
    return_type: Option<String>,         // type_qn
    parent_module: String,               // containing module qn
}
```

#### Populating the Index

1. **Function signatures**: Iterate all `AnalysisItem` nodes with `sub_kind` in `["function", "method"]` and extract `param_types` and `return_type` from metadata.

2. **From/Into impls**: Iterate all `AnalysisRelation` edges with `kind == Implements`. For relations where the target is a `From<T>` or `Into<T>` trait, extract the type parameter to record the conversion. The Rust analyzer already tracks `impl From<SourceType> for TargetType` as an `Implements` relation — the source type is extractable from the trait's type argument.

3. **Constructor patterns**: Functions named `new`, `from`, `try_from`, `parse`, `deserialize` where the return type differs from the parameter types are recorded as potential conversions.

### Phase 3: Walk Call Chains and Detect Transformations

For each function in the index, compare its parameter types against its return type. When they differ, this function is a **transformation boundary**.

#### Algorithm

```
for each function F in TypeSignatureIndex:
    input_types = F.param_types (excluding self, &self, primitives)
    output_type = F.return_type

    if output_type is None or input_types is empty:
        continue

    for each (param_name, input_type) in input_types:
        if input_type != output_type and both are project-local types:
            emit transforms edge: input_type → output_type
                metadata: { mechanism: "manual", via_function: F.qualified_name }
```

#### Following Call Chains

To build end-to-end data flow paths, chain transformations through `calls` edges:

```
for each calls edge (caller → callee):
    caller_sig = TypeSignatureIndex[caller]
    callee_sig = TypeSignatureIndex[callee]

    if caller and callee are in different modules:
        // Cross-module call with type transformation
        if callee_sig.param_types overlap with caller_sig.return_type:
            // Data flows from caller's module to callee's module
            emit data_flow edge: caller.parent_module → callee.parent_module
                metadata: {
                    source_type: caller.return_type,
                    target_type: callee.param_types[matching],
                    direction: "push"
                }
```

#### From/Into Shortcut

When a `From<A> for B` implementation exists:

```
emit transforms edge: A → B
    metadata: { mechanism: "from_impl", is_fallible: false }
```

When a `TryFrom<A> for B` implementation exists:

```
emit transforms edge: A → B
    metadata: { mechanism: "from_impl", is_fallible: true }
```

### Phase 4: Identify Entry Points and Sinks

Classify functions as entry points or sinks using topology and heuristics (complementing the root detection in `crates/core/src/roots.rs`):

#### Entry Point Heuristics

| Signal | Confidence | Example |
|--------|-----------|---------|
| Call-tree root (no incoming `calls` edges) + has outgoing `data_flow` | High | `main()`, top-level handlers |
| Parent module named `server`, `handler`, `api`, `routes`, `controller` | Medium | `routes::create_user()` |
| First parameter is a network/request type (`HttpRequest`, `TcpStream`, `Request`) | Medium | `async fn handle(req: Request)` |
| Function name matches `handle_*`, `on_*`, `process_*`, `accept_*` | Low | `handle_connection()` |

#### Sink Heuristics

| Signal | Confidence | Example |
|--------|-----------|---------|
| Call-tree leaf (no outgoing `calls` edges) + has incoming `data_flow` | High | Storage write functions |
| Parent module named `storage`, `db`, `persistence`, `repository`, `wal` | Medium | `storage::put()` |
| Parameter includes a write/output type (`Writer`, `Sink`, `Connection`) | Medium | `fn persist(conn: &Connection, entry: LogEntry)` |
| Function name matches `write_*`, `save_*`, `persist_*`, `store_*`, `log_*` | Low | `write_to_disk()` |

Entry/sink classification is stored as metadata on the function's `AnalysisItem` node, not as a separate node kind.

### Phase 5: Chain Transformations into End-to-End Paths

Once individual `transforms` and `data_flow` edges are emitted, compute **data flow paths** — ordered sequences of transformations from an entry point to a sink:

```
for each entry_point E:
    BFS/DFS along data_flow edges from E:
        collect path: [E, module_1, module_2, ..., sink_S]
        collect transforms chain: [type_A → type_B → type_C → ... → type_N]
```

These paths are not stored as edges — they are **query results** computed on demand via `GraphStore::query_paths()` with `edge_kinds: [DataFlow]`. This follows the existing pattern where conformance is computed, not stored.

## Integration Points

### Analyzer Crate (`crates/analyzer/`)

#### New Module: `crates/analyzer/src/type_flow.rs`

The type-flow analysis pass, invoked by the orchestrator after per-file parsing completes:

```rust
pub struct TypeFlowAnalysis {
    signatures: TypeSignatureIndex,
    transforms: Vec<AnalysisRelation>,
    data_flows: Vec<AnalysisRelation>,
}

impl TypeFlowAnalysis {
    /// Build the analysis from parsed results.
    pub fn from_parse_results(results: &[ParseResult]) -> Self;

    /// Run the analysis, emitting transforms and data_flow relations.
    pub fn analyze(&mut self) -> Vec<AnalysisRelation>;
}
```

#### Orchestrator Integration

In `crates/analyzer/src/orchestrator/mod.rs`, after all language-specific parsing is complete and before insertion into the graph store, run the type-flow pass:

```rust
// After: all ParseResults collected from language parsers
// Before: mapping to graph nodes/edges and insertion

let mut type_flow = TypeFlowAnalysis::from_parse_results(&all_results);
let flow_relations = type_flow.analyze();

// Merge flow_relations into the combined result
combined_result.relations.extend(flow_relations);
```

#### Language Parser Changes

Each language parser's `parse()` method is extended to populate `param_types` and `return_type` in function/method item metadata. The changes are incremental — parsers that don't yet extract type information simply produce `None` for these fields, and the type-flow pass skips those functions.

**Priority order for language support:**

1. **Rust** — Strongest signals. Explicit types on all parameters and return values. Ownership semantics mean data movement is unambiguous. `From`/`Into` impls are explicit transform markers. The analyzer already extracts parameter types via `extract_param_types()`.

2. **Java** — Mandatory type annotations on all method parameters and return values. Constructor patterns (`new Foo(bar)`) are explicit. Interface implementations provide transform contracts.

3. **Go** — Explicit types. Multiple return values are common (data + error). Interface satisfaction is implicit but detectable from method sets.

4. **TypeScript** — Type annotations when present give good signals. Generics (`Promise<T>`, `Observable<T>`) need unwrapping. Many functions lack type annotations in JavaScript-heavy codebases.

5. **Python** — Type hints are optional. Coverage depends on the codebase's typing discipline. Dataclass/Pydantic model definitions provide structural type information.

### Core Crate (`crates/core/`)

- Add `Transforms` variant to `EdgeKind` enum in `crates/core/src/model/mod.rs`
- Update serialization/deserialization for the new variant (serde `rename_all = "snake_case"` handles this automatically)
- Update `crates/core/src/validation.rs` to validate `transforms` edges (source and target must be type nodes: `sub_kind` in `["struct", "enum", "class", "interface", "type_alias"]`)
- Update `crates/core/src/export/mermaid.rs` and `crates/core/src/export/dot.rs` to render `transforms` edges with distinct styling
- Update snapshot tests in `crates/core/src/export/snapshots/`

### Server Crate (`crates/server/`)

No new API endpoints needed. The existing `GET /api/projects/{project}/snapshots/{version}/graph` endpoint already returns all edges including any new edge kinds. The Flow View frontend filters by edge kind client-side.

### Web Frontend (`web/`)

#### Flow View Enhancement

In `web/src/components/FlowView.svelte` and `web/src/lib/flow-layout.ts`:

| Edge Kind | Visual Treatment | Animation |
|-----------|-----------------|-----------|
| `data_flow` | Gradient stroke (warm-to-cool, per flow-view.md) | Slower particles, larger dots (4px), different colour than `calls` |
| `transforms` | Thin dashed arrow between type nodes | No animation (structural relationship) |

#### New Toggle

Add `data_flow` and `transforms` to the edge-kind filter toggles in the Flow View toolbar. Default state: `data_flow` enabled, `transforms` disabled (to avoid clutter until the user wants type-level detail).

#### Data Flow Path Highlighting

When a user selects a node that is an entry point or participates in a data flow path:

1. Highlight the full data flow path from entry to sink
2. Show a **path summary panel** listing the type transformation chain:
   ```
   bytes → ClientMessage → RaftProposal → LogEntry → StorageRecord
   ```
3. Each type in the chain is clickable, navigating to the type's node in the graph

## Heuristics for Inferring Flow Direction

When function signatures alone are insufficient (e.g., a function takes `&self` and returns `()` but internally writes to a channel), apply these heuristics in priority order:

### 1. Explicit Transform Markers (High Confidence)

- `impl From<A> for B` — `A` flows to `B` (direction: A → B)
- `impl TryFrom<A> for B` — `A` flows to `B`, fallibly
- `impl Into<B> for A` — `A` flows to `B`
- Serde `#[derive(Serialize)]` on type T + `#[derive(Deserialize)]` on type U in a different module — serialization boundary

### 2. Constructor Patterns (Medium Confidence)

- `B::new(a: A)` — `A` flows to `B`
- `B::from(a: A)` — `A` flows to `B`
- `B { field: a }` where `a: A` — `A` flows to `B`
- Builder pattern: `BBuilder::new().field(a).build()` — `A` flows to `B`

### 3. Known Function Name Patterns (Medium Confidence)

- `parse(input: A) -> B` — `A` flows to `B`
- `convert(a: A) -> B` — `A` flows to `B`
- `serialize(t: T) -> Vec<u8>` — `T` flows to serialized form (sink)
- `deserialize(bytes: &[u8]) -> T` — bytes flow to `T` (entry)
- `map(f: Fn(A) -> B)` — `A` flows to `B`

### 4. Channel/Queue Patterns (Low Confidence, Async Boundaries)

- `sender.send(msg: A)` paired with `receiver.recv() -> A` — inferred `data_flow` edge across the channel boundary
- `tokio::sync::mpsc::Sender<A>` type in one module and `Receiver<A>` in another — cross-module async data flow
- This requires matching sender/receiver types across modules, which is tractable when the channel's type parameter is a project-local type

### 5. Naming Convention Fallbacks (Low Confidence)

- Module named `adapter`, `converter`, `mapper`, `transformer` — likely contains transformation logic
- Function named `to_*`, `as_*`, `into_*` — likely a type conversion

## Progressive Refinement Strategy

The analysis is designed to be implemented incrementally, with each phase adding value independently:

### Phase A: Function Signature Capture (Minimum Viable)

- Extend Rust parser to emit `return_type` metadata on function items
- No new edges yet — just enriched metadata
- **Value**: Foundation for all subsequent phases; useful for IDE-like features

### Phase B: From/Into Transform Detection

- Detect `From`/`Into` implementations and emit `transforms` edges
- **Value**: Explicit conversions are high-confidence and immediately useful

### Phase C: Cross-Module Data Flow from Call Chains

- Walk `calls` edges, compare parameter/return types across module boundaries
- Emit `data_flow` edges for cross-module calls where types change
- **Value**: The core data flow visualization

### Phase D: Entry/Sink Classification

- Apply heuristics to classify entry points and sinks
- Enrich Flow View with path highlighting
- **Value**: End-to-end data flow path visualization

### Phase E: Async Boundary Detection

- Detect channel/queue patterns
- Infer `data_flow` edges across async boundaries
- **Value**: Complete data flow picture in concurrent systems

### Phase F: Multi-Language Support

- Extend Go, TypeScript, Java, Python parsers with return type extraction
- **Value**: Data flow analysis for polyglot codebases

## Language-Specific Considerations

### Rust

**Strongest signals.** Rust's type system provides unambiguous data flow information:

- **Ownership**: When a function takes `T` (by value), data *moves* — the caller no longer has it. This is a strong directional signal absent in GC languages.
- **Borrowing**: `&T` and `&mut T` indicate read-only or mutable access without transfer. The type-flow pass should distinguish moves from borrows — only moves create `data_flow` edges by default, with borrows optionally included.
- **From/Into**: The `From` and `Into` traits are the canonical conversion mechanism. Every `impl From<A> for B` is an explicit, high-confidence `transforms` edge.
- **Error handling**: `Result<T, E>` wraps the success type. The pass should unwrap `Result` and `Option` to extract the underlying data type, similar to how `RUST_WELL_KNOWN_CONTAINERS` is handled.
- **Existing infrastructure**: `extract_param_types()`, `extract_type_from_annotation()`, and `infer_type_from_value()` in `crates/analyzer/src/languages/rust.rs` already resolve types from annotations and expressions. Adding return type extraction requires minimal new code.

### Go

- **Explicit types**: All parameters and return values are explicitly typed. Multiple return values (`(T, error)`) are common — the error return should be ignored for data flow purposes.
- **Interfaces**: Go's structural typing means a type satisfies an interface without explicit declaration. The analyzer can detect this from method sets, but it adds complexity. Initially, focus on concrete types.
- **Channels**: `chan T` types provide explicit async data flow markers. A function that sends to `chan<- Proposal` and another that receives from `<-chan Proposal` are connected by a `data_flow` edge with `direction: "channel"`.

### TypeScript

- **Optional types**: Type annotations are optional. The pass works only with annotated functions, which limits coverage in loosely-typed codebases.
- **Generics**: `Promise<T>`, `Observable<T>`, `Array<T>` need unwrapping to extract the data type, similar to Rust's well-known containers.
- **Class-based transforms**: TypeScript classes with constructors that take one type and expose another via methods are implicit transforms. Constructor parameter types vs. method return types reveal the transformation.

### Java

- **Complete type information**: Every method parameter and return value has a declared type. This gives the highest coverage of any supported language.
- **Inheritance**: `extends` and `implements` relationships provide explicit type hierarchy information. A method that accepts `Animal` and returns `Dog` is a narrowing transform.
- **Annotations**: Framework annotations (`@RequestBody`, `@ResponseBody`, `@Autowired`) provide strong hints about data flow direction. These are additive heuristics, not required for the core algorithm.
- **Stream API**: `stream().map(A::toB).collect()` chains are common data transformation patterns. Detecting these requires tracking lambda/method-reference types through stream pipelines — deferred to a future phase.

### Python

- **Type hints are optional**: Coverage depends entirely on the codebase's typing discipline. Projects using mypy/pyright with strict mode will have good coverage; others will have sparse data.
- **Dataclasses/Pydantic**: Model definitions (`@dataclass`, `BaseModel`) provide structural type information even when function signatures lack type hints.
- **Duck typing**: Python's dynamic nature means data flow can happen through untyped channels. The pass should be conservative — only emit edges when types are explicitly annotated.

## Limitations

1. **No runtime information**: This is static analysis only. Dynamic dispatch, reflection, and runtime-constructed calls are invisible.

2. **Generic type erasure**: When a function signature uses a generic type parameter (`fn process<T>(item: T) -> T`), the pass cannot determine the concrete type at each call site without whole-program monomorphization analysis. These functions are skipped.

3. **Closures and higher-order functions**: Closures capture types implicitly. A function that returns `impl Fn(A) -> B` is a transformation factory, but tracing the closure's types requires closure type inference, which is deferred.

4. **Cross-crate/cross-package analysis**: The type-flow pass operates on a single analysis snapshot. Types defined in external dependencies (outside the analyzed project) are treated as opaque — no `transforms` edges are emitted for external types.

5. **Channel matching**: Connecting `Sender<T>` to `Receiver<T>` across modules requires heuristic matching (same type parameter `T`, reasonable module proximity). False positives are possible when multiple channels carry the same type.

6. **Scale**: For large codebases (10,000+ functions), the call-chain walking in Phase 3 could be expensive. Mitigation: limit chain depth (configurable, default 10), process only cross-module boundaries, and cache the type signature index.

## Future Work

- **Interactive data flow exploration**: Click a type in the Flow View to see all functions that consume or produce that type.
- **Data flow constraints**: New constraint kinds for conformance mode — e.g., `must_not_flow` (raw network types must not reach the storage layer), `must_transform` (data must pass through a validation layer before persistence).
- **Runtime trace integration**: Correlate static data flow analysis with runtime traces (OpenTelemetry spans) to validate that the inferred paths match actual execution.
- **Data flow diff**: Compare data flow paths between two analysis snapshots to detect architectural drift in data pipelines.
- **Taint tracking**: Mark entry point types as "tainted" and track whether they pass through a sanitization transform before reaching a sink — a lightweight security analysis.

## References

- `docs/design/DATA_MODEL.md` — Graph schema, edge kinds, metadata conventions
- `docs/design/flow-view.md` — Flow View design, edge rendering styles, root detection
- `crates/core/src/model/mod.rs` — `EdgeKind` enum definition (line 79)
- `crates/core/src/analysis.rs` — `AnalysisItem`, `AnalysisRelation`, `ParseResult` types
- `crates/core/src/roots.rs` — Root detection (call-tree roots, dependency sinks)
- `crates/analyzer/src/languages/rust.rs` — `extract_param_types()` (line 1647), `extract_type_from_annotation()` (line 1557), `infer_type_from_value()` (line 1599)
- `crates/analyzer/src/orchestrator/mod.rs` — Analysis orchestration pipeline
