# Flow View: Entry Point Detection & Force-Directed Visualization

## Status: Approved (with refinements)
## Date: 2026-03-07

## Problem

The existing visualization modes (Treemap, Bundle, Matrix, Chord, Sunburst, Mermaid) show structural relationships — containment, dependency counts, module coupling. None answer the questions:

- Where does execution start?
- How does a request flow through the system?
- What are the entry points (main functions, network listeners, event handlers)?
- What does the call/data path look like from a given root?

## Design

### 1. Generic Root Detection (Topology-Based)

Rather than encoding framework-specific patterns (Spring annotations, Axum routes, Express middleware), roots are computed from **graph topology**. This works for any language or framework.

#### Root Categories

| Category | Detection Rule | Example |
|----------|---------------|---------|
| **Call-tree roots** | Nodes with outgoing `Calls` edges but zero incoming `Calls` edges | `fn main()`, top-level test functions |
| **Dependency sources** | Nodes depended on by many but depending on few (high in-degree / low out-degree on `Depends` edges) | `core::Config`, shared libraries |
| **Dependency sinks** | Nodes that depend on many but nothing depends on them (high out-degree / low in-degree on `Depends`) | Application entry modules, CLI |
| **Containment roots** | `System`/`Service` nodes at the top of the `Contains` hierarchy | Workspace root, crate roots |
| **Leaf sinks** | Nodes with only incoming edges (pure consumers) | Database adapters, output formatters |

#### Heuristic Enrichment (Optional, Additive)

On top of topology, lightweight heuristics refine classification via **tags** and **metadata** on `AnalysisItem`. No schema changes needed.

- **Name-based**: `sub_kind == "function"` && name == `main` → tag `entry_point`
- **Module-based**: Parent module named `rest`, `api`, `handler`, `controller`, `routes`, `server` → tag `network_entry`
- **Attribute-based**: Already detecting `#[test]`, `@Test` → tag `test_entry`
- **Metadata**: Store hints like `{"entry_hint": "call_tree_root", "fan_out": 12, "fan_in": 0}`

These heuristics are intentionally shallow — they don't require understanding frameworks, only recognizing common naming conventions. They are secondary to the topological analysis.

### 2. Flow View: Force-Directed Visualization

A new 7th view mode added alongside the existing views.

#### Layout

**Organic force-directed graph** with root-aware forces:

- Detected roots are pinned/attracted toward the top (or given stronger upward force)
- Leaf sinks attracted toward the bottom
- Intermediate nodes settle organically based on their connections
- Module clustering via attractive forces between siblings (nodes sharing a `Contains` parent)

This gives a natural "gravity" where execution flows downward from entry points to leaves, while related nodes cluster together organically.

#### Edge Rendering by Type

Each edge kind has distinct visual treatment:

| Edge Kind | Style | Animation | Purpose |
|-----------|-------|-----------|---------|
| `Calls` | Solid, medium weight | Animated particles (dots flowing source→target) | Runtime invocation flow |
| `DataFlow` | Gradient stroke (warm→cool) | Slower particle animation, different dot style | Data movement |
| `Depends` | Dashed, thin | None (static) | Structural/import relationship |
| `Implements` | Dotted, thin | None | Type contract |
| `Extends` | Dotted, thin, different color | None | Inheritance |
| `Exports` | Dashed, thin | None | Visibility boundary |

Animations are subtle — small dots (2-3px) moving along edge paths at a gentle pace. `Calls` and `DataFlow` get animation because they represent runtime behavior; structural edges (`Depends`, `Implements`, `Extends`, `Exports`) remain static.

#### Node Interaction

**On node selection:**

1. Selected node highlighted with a glow/ring
2. Connected subgraph highlighted (all nodes reachable via edges of any filtered type)
3. Non-connected nodes dim to ~20% opacity
4. On-canvas floating labels appear on connected nodes showing:
   - Node name
   - Edge type badge (small colored pill: "calls", "depends", etc.)
   - Direction indicator (→ outgoing, ← incoming)
5. Multiple edge types between the same pair shown as parallel offset paths

**On hover:**
- Tooltip with node details (name, kind, sub_kind, LOC, fan-in/out)
- Immediate neighbor edges brighten

**Edge type filtering:**
- Toggle buttons per edge kind (matching existing filter infrastructure)
- Toggling off `Depends` leaves only runtime edges visible — shows pure execution flow
- Toggling off `Calls` leaves structural view — shows dependency architecture

#### Progressive Disclosure (Scale Handling)

For large graphs (hundreds+ of nodes):

1. **Initial view**: Show detected roots + depth-1 neighbors, capped at ~200 visible nodes via module-collapse (a graph with 50 roots x 20 neighbors = 1000 nodes would overwhelm)
2. **Expand on click/double-click**: Reveal next depth level for selected node
3. **Module collapse**: Nodes within a module collapse into a single "module node" showing aggregate edge counts
4. **Semantic zoom**: Zooming in progressively reveals more detail (labels appear, then sub-nodes expand)
5. **Search + reveal**: Search for a node → auto-expand path from nearest root to that node

#### Root Highlighting

Detected roots get distinct visual treatment:

- **Call-tree roots**: Larger node, bold border, positioned higher in layout
- **Network entry hints**: Icon overlay or shape change (e.g., hexagon instead of circle)
- **Test entry points**: Existing test-tag styling, but also positioned as roots when viewing test flow
- **Containment roots**: Shown as background regions (compound nodes) rather than point nodes — included from the start since fcose supports compound nodes natively and module clustering needs visual anchors

### 3. Implementation Approach

#### Backend (Rust)

New module: **`crates/core/src/roots.rs`** behind the `store` feature flag (same pattern as `validation.rs`, `conformance.rs`, `diff.rs`).

```rust
// crates/core/src/roots.rs
#[cfg(feature = "store")]

pub struct RootAnalysis {
    pub call_tree_roots: Vec<NodeId>,
    pub dependency_sources: Vec<NodeId>,
    pub dependency_sinks: Vec<NodeId>,
    pub containment_roots: Vec<NodeId>,
    pub leaf_sinks: Vec<NodeId>,
}

pub fn detect_roots(store: &dyn GraphStore, version: Version) -> Result<RootAnalysis>;
```

This function queries the graph store for edge statistics per node and classifies them. It depends only on `model` and `store` — NOT on `analysis.rs` (which is about source-code parsing types). Computed on demand (not persisted); O(nodes + edges) traversal is lightweight enough to skip caching.

Add to `lib.rs`:
```rust
#[cfg(feature = "store")]
pub mod roots;
```

#### Frontend (Svelte + Cytoscape/Library)

New component: `FlowView.svelte`

**Library evaluation order:**
1. **Cytoscape.js + cytoscape-fcose** (force-directed compound graph layout) — first choice, consistent with potential existing usage, supports compound nodes, edge animations via extensions
2. **Cytoscape.js + cytoscape-elk** (Eclipse Layout Kernel) — alternative layout engine if fcose doesn't handle root pinning well
3. **d3-force** with custom rendering — if Cytoscape's canvas performance is insufficient at scale
4. **Three.js / deck.gl** — only if 2D force-directed hits hard limits on the target graph sizes

Edge particle animation via **custom canvas overlay** (preferred — the `cytoscape-edge-animation` extension is unmaintained):
- Transparent canvas layered on top of the Cytoscape canvas
- requestAnimationFrame loop drawing dots along edge bezier paths
- Single speed/density setting initially; configurable only if users request it

#### Data Flow

```
GraphStore → detect_roots() → RootAnalysis
                                    ↓
API: GET /api/projects/{project}/snapshots/{version}/roots
                                    ↓
flowStore (new Svelte store)
  - roots: RootAnalysis
  - expandedDepth: Map<NodeId, number>
  - activeEdgeKinds: Set<EdgeKind>
  - animationEnabled: boolean
                                    ↓
FlowView.svelte
  - Receives filteredVisibleGraph + roots
  - Configures force-directed layout with root pinning
  - Renders edges with type-specific styles
  - Handles selection → subgraph highlight + on-canvas labels
```

### 4. Edge Cases & Constraints

- **Cyclic call graphs**: Force-directed handles cycles naturally (no DAG requirement). Cycles should be visually detectable (particles flowing in loops).
- **Disconnected components**: Multiple root clusters will separate naturally in force-directed layout. Add a weak central gravity to prevent drift.
- **No detected roots**: Fall back to showing nodes with highest fan-out as probable roots. Always show something useful.
- **WASM compatibility**: Root detection logic in `core/` must remain WASM-compatible (no filesystem, no network calls). All graph queries go through `GraphStore` trait.
- **Performance target**: Smooth interaction (>30fps) with up to 1000 visible nodes and 5000 edges. Beyond that, progressive disclosure / module collapsing kicks in.

### 5. Out of Scope (Future)

- 3D visualization (Three.js) — evaluate after 2D flow view ships
- Framework-specific annotation parsing (Spring, Axum, Express) — topology-first approach should cover 80% of value
- Time-based flow animation (showing request lifecycle) — requires runtime trace data, not static analysis
- Diff overlay on flow view (comparing two versions) — future enhancement

## Resolved Questions

1. **Persisted or on-demand?** — Compute on demand. O(nodes + edges) is lightweight. Cache at API layer if needed later.
2. **Particle animation subtlety?** — Single speed/density setting to start. Configurable only if users request it.
3. **Compound nodes from the start?** — Yes. fcose supports them natively and module clustering needs visual anchors.

## Architecture Review Notes

Reviewed 2026-03-07. Approved. Key decisions:
- `roots.rs` in `core/` behind `store` feature flag (same pattern as validation/conformance/diff)
- API endpoint: `GET /api/projects/{project}/snapshots/{version}/roots` (consistent path params)
- Custom canvas overlay for edge animation (not unmaintained extension)
- Initial view capped at ~200 visible nodes via module-collapse
- Performance risk: `get_all_edges()` loads all edges into memory — acceptable for now, add bulk stats query if profiling shows bottleneck
