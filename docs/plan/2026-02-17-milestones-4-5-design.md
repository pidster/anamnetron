# Milestones 4 & 5: Server API + Web Frontend — Design

## Overview

Two sequential milestones to deliver the web-based architecture explorer:

- **Milestone 4:** Axum REST API serving graph data read-only from a pre-loaded store
- **Milestone 5:** Svelte + Cytoscape.js frontend consuming the API

WASM is deferred — the frontend fetches all data from the server API over HTTP. The WASM bridge to svt-core can be added in a later milestone once the UI is proven.

## Milestone 4: Server API

### Startup Modes

The server loads data at startup and stays read-only afterward. CLI flags control what gets loaded:

- `svt-server --project ./path` — runs `analyze_project()`, serves the analysis
- `svt-server --design ./design.yaml` — imports design YAML, serves design-only
- `svt-server --project ./path --design ./design.yaml` — both, enables full conformance
- Flags are combinable. At least one is required.

### Module Structure

```
crates/server/src/
  main.rs          — CLI args (clap), startup logic, launch Axum
  state.rs         — AppState (Arc-wrapped CozoStore + version metadata)
  routes/
    mod.rs         — router assembly, middleware (CORS, tracing)
    snapshots.rs   — list snapshots, version metadata
    nodes.rs       — get nodes, children, ancestors, search by path
    edges.rs       — get edges filtered by kind
    graph.rs       — full graph payload shaped for Cytoscape.js
    conformance.rs — evaluate/evaluate_design, return report
    health.rs      — health check
```

### AppState

```rust
struct AppState {
    store: CozoStore,  // CozoStore is already thread-safe (Arc internally)
    design_version: Option<Version>,
    analysis_version: Option<Version>,
}
```

Wrapped in `Arc<AppState>` for Axum's `State` extractor.

### API Endpoints

```
GET /api/health                                    → { status: "ok" }

GET /api/snapshots                                 → [{ version, kind }]

GET /api/snapshots/:version/nodes                  → [Node]
GET /api/snapshots/:version/nodes/:id              → Node
GET /api/snapshots/:version/nodes/:id/children     → [Node]
GET /api/snapshots/:version/nodes/:id/ancestors    → [Node]
GET /api/snapshots/:version/nodes/:id/dependencies → [Edge]
GET /api/snapshots/:version/nodes/:id/dependents   → [Edge]

GET /api/snapshots/:version/edges                  → [Edge]
GET /api/snapshots/:version/edges?kind=depends     → [Edge] (filtered)

GET /api/snapshots/:version/graph                  → CytoscapeGraph
GET /api/conformance/design/:version               → ConformanceReport
GET /api/conformance?design=V&analysis=V           → ConformanceReport

GET /api/search?path=GLOB&version=V                → [Node]
```

All endpoints return JSON. Errors return `{ "error": "message" }` with HTTP 400/404/500.

### The `/graph` Endpoint

The key endpoint for the frontend. Returns a pre-transformed payload matching Cytoscape.js element format:

```json
{
  "elements": {
    "nodes": [
      { "data": { "id": "...", "label": "core", "kind": "service", "parent": "svt" } }
    ],
    "edges": [
      { "data": { "id": "...", "source": "...", "target": "...", "kind": "depends" } }
    ]
  }
}
```

Contains edges become `parent` fields on child nodes (Cytoscape's compound node model) rather than separate edge elements. Only non-containment edges appear as edge elements.

### Error Handling

Custom `ApiError` enum using `thiserror`, implementing Axum's `IntoResponse`:

- `NotFound(String)` → 404
- `BadRequest(String)` → 400
- `StoreError(svt_core::store::Error)` → 500

No `anyhow` in route handlers — typed errors only. `anyhow` stays in `main.rs` for startup.

### Dependencies

Adding to existing `crates/server/Cargo.toml`:

- `serde` + `serde_json` — JSON serialization
- `tracing` + `tracing-subscriber` — structured logging (replaces `println!`)
- `clap` — CLI argument parsing

Already present: `axum`, `tokio`, `tower`, `tower-http` (cors, fs), `svt-core`, `svt-analyzer`.

### Testing

- Unit tests per route module using Axum's `Router` + `tower::ServiceExt::oneshot` (no running server needed)
- Integration test with a pre-loaded in-memory store hitting all endpoints
- Conformance endpoint tests using known design+analysis fixtures

## Milestone 5: Svelte Web Frontend

### Tooling

- Svelte 5 + TypeScript
- Vite + `@sveltejs/vite-plugin-svelte`
- Cytoscape.js with `cose-bilkent` and `dagre` layout plugins
- No WASM — all data from server API
- No routing library — single-page panel-based UI

### Directory Structure

```
web/
  package.json
  vite.config.ts
  tsconfig.json
  index.html
  src/
    main.ts                          — mount Svelte app
    App.svelte                       — top-level layout
    lib/
      api.ts                         — typed fetch wrappers for /api/*
      types.ts                       — TypeScript types mirroring API responses
    components/
      GraphView.svelte               — Cytoscape.js rendering + interaction
      NodeDetail.svelte              — side panel: metadata, edges, source ref
      ConformanceReport.svelte       — constraint results, unimplemented/undocumented
      SnapshotSelector.svelte        — version picker (design/analysis)
      SearchBar.svelte               — glob-based node search
    stores/
      graph.ts                       — reactive: selected version, graph data
      selection.ts                   — reactive: selected node, panel state
```

### Interaction Flow

1. App loads → fetches `GET /api/snapshots` → renders `SnapshotSelector`
2. User picks version → fetches `GET /api/snapshots/:v/graph` → renders `GraphView`
3. Click node → fetches children/dependencies → populates `NodeDetail` panel
4. Select design + analysis versions → fetch conformance → overlay on graph + show report

### Cytoscape.js Configuration

- **Compound nodes:** Containment hierarchy via `parent` field. Click to expand/collapse subtrees.
- **Edge styling by kind:**
  - `depends` — solid line
  - `data_flow` — dashed line
  - `implements` — dotted line
  - `contains` — hidden (expressed as compound node nesting)
- **Conformance overlay** (when report loaded):
  - Green border — constraint passed / node matched
  - Red border — constraint violation
  - Orange border — unimplemented (design node missing in analysis)
  - Grey — undocumented (analysis node not in design)
- **Layouts:**
  - `cose-bilkent` — force-directed with compound node support (default)
  - `dagre` — hierarchical/layered (alternative, good for dependency flow)

### Build & Serving

- **Development:** `npm run dev` — Vite dev server with HMR, proxies `/api/*` to `svt-server:3000`
- **Production:** `npm run build` → `web/dist/` → served by `svt-server` via `tower-http::ServeDir` at `/`
- **Single binary (future):** Could embed `web/dist/` with `rust-embed` later. Not v1.

### Frontend Dependencies

- `svelte` — UI framework
- `cytoscape` — graph rendering
- `cytoscape-cose-bilkent` — compound-node-aware layout
- `cytoscape-dagre` — hierarchical layout
- Dev: TypeScript, Vite, `@sveltejs/vite-plugin-svelte`, Vitest

### Testing

- Vitest for unit tests on `api.ts`, `types.ts`, store logic
- No E2E browser tests this milestone — manual testing against real server
- Server integration tests (Milestone 4) cover the API contract

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| WASM | Deferred | Ship faster, prove UI first, add WASM later |
| Data loading | Both modes (existing store OR project path) | Flexible — works with CLI-prepared data or standalone |
| Graph viz | Cytoscape.js | Compound nodes, good layouts, matches TECH_STACK.md |
| Frontend routing | None (panel-based SPA) | Graph view always visible, simpler than URL routing |
| Static serving | Disk-based (`web/dist/`) | Simple, no Rust build complexity. Embed later if needed |
| API style | REST with JSON | Standard, easy to test with curl, matches Axum's strengths |

## Milestone Boundary

Milestone 4 is independently useful — the API can be explored with curl, httpie, or any HTTP client. Milestone 5 builds on a stable, tested API. This matches the existing milestone pattern and allows parallel work if desired.
