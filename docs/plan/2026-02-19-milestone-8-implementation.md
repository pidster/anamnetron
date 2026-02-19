# WASM Bridge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Compile svt-core to WASM via a new `crates/wasm` bridge crate, enabling browser-side graph browsing without server round-trips.

**Architecture:** A new `svt-wasm` crate provides wasm-bindgen exports wrapping `CozoStore` in-memory. The web frontend fetches snapshot data from the server, loads it into browser-side CozoDB via WASM, then performs all graph queries locally. Data crosses the WASM boundary as JSON strings.

**Tech Stack:** wasm-bindgen, wasm-pack, serde-wasm-bindgen, CozoDB (in-memory mode, no SQLite), Vite WASM plugin

---

## Prerequisites

Install tooling before starting:

```bash
cargo install wasm-pack
rustup target add wasm32-unknown-unknown
```

## Important Notes

- CozoDB's `storage-sqlite` feature does NOT compile to WASM. The wasm crate must use CozoDB without it — the in-memory engine (`DbInstance::new("mem", ...)`) works without SQLite.
- svt-core currently depends on CozoDB with `features = ["storage-sqlite"]`. The wasm crate needs svt-core with `default-features = false` plus a new feature configuration that enables CozoDB without SQLite.
- All data crosses the WASM boundary as JSON strings (`&str` in, `String` out). Complex Rust types cannot be passed directly.
- The `Direction` enum in svt-core does NOT derive `Serialize`/`Deserialize`, so it must be handled as a string parameter in the WASM API.

---

### Task 1: Install WASM Tooling

**Files:**
- None (system setup only)

**Step 1: Install wasm-pack**

Run: `cargo install wasm-pack`
Expected: wasm-pack installed successfully

**Step 2: Add wasm32 target**

Run: `rustup target add wasm32-unknown-unknown`
Expected: target installed

**Step 3: Verify**

Run: `wasm-pack --version && rustup target list --installed | grep wasm`
Expected: version number and `wasm32-unknown-unknown`

---

### Task 2: Configure svt-core Features for WASM

**Files:**
- Modify: `crates/core/Cargo.toml`

The current `store` feature enables `dep:cozo` with `features = ["storage-sqlite"]`. WASM cannot use SQLite. We need CozoDB without SQLite for WASM, but WITH SQLite for native builds.

**Step 1: Update Cargo.toml features**

Change `crates/core/Cargo.toml` to split the CozoDB dependency:

```toml
[features]
default = ["store"]
store = ["dep:cozo", "dep:chrono"]
wasm = ["store", "dep:wasm-bindgen", "dep:serde-wasm-bindgen", "dep:js-sys", "cozo?/wasm"]

[dependencies]
cozo = { version = "0.7", features = ["storage-sqlite"], optional = true }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
glob-match = "0.2"
chrono = { version = "0.4", optional = true }
thiserror = "2"
uuid = { version = "1", features = ["v4"] }
wasm-bindgen = { version = "0.2", optional = true }
serde-wasm-bindgen = { version = "0.6", optional = true }
js-sys = { version = "0.3", optional = true }
```

Note: We keep `storage-sqlite` on the base `cozo` dependency because native builds need it. The wasm crate will override this — see Task 3.

**Step 2: Verify native build still works**

Run: `cargo build && cargo test -p svt-core --lib`
Expected: All tests pass, no regressions

**Step 3: Commit**

```bash
git add crates/core/Cargo.toml
git commit -m "chore(core): add serde-wasm-bindgen and js-sys optional dependencies"
```

---

### Task 3: Create the svt-wasm Crate Scaffold

**Files:**
- Create: `crates/wasm/Cargo.toml`
- Create: `crates/wasm/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

**Step 1: Create Cargo.toml**

Create `crates/wasm/Cargo.toml`:

```toml
[package]
name = "svt-wasm"
description = "WASM bridge to svt-core for browser-side graph queries"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
svt-core = { path = "../core", default-features = false, features = ["store"] }
cozo = { version = "0.7", default-features = false, features = ["wasm"] }
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde-wasm-bindgen = "0.6"
js-sys = "0.3"

[dev-dependencies]
wasm-bindgen-test = "0.3"
```

Key: `svt-core` with `default-features = false, features = ["store"]` — this pulls in CozoDB via svt-core's `store` feature, BUT we also add a direct `cozo` dependency with `default-features = false, features = ["wasm"]` which gives us CozoDB's in-memory engine without SQLite, plus the `uuid/js` and `js-sys` deps needed for WASM. Cargo's feature unification means svt-core's CozoDB and our direct CozoDB resolve to the same crate with the union of features. In WASM builds, SQLite will fail to compile if pulled in — we must verify this works.

**Step 2: Create minimal lib.rs**

Create `crates/wasm/src/lib.rs`:

```rust
//! WASM bridge to svt-core for browser-side graph queries.
//!
//! Provides a `WasmStore` class accessible from JavaScript that wraps
//! an in-memory CozoDB graph store. Load snapshot data from the server
//! API, then query nodes, edges, and hierarchy locally.

use wasm_bindgen::prelude::*;

/// WASM-accessible graph store wrapping CozoDB in-memory.
#[wasm_bindgen]
pub struct WasmStore {
    // Will be populated in later tasks
}

#[wasm_bindgen]
impl WasmStore {
    /// Create a new in-memory graph store.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmStore, JsError> {
        Ok(WasmStore {})
    }
}
```

**Step 3: Add to workspace**

Add `"crates/wasm"` to the workspace members in root `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/core",
    "crates/analyzer",
    "crates/cli",
    "crates/server",
    "crates/wasm",
]
```

**Step 4: Verify native build**

Run: `cargo build -p svt-wasm`
Expected: Compiles (though the struct is empty, it should build)

Note: We cannot `wasm-pack build` yet because we haven't resolved the SQLite/WASM conflict. That's addressed in the WASM compilation step later.

**Step 5: Commit**

```bash
git add crates/wasm/Cargo.toml crates/wasm/src/lib.rs Cargo.toml
git commit -m "feat(wasm): scaffold svt-wasm crate with wasm-bindgen"
```

---

### Task 4: Implement WasmStore Core (new, load_snapshot)

**Files:**
- Modify: `crates/wasm/src/lib.rs`

**Step 1: Write tests for new() and load_snapshot()**

Add to `crates/wasm/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_store_new_succeeds() {
        let store = WasmStore::new().unwrap();
        assert!(store.store.list_snapshots().unwrap().is_empty());
    }

    #[test]
    fn load_snapshot_creates_version() {
        let mut store = WasmStore::new().unwrap();
        let nodes_json = serde_json::json!([
            {
                "id": "n1",
                "canonical_path": "/svc",
                "qualified_name": null,
                "kind": "service",
                "sub_kind": "crate",
                "name": "svc",
                "language": null,
                "provenance": "design",
                "source_ref": null,
                "metadata": null
            }
        ]);
        let edges_json = serde_json::json!([]);
        let version = store.load_snapshot(
            &nodes_json.to_string(),
            &edges_json.to_string(),
        ).unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn load_snapshot_with_edges() {
        let mut store = WasmStore::new().unwrap();
        let nodes_json = serde_json::json!([
            {"id": "n1", "canonical_path": "/a", "qualified_name": null, "kind": "service", "sub_kind": "crate", "name": "a", "language": null, "provenance": "design", "source_ref": null, "metadata": null},
            {"id": "n2", "canonical_path": "/b", "qualified_name": null, "kind": "service", "sub_kind": "crate", "name": "b", "language": null, "provenance": "design", "source_ref": null, "metadata": null}
        ]);
        let edges_json = serde_json::json!([
            {"id": "e1", "source": "n1", "target": "n2", "kind": "depends", "provenance": "design", "metadata": null}
        ]);
        let version = store.load_snapshot(
            &nodes_json.to_string(),
            &edges_json.to_string(),
        ).unwrap();
        assert_eq!(version, 1);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-wasm`
Expected: FAIL (WasmStore has no `store` field or `load_snapshot` method)

**Step 3: Implement WasmStore**

Replace the full `crates/wasm/src/lib.rs`:

```rust
//! WASM bridge to svt-core for browser-side graph queries.
//!
//! Provides a `WasmStore` class accessible from JavaScript that wraps
//! an in-memory CozoDB graph store. Load snapshot data from the server
//! API, then query nodes, edges, and hierarchy locally.

use wasm_bindgen::prelude::*;

use svt_core::model::{Edge, Node, SnapshotKind, Version};
use svt_core::store::{CozoStore, GraphStore};

/// WASM-accessible graph store wrapping CozoDB in-memory.
#[wasm_bindgen]
pub struct WasmStore {
    store: CozoStore,
}

#[wasm_bindgen]
impl WasmStore {
    /// Create a new in-memory graph store.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmStore, JsError> {
        let store =
            CozoStore::new_in_memory().map_err(|e| JsError::new(&format!("store error: {e}")))?;
        Ok(WasmStore { store })
    }

    /// Load a snapshot from JSON arrays of nodes and edges.
    ///
    /// The JSON format matches the server API response from
    /// `/api/snapshots/{v}/nodes` and `/api/snapshots/{v}/edges`.
    ///
    /// Returns the version number of the created snapshot.
    pub fn load_snapshot(
        &mut self,
        nodes_json: &str,
        edges_json: &str,
    ) -> Result<u64, JsError> {
        let nodes: Vec<Node> = serde_json::from_str(nodes_json)
            .map_err(|e| JsError::new(&format!("invalid nodes JSON: {e}")))?;
        let edges: Vec<Edge> = serde_json::from_str(edges_json)
            .map_err(|e| JsError::new(&format!("invalid edges JSON: {e}")))?;

        let version = self
            .store
            .create_snapshot(SnapshotKind::Import, None)
            .map_err(|e| JsError::new(&format!("snapshot error: {e}")))?;

        self.store
            .add_nodes_batch(version, &nodes)
            .map_err(|e| JsError::new(&format!("node insert error: {e}")))?;

        self.store
            .add_edges_batch(version, &edges)
            .map_err(|e| JsError::new(&format!("edge insert error: {e}")))?;

        Ok(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_store_new_succeeds() {
        let store = WasmStore::new().unwrap();
        assert!(store.store.list_snapshots().unwrap().is_empty());
    }

    #[test]
    fn load_snapshot_creates_version() {
        let mut store = WasmStore::new().unwrap();
        let nodes_json = serde_json::json!([
            {
                "id": "n1",
                "canonical_path": "/svc",
                "qualified_name": null,
                "kind": "service",
                "sub_kind": "crate",
                "name": "svc",
                "language": null,
                "provenance": "design",
                "source_ref": null,
                "metadata": null
            }
        ]);
        let edges_json = serde_json::json!([]);
        let version = store
            .load_snapshot(&nodes_json.to_string(), &edges_json.to_string())
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn load_snapshot_with_edges() {
        let mut store = WasmStore::new().unwrap();
        let nodes_json = serde_json::json!([
            {"id": "n1", "canonical_path": "/a", "qualified_name": null, "kind": "service", "sub_kind": "crate", "name": "a", "language": null, "provenance": "design", "source_ref": null, "metadata": null},
            {"id": "n2", "canonical_path": "/b", "qualified_name": null, "kind": "service", "sub_kind": "crate", "name": "b", "language": null, "provenance": "design", "source_ref": null, "metadata": null}
        ]);
        let edges_json = serde_json::json!([
            {"id": "e1", "source": "n1", "target": "n2", "kind": "depends", "provenance": "design", "metadata": null}
        ]);
        let version = store
            .load_snapshot(&nodes_json.to_string(), &edges_json.to_string())
            .unwrap();
        assert_eq!(version, 1);
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p svt-wasm`
Expected: 3 tests pass

**Step 5: Commit**

```bash
git add crates/wasm/src/lib.rs
git commit -m "feat(wasm): implement WasmStore::new() and load_snapshot()"
```

---

### Task 5: Add Read-Only Query Methods

**Files:**
- Modify: `crates/wasm/src/lib.rs`

**Step 1: Write tests for query methods**

Add these tests to the `tests` module:

```rust
    fn make_test_store() -> WasmStore {
        let mut store = WasmStore::new().unwrap();
        let nodes_json = serde_json::json!([
            {"id": "n1", "canonical_path": "/app", "qualified_name": null, "kind": "system", "sub_kind": "system", "name": "app", "language": null, "provenance": "design", "source_ref": null, "metadata": null},
            {"id": "n2", "canonical_path": "/app/core", "qualified_name": null, "kind": "service", "sub_kind": "crate", "name": "core", "language": "rust", "provenance": "design", "source_ref": null, "metadata": null},
            {"id": "n3", "canonical_path": "/app/cli", "qualified_name": null, "kind": "service", "sub_kind": "crate", "name": "cli", "language": "rust", "provenance": "design", "source_ref": null, "metadata": null}
        ]);
        let edges_json = serde_json::json!([
            {"id": "e1", "source": "n1", "target": "n2", "kind": "contains", "provenance": "design", "metadata": null},
            {"id": "e2", "source": "n1", "target": "n3", "kind": "contains", "provenance": "design", "metadata": null},
            {"id": "e3", "source": "n3", "target": "n2", "kind": "depends", "provenance": "design", "metadata": null}
        ]);
        store.load_snapshot(&nodes_json.to_string(), &edges_json.to_string()).unwrap();
        store
    }

    #[test]
    fn get_node_returns_json() {
        let store = make_test_store();
        let result = store.get_node(1, "n1").unwrap();
        let node: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(node["canonical_path"], "/app");
    }

    #[test]
    fn get_node_by_path_returns_json() {
        let store = make_test_store();
        let result = store.get_node_by_path(1, "/app/core").unwrap();
        let node: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(node["name"], "core");
    }

    #[test]
    fn get_node_by_path_returns_null_for_missing() {
        let store = make_test_store();
        let result = store.get_node_by_path(1, "/nonexistent").unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn get_all_nodes_returns_array() {
        let store = make_test_store();
        let result = store.get_all_nodes(1).unwrap();
        let nodes: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn get_children_returns_child_nodes() {
        let store = make_test_store();
        let result = store.get_children(1, "n1").unwrap();
        let children: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn get_parent_returns_parent_node() {
        let store = make_test_store();
        let result = store.get_parent(1, "n2").unwrap();
        let parent: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parent["canonical_path"], "/app");
    }

    #[test]
    fn get_dependencies_returns_depends_targets() {
        let store = make_test_store();
        let result = store.get_dependencies(1, "n3", false).unwrap();
        let deps: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0]["canonical_path"], "/app/core");
    }

    #[test]
    fn get_edges_returns_filtered() {
        let store = make_test_store();
        let result = store.get_all_edges(1, Some("contains".to_string())).unwrap();
        let edges: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn search_returns_matching_nodes() {
        let store = make_test_store();
        let result = store.search(1, "/app/**").unwrap();
        let nodes: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert!(nodes.len() >= 2); // /app/core and /app/cli (and possibly /app itself)
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-wasm`
Expected: FAIL (methods don't exist yet)

**Step 3: Implement query methods**

Add these methods to the `#[wasm_bindgen] impl WasmStore` block:

```rust
    /// Get a node by ID. Returns JSON string or null.
    pub fn get_node(&self, version: u64, id: &str) -> Result<String, JsError> {
        let node = self
            .store
            .get_node(version, &id.to_string())
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&node).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get a node by canonical path. Returns JSON string or null.
    pub fn get_node_by_path(&self, version: u64, path: &str) -> Result<String, JsError> {
        let node = self
            .store
            .get_node_by_path(version, path)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&node).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get all nodes for a version. Returns JSON array string.
    pub fn get_all_nodes(&self, version: u64) -> Result<String, JsError> {
        let nodes = self
            .store
            .get_all_nodes(version)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&nodes).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get children of a node. Returns JSON array string.
    pub fn get_children(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let children = self
            .store
            .get_children(version, &node_id.to_string())
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&children)
            .map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get parent of a node. Returns JSON string or null.
    pub fn get_parent(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let parent = self
            .store
            .get_parent(version, &node_id.to_string())
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&parent).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get ancestors of a node (parent to root). Returns JSON array string.
    pub fn get_ancestors(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let ancestors = self
            .store
            .query_ancestors(version, &node_id.to_string())
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&ancestors)
            .map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get descendants of a node. Returns JSON array string.
    pub fn get_descendants(&self, version: u64, node_id: &str) -> Result<String, JsError> {
        let descendants = self
            .store
            .query_descendants(version, &node_id.to_string(), None)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&descendants)
            .map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get edges for a node. Direction: "outgoing", "incoming", or "both".
    /// Kind is optional (e.g., "depends", "contains").
    pub fn get_edges(
        &self,
        version: u64,
        node_id: &str,
        direction: &str,
        kind: Option<String>,
    ) -> Result<String, JsError> {
        use svt_core::model::{Direction, EdgeKind};

        let dir = match direction {
            "outgoing" => Direction::Outgoing,
            "incoming" => Direction::Incoming,
            "both" => Direction::Both,
            _ => return Err(JsError::new(&format!("invalid direction: {direction}"))),
        };

        let edge_kind = kind
            .as_deref()
            .map(|k| {
                serde_json::from_value::<EdgeKind>(serde_json::Value::String(k.to_string()))
            })
            .transpose()
            .map_err(|e| JsError::new(&format!("invalid edge kind: {e}")))?;

        let edges = self
            .store
            .get_edges(version, &node_id.to_string(), dir, edge_kind)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&edges).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get all edges for a version, optionally filtered by kind. Returns JSON array string.
    pub fn get_all_edges(
        &self,
        version: u64,
        kind: Option<String>,
    ) -> Result<String, JsError> {
        use svt_core::model::EdgeKind;

        let edge_kind = kind
            .as_deref()
            .map(|k| {
                serde_json::from_value::<EdgeKind>(serde_json::Value::String(k.to_string()))
            })
            .transpose()
            .map_err(|e| JsError::new(&format!("invalid edge kind: {e}")))?;

        let edges = self
            .store
            .get_all_edges(version, edge_kind)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&edges).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get dependencies of a node. Returns JSON array of nodes.
    pub fn get_dependencies(
        &self,
        version: u64,
        node_id: &str,
        transitive: bool,
    ) -> Result<String, JsError> {
        let deps = self
            .store
            .query_dependencies(version, &node_id.to_string(), transitive)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&deps).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Get dependents of a node. Returns JSON array of nodes.
    pub fn get_dependents(
        &self,
        version: u64,
        node_id: &str,
        transitive: bool,
    ) -> Result<String, JsError> {
        let deps = self
            .store
            .query_dependents(version, &node_id.to_string(), transitive)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        serde_json::to_string(&deps).map_err(|e| JsError::new(&format!("json error: {e}")))
    }

    /// Search nodes by glob pattern on canonical path. Returns JSON array of nodes.
    pub fn search(&self, version: u64, pattern: &str) -> Result<String, JsError> {
        use svt_core::canonical::canonical_path_matches;

        let all_nodes = self
            .store
            .get_all_nodes(version)
            .map_err(|e| JsError::new(&format!("store error: {e}")))?;
        let matched: Vec<_> = all_nodes
            .into_iter()
            .filter(|n| canonical_path_matches(&n.canonical_path, pattern))
            .collect();
        serde_json::to_string(&matched)
            .map_err(|e| JsError::new(&format!("json error: {e}")))
    }
```

**Step 4: Run tests**

Run: `cargo test -p svt-wasm`
Expected: All 12 tests pass

**Step 5: Commit**

```bash
git add crates/wasm/src/lib.rs
git commit -m "feat(wasm): add read-only query methods to WasmStore"
```

---

### Task 6: WASM Compilation Verification

**Files:**
- Possibly modify: `crates/core/Cargo.toml` (if feature conflicts arise)

This task verifies that `svt-wasm` actually compiles to WASM. CozoDB's `storage-sqlite` feature will fail on `wasm32-unknown-unknown`. We need to ensure the dependency resolution excludes SQLite for the WASM target.

**Step 1: Attempt wasm-pack build**

Run: `cd crates/wasm && wasm-pack build --target web --out-dir pkg`
Expected: Either succeeds (CozoDB feature unification works) or fails with SQLite compilation errors.

**Step 2: If SQLite fails, fix feature configuration**

If `storage-sqlite` is pulled in, we need to adjust. The fix is to ensure svt-core does NOT include `storage-sqlite` when compiled for WASM. Options:

Option A: In `crates/core/Cargo.toml`, split the cozo dependency:
```toml
[dependencies]
cozo = { version = "0.7", optional = true, default-features = false }

[features]
store = ["dep:cozo", "dep:chrono"]
store-sqlite = ["store", "cozo?/storage-sqlite"]
default = ["store-sqlite"]
```

Then `svt-wasm/Cargo.toml` uses `svt-core` with `default-features = false, features = ["store"]` which gets CozoDB without SQLite.

The other crates (`cli`, `server`, `analyzer`) continue using `svt-core` with default features which include `store-sqlite`.

Option B: If CozoDB in-memory works without any storage feature, the `crates/wasm/Cargo.toml` direct dependency on `cozo` with `features = ["wasm"]` may be sufficient and override the sqlite feature.

Try the simpler option first, escalate if needed.

**Step 3: Verify wasm-pack build succeeds**

Run: `cd crates/wasm && wasm-pack build --target web --out-dir pkg`
Expected: Build succeeds, produces `pkg/` with `.wasm`, `.js`, `.d.ts` files

**Step 4: Verify native workspace still builds**

Run: `cargo build && cargo test`
Expected: All existing tests still pass (no regressions from feature changes)

**Step 5: Commit any feature configuration changes**

```bash
git add crates/core/Cargo.toml crates/wasm/Cargo.toml
git commit -m "fix(wasm): resolve CozoDB feature configuration for WASM target"
```

---

### Task 7: TypeScript WASM Wrapper

**Files:**
- Create: `web/src/lib/wasm.ts`

**Step 1: Create TypeScript wrapper**

Create `web/src/lib/wasm.ts`:

```typescript
import type { ApiNode, ApiEdge, Version } from "./types";

// The WASM module will be imported at runtime
let wasmModule: typeof import("../../crates/wasm/pkg") | null = null;
let wasmStore: any | null = null;
let loadedVersion: Version | null = null;

/** Initialize the WASM module. Call once on app startup. */
export async function initWasm(): Promise<void> {
  try {
    wasmModule = await import("../../crates/wasm/pkg");
    // wasm-pack with --target web requires manual init
    if (wasmModule.default && typeof wasmModule.default === "function") {
      await wasmModule.default();
    }
  } catch (e) {
    console.warn("WASM module not available, falling back to API:", e);
  }
}

/** Whether the WASM module is loaded and ready. */
export function isWasmReady(): boolean {
  return wasmModule !== null;
}

/** Whether a snapshot has been loaded into WASM store. */
export function isSnapshotLoaded(): boolean {
  return wasmStore !== null && loadedVersion !== null;
}

/** Get the currently loaded WASM version, if any. */
export function getLoadedVersion(): Version | null {
  return loadedVersion;
}

/** Load a snapshot into the WASM store from server API data. */
export async function loadSnapshot(
  nodes: ApiNode[],
  edges: ApiEdge[],
): Promise<Version> {
  if (!wasmModule) {
    throw new Error("WASM module not initialized");
  }
  wasmStore = new wasmModule.WasmStore();
  const version = wasmStore.load_snapshot(
    JSON.stringify(nodes),
    JSON.stringify(edges),
  );
  loadedVersion = version;
  return version;
}

/** Get a node by ID from the WASM store. */
export function getNode(id: string): ApiNode | null {
  if (!wasmStore || loadedVersion === null) return null;
  const json = wasmStore.get_node(loadedVersion, id);
  return JSON.parse(json);
}

/** Get a node by canonical path from the WASM store. */
export function getNodeByPath(path: string): ApiNode | null {
  if (!wasmStore || loadedVersion === null) return null;
  const json = wasmStore.get_node_by_path(loadedVersion, path);
  return JSON.parse(json);
}

/** Get all nodes from the WASM store. */
export function getAllNodes(): ApiNode[] {
  if (!wasmStore || loadedVersion === null) return [];
  const json = wasmStore.get_all_nodes(loadedVersion);
  return JSON.parse(json);
}

/** Get children of a node. */
export function getChildren(nodeId: string): ApiNode[] {
  if (!wasmStore || loadedVersion === null) return [];
  const json = wasmStore.get_children(loadedVersion, nodeId);
  return JSON.parse(json);
}

/** Get parent of a node. */
export function getParent(nodeId: string): ApiNode | null {
  if (!wasmStore || loadedVersion === null) return null;
  const json = wasmStore.get_parent(loadedVersion, nodeId);
  return JSON.parse(json);
}

/** Get ancestors of a node (parent to root). */
export function getAncestors(nodeId: string): ApiNode[] {
  if (!wasmStore || loadedVersion === null) return [];
  const json = wasmStore.get_ancestors(loadedVersion, nodeId);
  return JSON.parse(json);
}

/** Get descendants of a node. */
export function getDescendants(nodeId: string): ApiNode[] {
  if (!wasmStore || loadedVersion === null) return [];
  const json = wasmStore.get_descendants(loadedVersion, nodeId);
  return JSON.parse(json);
}

/** Get dependencies of a node. */
export function getDependencies(
  nodeId: string,
  transitive = false,
): ApiNode[] {
  if (!wasmStore || loadedVersion === null) return [];
  const json = wasmStore.get_dependencies(loadedVersion, nodeId, transitive);
  return JSON.parse(json);
}

/** Get dependents of a node. */
export function getDependents(
  nodeId: string,
  transitive = false,
): ApiNode[] {
  if (!wasmStore || loadedVersion === null) return [];
  const json = wasmStore.get_dependents(loadedVersion, nodeId, transitive);
  return JSON.parse(json);
}

/** Search nodes by glob pattern. */
export function searchNodes(pattern: string): ApiNode[] {
  if (!wasmStore || loadedVersion === null) return [];
  const json = wasmStore.search(loadedVersion, pattern);
  return JSON.parse(json);
}
```

**Step 2: Commit**

```bash
git add web/src/lib/wasm.ts
git commit -m "feat(web): add TypeScript WASM wrapper with typed API"
```

---

### Task 8: Integrate WASM into Web Frontend

**Files:**
- Modify: `web/src/App.svelte` (init WASM on startup)
- Modify: `web/vite.config.ts` (WASM plugin if needed)

This task wires the WASM module into the frontend. The integration is lightweight — WASM is initialized on startup and made available for future use by components. The actual component-level switchover (using WASM for detail lookups instead of API calls) is left as a follow-up since it requires careful testing with the real UI.

**Step 1: Check current App.svelte**

Read `web/src/App.svelte` to understand the current initialization flow.

**Step 2: Add WASM initialization**

Import and call `initWasm()` in the app's initialization path (e.g., in an `onMount` or top-level `$effect`):

```typescript
import { initWasm, isWasmReady } from "./lib/wasm";
import { onMount } from "svelte";

onMount(async () => {
  await initWasm();
  console.log("WASM ready:", isWasmReady());
});
```

**Step 3: Configure Vite for WASM**

Check if Vite needs configuration to handle `.wasm` imports. If `wasm-pack build --target web` produces ES module output, Vite may handle it natively. If not, add `vite-plugin-wasm`:

```bash
cd web && npm install -D vite-plugin-wasm
```

And update `vite.config.ts`:

```typescript
import wasm from "vite-plugin-wasm";

export default defineConfig({
  plugins: [svelte(), wasm()],
  // ...
});
```

**Step 4: Verify the web app still builds**

Run: `cd web && npm run build`
Expected: Builds successfully (WASM import is lazy/dynamic, so it's OK if the .wasm file isn't present yet)

**Step 5: Commit**

```bash
git add web/src/App.svelte web/vite.config.ts web/package.json web/package-lock.json
git commit -m "feat(web): integrate WASM initialization into app startup"
```

---

### Task 9: End-to-End Verification

**Files:**
- None (testing only)

**Step 1: Build WASM package**

Run: `cd crates/wasm && wasm-pack build --target web --out-dir pkg`
Expected: Produces `pkg/svt_wasm.js`, `pkg/svt_wasm_bg.wasm`, `pkg/svt_wasm.d.ts`

**Step 2: Build web app**

Run: `cd web && npm run build`
Expected: Builds successfully with WASM module linked

**Step 3: Run full Rust test suite**

Run: `cargo test`
Expected: All tests pass (existing + new svt-wasm tests)

**Step 4: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean

**Step 5: Manual browser test**

Start the server and load the web UI to verify WASM initializes:

Run: `cargo run -p svt-server -- --design design/architecture.yaml --project .`
Open: `http://localhost:3000`
Check browser console for: `WASM ready: true` (or `false` if WASM pkg isn't served — this is OK for now)

---

### Task 10: Update PROGRESS.md

**Files:**
- Modify: `docs/plan/PROGRESS.md`

**Step 1: Update milestone table**

Add Milestone 8 to the completed milestones table with test count, key changes.

**Step 2: Update "What's Working Now"**

Note that WASM bridge is available for browser-side queries.

**Step 3: Update "Not Yet Built"**

Remove WASM bridge from the list. Update suggested next milestones.

**Step 4: Update plan documents table**

Add M8 design and implementation plan entries.

**Step 5: Commit**

```bash
git add docs/plan/PROGRESS.md
git commit -m "docs: mark milestone 8 as complete"
```
