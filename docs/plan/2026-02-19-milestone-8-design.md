# Milestone 8: WASM Bridge — Design

## Goal

Compile svt-core to WASM via a new `crates/wasm` bridge crate, enabling browser-side graph browsing without server round-trips after initial snapshot load.

## Scope

- New `crates/wasm` crate (`svt-wasm`) with wasm-bindgen exports
- `WasmStore` class wrapping CozoDB in-memory store
- Read-only GraphStore subset exposed to JavaScript
- Server-loaded snapshot data (JSON) imported into browser-side CozoDB
- TypeScript wrapper (`web/src/lib/wasm.ts`) with typed API
- Web stores modified to use WASM for detail lookups after snapshot load

## Architecture

```
web/ (TypeScript) → svt-wasm (wasm-bindgen) → svt-core (CozoDB in-memory)
```

The server still handles analysis, import, and conformance. WASM handles read-only graph browsing after the initial data load.

The WASM module receives data as serialized JSON (the same format the API already returns) rather than a new wire format. `load_snapshot` parses the JSON and inserts nodes/edges into the in-memory CozoDB.

### Data Flow

1. Browser loads, fetches snapshots list from server API.
2. User selects version; frontend fetches `/api/snapshots/{v}/nodes` + `/api/snapshots/{v}/edges`.
3. Frontend calls `wasmStore.loadSnapshot(nodes, edges)` — WASM inserts into CozoDB.
4. All subsequent browsing (node detail, children, ancestors, dependencies) — WASM calls, zero API round-trips.
5. Graph view still uses server's `/api/snapshots/{v}/graph` for initial Cytoscape layout.

## Crate Structure

```
crates/wasm/
  Cargo.toml          — depends on svt-core (default features include store + CozoDB)
  src/lib.rs          — WasmStore struct + all wasm-bindgen exports
```

## WASM API Surface

The `WasmStore` class exposes these methods to JavaScript. All return `JsValue` (serialized JSON) since complex Rust types can't cross the WASM boundary directly. Methods return `Result<JsValue, JsError>` — errors become JavaScript exceptions.

### Data Loading

| Method | Description |
|--------|-------------|
| `WasmStore::new()` | Create an in-memory CozoDB store |
| `load_snapshot(json: &str)` | Parse server response (nodes + edges arrays), create snapshot, batch-insert. Returns version number. |

### Read-Only Browsing

| Method | Returns |
|--------|---------|
| `get_node(version, id)` | Single node JSON |
| `get_node_by_path(version, path)` | Single node JSON |
| `get_all_nodes(version)` | Array of nodes |
| `get_children(version, node_id)` | Array of child nodes |
| `get_parent(version, node_id)` | Optional parent node |
| `get_ancestors(version, node_id)` | Array of ancestor nodes |
| `get_descendants(version, node_id)` | Array of descendant nodes |
| `get_edges(version, node_id, direction, kind?)` | Array of edges |
| `get_all_edges(version, kind?)` | Array of edges |
| `get_dependencies(version, node_id, transitive)` | Array of nodes |
| `get_dependents(version, node_id, transitive)` | Array of nodes |
| `search(version, glob_pattern)` | Array of matching nodes |

### Not Exposed (Server-Only)

- `create_snapshot`, `add_node`, `add_edge`, `add_constraint`, `compact`
- Conformance evaluation
- Analysis (tree-sitter stays server-side)

## Web Integration

### New Files

```
web/src/lib/wasm.ts   — TypeScript wrapper around WASM module, typed API
```

### Modified Files

```
web/src/stores/graph.ts — Use WasmStore for detail lookups after initial server load
```

The `WasmGraphStore` TypeScript class wraps the WASM module and provides typed methods matching the existing `api.ts` signatures. The Svelte stores switch from API calls to WASM calls for node detail lookups once a snapshot is loaded.

## Build Tooling

`wasm-pack build` compiles `crates/wasm` to a `pkg/` directory containing `.wasm` + JS glue + TypeScript type declarations. The web frontend's Vite config imports the WASM package from a relative path (`../../crates/wasm/pkg`).

## Dependencies

| Crate | Version | Justification |
|-------|---------|---------------|
| `wasm-bindgen` | 0.2 | Rust/JS interop for WASM exports |
| `serde-wasm-bindgen` | 0.6 | Convert serde types to/from JsValue |
| `js-sys` | 0.3 | JavaScript type access from Rust |
| `svt-core` | path | Core graph model and CozoDB store |

## Testing Strategy

### Rust unit tests (`crates/wasm`)

Standard `cargo test` — test WasmStore methods with in-memory CozoDB. No WASM target needed for logic tests since the underlying CozoDB operations are the same.

### WASM integration tests

`wasm-pack test --headless --chrome` — verify actual WASM compilation and browser execution. Load a snapshot, query nodes/edges, check results.

### Web-side

Manual verification that browsing works after snapshot load. WASM mocking in vitest is fragile, so browser-level testing is preferred.

## Out of Scope

- No offline-first / service worker — server required for initial data
- No conformance in WASM — stays server-side
- No WASM-based analysis — tree-sitter stays server-side
- No changes to the server API — continues working as before
