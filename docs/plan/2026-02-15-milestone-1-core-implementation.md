# Milestone 1: Core Data Model + CozoDB Store — Implementation Plan

## Status: COMPLETE

Completed: 2026-02-15

---

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement `svt-core` with Rust type definitions, `GraphStore` trait, and CozoDB backend supporting write, read, and basic traversal operations.

**Architecture:** Vertical slices through seven features. Each slice adds types to `model.rs`, methods to the `GraphStore` trait, CozoDB implementation in `store/cozo.rs`, and tests. The trait is defined in `store/mod.rs` and the CozoDB backend implements it.

**Tech Stack:** Rust 2021, CozoDB 0.7 (embedded, SQLite storage), serde/serde_json, thiserror, proptest, uuid.

**Design doc:** `docs/plan/2026-02-15-milestone-1-core-data-model-design.md`

**Data model reference:** `docs/design/DATA_MODEL.md`

---

### Task 0: Add uuid dependency

**Files:**
- Modify: `crates/core/Cargo.toml`

**Step 1: Add uuid to dependencies**

Add `uuid = { version = "1", features = ["v4"] }` to `[dependencies]` in `crates/core/Cargo.toml`, after the `thiserror` line.

**Step 2: Verify it compiles**

Run: `cargo check -p svt-core`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/core/Cargo.toml
git commit -m "Add uuid dependency to svt-core"
```

---

### Task 1: Core types — enums and type aliases

**Files:**
- Create: `crates/core/src/model/mod.rs`
- Modify: `crates/core/src/lib.rs` (update module declaration)

**Step 1: Create model module directory**

Replace the empty `pub mod model {}` in `lib.rs` with `pub mod model;` and create `crates/core/src/model/mod.rs`.

The model module re-exports all public types.

**Step 2: Write the types**

In `crates/core/src/model/mod.rs`:

```rust
//! Core data model types for the software visualizer graph.

use serde::{Deserialize, Serialize};

/// Snapshot version number. Monotonically increasing.
pub type Version = u64;

/// Unique identifier for a node (UUID v4).
pub type NodeId = String;

/// Unique identifier for an edge (UUID v4).
pub type EdgeId = String;

/// Abstraction level of a node in the architecture hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// Top-level boundary (workspace, monorepo, solution).
    System,
    /// Deployable or distributable unit (crate, package, assembly).
    Service,
    /// Logical grouping within a service (module, namespace, package).
    Component,
    /// Individual code element (class, struct, function, trait).
    Unit,
}

/// Relationship type between nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Hierarchical nesting (parent contains child).
    Contains,
    /// Import/use dependency.
    Depends,
    /// Runtime invocation.
    Calls,
    /// Fulfills a contract (trait, interface, protocol).
    Implements,
    /// Inheritance relationship.
    Extends,
    /// Data movement between elements.
    DataFlow,
    /// Public visibility boundary.
    Exports,
}

/// Origin of a piece of knowledge in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Human-authored, prescriptive.
    Design,
    /// Machine-derived from code analysis.
    Analysis,
    /// Ingested from an external knowledge source.
    Import,
    /// Derived from heuristics or patterns.
    Inferred,
}

/// Type of snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    /// Design model snapshot.
    Design,
    /// Code analysis snapshot.
    Analysis,
    /// External import snapshot.
    Import,
}

/// Severity level for constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Conformance check fails.
    Error,
    /// Reported but does not fail.
    Warning,
    /// Informational only.
    Info,
}

/// Direction for edge queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Edges where the node is the source.
    Outgoing,
    /// Edges where the node is the target.
    Incoming,
    /// Edges in either direction.
    Both,
}
```

**Step 3: Write tests for enum serialisation**

At the bottom of `crates/core/src/model/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_serialises_to_snake_case() {
        assert_eq!(serde_json::to_string(&NodeKind::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&NodeKind::DataFlow).unwrap_err().to_string().contains("error"), false);
    }

    #[test]
    fn edge_kind_round_trips_through_json() {
        for kind in [EdgeKind::Contains, EdgeKind::Depends, EdgeKind::Calls,
                     EdgeKind::Implements, EdgeKind::Extends, EdgeKind::DataFlow,
                     EdgeKind::Exports] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: EdgeKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn snapshot_kind_round_trips_through_json() {
        for kind in [SnapshotKind::Design, SnapshotKind::Analysis, SnapshotKind::Import] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: SnapshotKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p svt-core`
Expected: all tests pass

**Step 5: Commit**

```bash
git add crates/core/src/model/mod.rs crates/core/src/lib.rs
git commit -m "Add core enums and type aliases"
```

---

### Task 2: Core types — structs (Node, Edge, Constraint, Snapshot, NodeFilter)

**Files:**
- Modify: `crates/core/src/model/mod.rs`

**Step 1: Add struct definitions**

Append to `crates/core/src/model/mod.rs`, before the `#[cfg(test)]` module:

```rust
/// A node in the architecture graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Internal unique identifier (UUID v4).
    pub id: NodeId,
    /// Language-neutral path derived from containment hierarchy.
    pub canonical_path: String,
    /// Language-specific qualified name (null for design nodes).
    pub qualified_name: Option<String>,
    /// Abstraction level.
    pub kind: NodeKind,
    /// Language-specific or domain-specific type (e.g., "crate", "class", "trait").
    pub sub_kind: String,
    /// Human-readable name (last segment of canonical path).
    pub name: String,
    /// Source language, if derived from code analysis.
    pub language: Option<String>,
    /// Origin of this knowledge.
    pub provenance: Provenance,
    /// File path, line number, or external URL.
    pub source_ref: Option<String>,
    /// Extensible key-value properties.
    pub metadata: Option<serde_json::Value>,
}

/// An edge (relationship) in the architecture graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Unique identifier.
    pub id: EdgeId,
    /// Source node ID.
    pub source: NodeId,
    /// Target node ID.
    pub target: NodeId,
    /// Relationship type.
    pub kind: EdgeKind,
    /// Origin of this knowledge.
    pub provenance: Provenance,
    /// Extensible key-value properties.
    pub metadata: Option<serde_json::Value>,
}

/// An architectural constraint (design-mode assertion).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Unique identifier.
    pub id: String,
    /// Constraint type (e.g., "must_not_depend", "boundary"). String for extensibility.
    pub kind: String,
    /// Human-readable name.
    pub name: String,
    /// Canonical path pattern this constraint applies to (supports glob).
    pub scope: String,
    /// Target path pattern (for dependency constraints).
    pub target: Option<String>,
    /// Additional parameters.
    pub params: Option<serde_json::Value>,
    /// Description shown on violation.
    pub message: String,
    /// Severity level.
    pub severity: Severity,
}

/// A versioned snapshot of the graph state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Monotonically increasing version number.
    pub version: Version,
    /// Type of snapshot.
    pub kind: SnapshotKind,
    /// Git commit hash, if applicable.
    pub commit_ref: Option<String>,
    /// Timestamp (informational, not used for ordering).
    pub created_at: String,
    /// Additional context.
    pub metadata: Option<serde_json::Value>,
}

/// Filter criteria for node queries.
#[derive(Debug, Clone, Default)]
pub struct NodeFilter {
    /// Filter by abstraction level.
    pub kind: Option<NodeKind>,
    /// Filter by language-specific type.
    pub sub_kind: Option<String>,
    /// Filter by source language.
    pub language: Option<String>,
}
```

**Step 2: Add struct tests**

Add to the existing `#[cfg(test)]` module:

```rust
    #[test]
    fn node_round_trips_through_json() {
        let node = Node {
            id: "test-id".to_string(),
            canonical_path: "/test-service/handlers/create".to_string(),
            qualified_name: Some("test_service::handlers::create".to_string()),
            kind: NodeKind::Unit,
            sub_kind: "function".to_string(),
            name: "create".to_string(),
            language: Some("rust".to_string()),
            provenance: Provenance::Analysis,
            source_ref: Some("src/handlers.rs:42".to_string()),
            metadata: Some(serde_json::json!({"is_async": true})),
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, node.id);
        assert_eq!(back.canonical_path, node.canonical_path);
        assert_eq!(back.kind, node.kind);
        assert_eq!(back.qualified_name, node.qualified_name);
    }

    #[test]
    fn node_with_none_fields_round_trips() {
        let node = Node {
            id: "test-id".to_string(),
            canonical_path: "/design-service".to_string(),
            qualified_name: None,
            kind: NodeKind::Service,
            sub_kind: "crate".to_string(),
            name: "design-service".to_string(),
            language: None,
            provenance: Provenance::Design,
            source_ref: None,
            metadata: None,
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(back.qualified_name, None);
        assert_eq!(back.language, None);
        assert_eq!(back.metadata, None);
    }

    #[test]
    fn edge_round_trips_through_json() {
        let edge = Edge {
            id: "edge-1".to_string(),
            source: "node-a".to_string(),
            target: "node-b".to_string(),
            kind: EdgeKind::Depends,
            provenance: Provenance::Analysis,
            metadata: None,
        };
        let json = serde_json::to_string(&edge).unwrap();
        let back: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, edge.source);
        assert_eq!(back.kind, edge.kind);
    }

    #[test]
    fn constraint_round_trips_through_json() {
        let constraint = Constraint {
            id: "c-1".to_string(),
            kind: "must_not_depend".to_string(),
            name: "no-internal-access".to_string(),
            scope: "/payments/**".to_string(),
            target: Some("/user-service/internal/**".to_string()),
            params: None,
            message: "Payment must not access user internals".to_string(),
            severity: Severity::Error,
        };
        let json = serde_json::to_string(&constraint).unwrap();
        let back: Constraint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.kind, "must_not_depend");
        assert_eq!(back.severity, Severity::Error);
    }
```

**Step 3: Run tests**

Run: `cargo test -p svt-core`
Expected: all tests pass

**Step 4: Commit**

```bash
git add crates/core/src/model/mod.rs
git commit -m "Add core structs: Node, Edge, Constraint, Snapshot, NodeFilter"
```

---

### Task 3: Error type and GraphStore trait (snapshot methods only)

**Files:**
- Create: `crates/core/src/store/mod.rs`
- Create: `crates/core/src/store/error.rs`
- Modify: `crates/core/src/lib.rs` (update module declaration)

**Step 1: Create store module structure**

Replace the empty `pub mod store {}` in `lib.rs` with `pub mod store;`.

Create `crates/core/src/store/mod.rs`:

```rust
//! Graph store trait and backend implementations.

mod error;

pub use error::StoreError;

use crate::model::*;

/// Result type for graph store operations.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Abstract interface for the graph store.
///
/// All operations require an explicit version — there is no implicit
/// "current version". This keeps the store stateless and makes
/// conformance (comparing two versions) natural.
pub trait GraphStore {
    /// Create a new snapshot and return its version number.
    fn create_snapshot(
        &mut self,
        kind: SnapshotKind,
        commit_ref: Option<&str>,
    ) -> Result<Version>;

    /// List all snapshots in version order.
    fn list_snapshots(&self) -> Result<Vec<Snapshot>>;

    /// Get the latest version for a given snapshot kind, or None if no snapshots exist.
    fn latest_version(&self, kind: SnapshotKind) -> Result<Option<Version>>;
}
```

Create `crates/core/src/store/error.rs`:

```rust
//! Error types for graph store operations.

use crate::model::{NodeId, Version};

/// Errors that can occur during graph store operations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A node was not found.
    #[error("node not found: {0}")]
    NodeNotFound(NodeId),

    /// A version was not found.
    #[error("version not found: {0}")]
    VersionNotFound(Version),

    /// Attempted to add a node with a duplicate ID.
    #[error("duplicate node: {0}")]
    DuplicateNode(String),

    /// Attempted to add an edge with a duplicate ID.
    #[error("duplicate edge: {0}")]
    DuplicateEdge(String),

    /// An edge references a node that does not exist.
    #[error("invalid reference: edge {edge_id} references unknown node {node_id}")]
    InvalidReference {
        /// The edge with the invalid reference.
        edge_id: String,
        /// The node ID that was not found.
        node_id: String,
    },

    /// An internal store error.
    #[error("store error: {0}")]
    Internal(String),
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p svt-core`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add crates/core/src/store/mod.rs crates/core/src/store/error.rs crates/core/src/lib.rs
git commit -m "Add StoreError and GraphStore trait with snapshot methods"
```

---

### Task 4: CozoStore — struct, constructors, schema init, snapshot implementation

**Files:**
- Create: `crates/core/src/store/cozo.rs`
- Modify: `crates/core/src/store/mod.rs` (add module + re-export)

**Step 1: Write the CozoStore snapshot tests first**

Create `crates/core/tests/snapshot_tests.rs`:

```rust
use svt_core::model::SnapshotKind;
use svt_core::store::{CozoStore, GraphStore};

#[test]
fn create_snapshot_returns_version_one() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let version = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    assert_eq!(version, 1);
}

#[test]
fn create_second_snapshot_returns_version_two() {
    let mut store = CozoStore::new_in_memory().unwrap();
    store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    let v2 = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    assert_eq!(v2, 2);
}

#[test]
fn latest_version_returns_none_for_empty_store() {
    let store = CozoStore::new_in_memory().unwrap();
    let latest = store.latest_version(SnapshotKind::Analysis).unwrap();
    assert_eq!(latest, None);
}

#[test]
fn latest_version_filters_by_kind() {
    let mut store = CozoStore::new_in_memory().unwrap();
    store.create_snapshot(SnapshotKind::Design, None).unwrap();
    store.create_snapshot(SnapshotKind::Analysis, None).unwrap();
    store.create_snapshot(SnapshotKind::Design, None).unwrap();

    assert_eq!(store.latest_version(SnapshotKind::Design).unwrap(), Some(3));
    assert_eq!(store.latest_version(SnapshotKind::Analysis).unwrap(), Some(2));
    assert_eq!(store.latest_version(SnapshotKind::Import).unwrap(), None);
}

#[test]
fn list_snapshots_returns_all_in_version_order() {
    let mut store = CozoStore::new_in_memory().unwrap();
    store.create_snapshot(SnapshotKind::Design, Some("abc123")).unwrap();
    store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let snapshots = store.list_snapshots().unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].version, 1);
    assert_eq!(snapshots[0].kind, SnapshotKind::Design);
    assert_eq!(snapshots[0].commit_ref.as_deref(), Some("abc123"));
    assert_eq!(snapshots[1].version, 2);
    assert_eq!(snapshots[1].kind, SnapshotKind::Analysis);
    assert_eq!(snapshots[1].commit_ref, None);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core --test snapshot_tests`
Expected: FAIL — `CozoStore` does not exist yet

**Step 3: Implement CozoStore**

Add `pub mod cozo;` and `pub use cozo::CozoStore;` to `crates/core/src/store/mod.rs`.

Create `crates/core/src/store/cozo.rs`:

```rust
//! CozoDB implementation of the [`GraphStore`] trait.

use cozo::{DataValue, DbInstance, NamedRows, ScriptMutability};
use std::collections::BTreeMap;
use std::path::Path;

use crate::model::*;
use super::{GraphStore, Result, StoreError};

/// CozoDB-backed graph store.
///
/// Supports both in-memory (for tests) and persistent (SQLite-backed) modes.
#[derive(Debug)]
pub struct CozoStore {
    db: DbInstance,
}

impl CozoStore {
    /// Create a new in-memory store. Useful for testing.
    pub fn new_in_memory() -> Result<Self> {
        let db = DbInstance::new("mem", "", Default::default())
            .map_err(|e| StoreError::Internal(e.to_string()))?;
        let store = Self { db };
        store.init_schema()?;
        Ok(store)
    }

    /// Create a new persistent store backed by SQLite at the given path.
    pub fn new_persistent(path: &Path) -> Result<Self> {
        let db = DbInstance::new("sqlite", path.to_str().unwrap_or_default(), Default::default())
            .map_err(|e| StoreError::Internal(e.to_string()))?;
        let store = Self { db };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialise the database schema. Idempotent — safe to call on an existing database.
    fn init_schema(&self) -> Result<()> {
        let queries = [
            "{ :create snapshots { version: Int => kind: String, commit_ref: String?, created_at: String, metadata: Json? } }",
            "{ :create nodes { id: String, version: Int => canonical_path: String, qualified_name: String?, kind: String, sub_kind: String, name: String, language: String?, provenance: String, source_ref: String?, metadata: Json? } }",
            "{ :create edges { id: String, version: Int => source: String, target: String, kind: String, provenance: String, metadata: Json? } }",
            "{ :create constraints { id: String, version: Int => kind: String, name: String, scope: String, target: String?, params: Json?, message: String, severity: String } }",
        ];

        for query in queries {
            // Ignore "already exists" errors — makes init_schema idempotent
            let result = self.db.run_script(query, Default::default(), ScriptMutability::Mutable);
            if let Err(e) = &result {
                let msg = e.to_string();
                if !msg.contains("already exists") {
                    return Err(StoreError::Internal(msg));
                }
            }
        }
        Ok(())
    }

    /// Run a CozoScript query and return the result.
    fn run_query(&self, query: &str, params: BTreeMap<String, DataValue>) -> Result<NamedRows> {
        self.db
            .run_script(query, params, ScriptMutability::Mutable)
            .map_err(|e| StoreError::Internal(e.to_string()))
    }

    /// Run a read-only CozoScript query.
    fn run_query_immutable(&self, query: &str, params: BTreeMap<String, DataValue>) -> Result<NamedRows> {
        self.db
            .run_script(query, params, ScriptMutability::Immutable)
            .map_err(|e| StoreError::Internal(e.to_string()))
    }
}

impl GraphStore for CozoStore {
    fn create_snapshot(
        &mut self,
        kind: SnapshotKind,
        commit_ref: Option<&str>,
    ) -> Result<Version> {
        // Get the next version number
        let result = self.run_query_immutable(
            "?[max_v] := *snapshots{version}, max_v = max(version)
             ?[max_v] := max_v = 0, not *snapshots{version: _}",
            Default::default(),
        )?;

        let max_version: i64 = result.rows.iter()
            .filter_map(|row| row.first().and_then(|v| match v {
                DataValue::Num(n) => n.get_int(),
                _ => None,
            }))
            .max()
            .unwrap_or(0);

        let new_version = (max_version + 1) as Version;
        let kind_str = serde_json::to_value(&kind)
            .map_err(|e| StoreError::Internal(e.to_string()))?
            .as_str()
            .unwrap_or_default()
            .to_string();

        let now = chrono::Utc::now().to_rfc3339();

        let mut params = BTreeMap::new();
        params.insert("version".to_string(), DataValue::from(new_version as i64));
        params.insert("kind".to_string(), DataValue::Str(kind_str.into()));
        params.insert("commit_ref".to_string(), match commit_ref {
            Some(r) => DataValue::Str(r.to_string().into()),
            None => DataValue::Null,
        });
        params.insert("created_at".to_string(), DataValue::Str(now.into()));
        params.insert("metadata".to_string(), DataValue::Null);

        self.run_query(
            "?[version, kind, commit_ref, created_at, metadata] <- [[$version, $kind, $commit_ref, $created_at, $metadata]]
             :put snapshots { version => kind, commit_ref, created_at, metadata }",
            params,
        )?;

        Ok(new_version)
    }

    fn list_snapshots(&self) -> Result<Vec<Snapshot>> {
        let result = self.run_query_immutable(
            "?[version, kind, commit_ref, created_at, metadata] := *snapshots{version, kind, commit_ref, created_at, metadata}, :order version",
            Default::default(),
        )?;

        result.rows.iter().map(|row| {
            Ok(Snapshot {
                version: match &row[0] { DataValue::Num(n) => n.get_int().unwrap_or(0) as Version, _ => 0 },
                kind: serde_json::from_str(
                    &format!("\"{}\"", match &row[1] { DataValue::Str(s) => s.as_ref(), _ => "" })
                ).map_err(|e| StoreError::Internal(e.to_string()))?,
                commit_ref: match &row[2] { DataValue::Str(s) => Some(s.to_string()), _ => None },
                created_at: match &row[3] { DataValue::Str(s) => s.to_string(), _ => String::new() },
                metadata: match &row[4] { DataValue::Null => None, v => serde_json::to_value(format!("{v:?}")).ok() },
            })
        }).collect()
    }

    fn latest_version(&self, kind: SnapshotKind) -> Result<Option<Version>> {
        let kind_str = serde_json::to_value(&kind)
            .map_err(|e| StoreError::Internal(e.to_string()))?
            .as_str()
            .unwrap_or_default()
            .to_string();

        let mut params = BTreeMap::new();
        params.insert("kind".to_string(), DataValue::Str(kind_str.into()));

        let result = self.run_query_immutable(
            "?[version] := *snapshots{version, kind: $kind}, :order -version, :limit 1",
            params,
        )?;

        Ok(result.rows.first().and_then(|row| {
            match &row[0] {
                DataValue::Num(n) => n.get_int().map(|v| v as Version),
                _ => None,
            }
        }))
    }
}
```

Note: This implementation will need `chrono` for timestamps. Add `chrono = "0.4"` to `crates/core/Cargo.toml` dependencies.

**Step 4: Run tests**

Run: `cargo test -p svt-core`
Expected: all snapshot tests pass

**Step 5: Commit**

```bash
git add crates/core/src/store/ crates/core/tests/snapshot_tests.rs crates/core/Cargo.toml
git commit -m "Implement CozoStore with snapshot management"
```

---

### Task 5: Node CRUD — tests, trait methods, CozoDB implementation

**Files:**
- Modify: `crates/core/src/store/mod.rs` (add trait methods)
- Modify: `crates/core/src/store/cozo.rs` (implement methods)
- Create: `crates/core/tests/node_tests.rs`

**Step 1: Write failing node tests**

Create `crates/core/tests/node_tests.rs`:

```rust
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

fn make_node(id: &str, path: &str, kind: NodeKind, sub_kind: &str) -> Node {
    Node {
        id: id.to_string(),
        canonical_path: path.to_string(),
        qualified_name: None,
        kind,
        sub_kind: sub_kind.to_string(),
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        language: None,
        provenance: Provenance::Design,
        source_ref: None,
        metadata: None,
    }
}

#[test]
fn add_node_then_get_by_id_round_trips() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let node = make_node("n1", "/test-service", NodeKind::Service, "crate");
    store.add_node(v, &node).unwrap();

    let retrieved = store.get_node(v, &"n1".to_string()).unwrap().expect("node should exist");
    assert_eq!(retrieved.id, "n1");
    assert_eq!(retrieved.canonical_path, "/test-service");
    assert_eq!(retrieved.kind, NodeKind::Service);
}

#[test]
fn add_node_then_get_by_path_round_trips() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
    let node = make_node("n1", "/test-service", NodeKind::Service, "crate");
    store.add_node(v, &node).unwrap();

    let retrieved = store.get_node_by_path(v, "/test-service").unwrap().expect("node should exist");
    assert_eq!(retrieved.id, "n1");
}

#[test]
fn get_nonexistent_node_returns_none() {
    let store = CozoStore::new_in_memory().unwrap();
    let result = store.get_node(1, &"missing".to_string()).unwrap();
    assert!(result.is_none());
}

#[test]
fn add_nodes_batch_then_retrieve_all() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let nodes: Vec<Node> = (0..100)
        .map(|i| make_node(&format!("n{i}"), &format!("/svc/comp{i}"), NodeKind::Component, "module"))
        .collect();
    store.add_nodes_batch(v, &nodes).unwrap();

    for i in 0..100 {
        let n = store.get_node_by_path(v, &format!("/svc/comp{i}")).unwrap();
        assert!(n.is_some(), "node /svc/comp{i} not found");
    }
}

#[test]
fn node_optional_fields_survive_round_trip() {
    let mut store = CozoStore::new_in_memory().unwrap();
    let v = store.create_snapshot(SnapshotKind::Analysis, None).unwrap();

    let node = Node {
        id: "n1".to_string(),
        canonical_path: "/svc".to_string(),
        qualified_name: Some("my_svc".to_string()),
        kind: NodeKind::Service,
        sub_kind: "crate".to_string(),
        name: "svc".to_string(),
        language: Some("rust".to_string()),
        provenance: Provenance::Analysis,
        source_ref: Some("src/lib.rs:1".to_string()),
        metadata: Some(serde_json::json!({"wasm": true})),
    };
    store.add_node(v, &node).unwrap();

    let back = store.get_node(v, &"n1".to_string()).unwrap().unwrap();
    assert_eq!(back.qualified_name.as_deref(), Some("my_svc"));
    assert_eq!(back.language.as_deref(), Some("rust"));
    assert_eq!(back.source_ref.as_deref(), Some("src/lib.rs:1"));
    assert!(back.metadata.is_some());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p svt-core --test node_tests`
Expected: FAIL — trait methods don't exist yet

**Step 3: Add node methods to GraphStore trait**

In `crates/core/src/store/mod.rs`, add to the `GraphStore` trait:

```rust
    /// Add a single node to the store.
    fn add_node(&mut self, version: Version, node: &Node) -> Result<()>;

    /// Add multiple nodes in a single batch operation.
    fn add_nodes_batch(&mut self, version: Version, nodes: &[Node]) -> Result<()>;

    /// Get a node by its ID within a version.
    fn get_node(&self, version: Version, id: &NodeId) -> Result<Option<Node>>;

    /// Get a node by its canonical path within a version.
    fn get_node_by_path(&self, version: Version, canonical_path: &str) -> Result<Option<Node>>;
```

**Step 4: Implement node methods in CozoStore**

Add the implementations to the `impl GraphStore for CozoStore` block in `cozo.rs`. The implementation should:
- Use `:put nodes` for writes
- Use pattern matching `*nodes{...}` for reads
- Convert between Rust types and `DataValue` using helper methods
- Handle optional fields (None → DataValue::Null)

**Step 5: Run tests**

Run: `cargo test -p svt-core`
Expected: all tests pass (snapshot + node)

**Step 6: Commit**

```bash
git add crates/core/src/store/ crates/core/tests/node_tests.rs
git commit -m "Implement node CRUD: add_node, add_nodes_batch, get_node, get_node_by_path"
```

---

### Task 6: Edge CRUD — tests, trait methods, CozoDB implementation

**Files:**
- Modify: `crates/core/src/store/mod.rs` (add trait methods)
- Modify: `crates/core/src/store/cozo.rs` (implement methods)
- Create: `crates/core/tests/edge_tests.rs`

**Step 1: Write failing edge tests**

Create `crates/core/tests/edge_tests.rs` with tests for:
- Add edge, get outgoing edges from source — returns it
- Add edge, get incoming edges to target — returns it
- Direction::Both returns edges in either direction
- Filter by edge kind returns only matching edges
- `add_edges_batch` with many edges, all retrievable
- Edge metadata survives round-trip

**Step 2: Run to verify failure**

Run: `cargo test -p svt-core --test edge_tests`
Expected: FAIL

**Step 3: Add edge methods to trait**

```rust
    fn add_edge(&mut self, version: Version, edge: &Edge) -> Result<()>;
    fn add_edges_batch(&mut self, version: Version, edges: &[Edge]) -> Result<()>;
    fn get_edges(&self, version: Version, node_id: &NodeId, direction: Direction, kind: Option<EdgeKind>) -> Result<Vec<Edge>>;
```

**Step 4: Implement in CozoStore**

Edge queries need to handle direction (outgoing: `source=$id`, incoming: `target=$id`, both: union). Optional kind filter adds a condition.

**Step 5: Run tests, verify pass**

Run: `cargo test -p svt-core`
Expected: all pass

**Step 6: Commit**

```bash
git add crates/core/src/store/ crates/core/tests/edge_tests.rs
git commit -m "Implement edge CRUD: add_edge, add_edges_batch, get_edges with direction and kind filter"
```

---

### Task 7: Containment traversal — tests, trait methods, CozoDB implementation

**Files:**
- Modify: `crates/core/src/store/mod.rs` (add trait methods)
- Modify: `crates/core/src/store/cozo.rs` (implement methods)
- Create: `crates/core/tests/containment_tests.rs`

**Step 1: Write failing containment tests**

Create `crates/core/tests/containment_tests.rs` with tests for:
- get_children returns direct children
- get_children of leaf returns empty vec
- get_parent returns direct parent
- get_parent of root returns None
- query_ancestors returns full path to root
- query_descendants returns entire subtree
- query_descendants with NodeFilter returns only matching nodes
- 5-level deep hierarchy: verify ancestors and descendants at each level

Use a shared helper to build a test hierarchy: system → service → component → component → unit.

**Step 2: Run to verify failure**

**Step 3: Add containment methods to trait**

```rust
    fn get_children(&self, version: Version, node_id: &NodeId) -> Result<Vec<Node>>;
    fn get_parent(&self, version: Version, node_id: &NodeId) -> Result<Option<Node>>;
    fn query_ancestors(&self, version: Version, node_id: &NodeId) -> Result<Vec<Node>>;
    fn query_descendants(&self, version: Version, node_id: &NodeId, filter: Option<&NodeFilter>) -> Result<Vec<Node>>;
```

**Step 4: Implement in CozoStore**

- `get_children` / `get_parent`: single join between edges(kind=contains) and nodes
- `query_ancestors` / `query_descendants`: recursive Datalog rules (see design doc for query patterns)
- `query_descendants` with filter: add conditions for kind, sub_kind, language

**Step 5: Run tests, verify pass**

Run: `cargo test -p svt-core`
Expected: all pass

**Step 6: Add proptest for ancestors**

Add to the test file:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn ancestor_chain_has_no_duplicates(depth in 2..8usize) {
        let mut store = CozoStore::new_in_memory().unwrap();
        let v = store.create_snapshot(SnapshotKind::Design, None).unwrap();
        // Build a chain of depth levels
        // ... (build nodes and contains edges)
        // Query ancestors of deepest node
        // Assert no duplicate IDs
    }
}
```

**Step 7: Run all tests, commit**

```bash
git add crates/core/src/store/ crates/core/tests/containment_tests.rs
git commit -m "Implement containment traversal: children, parent, ancestors, descendants with filter"
```

---

### Task 8: Dependency traversal — tests, trait methods, CozoDB implementation

**Files:**
- Modify: `crates/core/src/store/mod.rs` (add trait methods)
- Modify: `crates/core/src/store/cozo.rs` (implement methods)
- Create: `crates/core/tests/dependency_tests.rs`

**Step 1: Write failing dependency tests**

Tests for:
- Direct dependencies returns immediate targets
- Transitive: A→B→C, query_dependencies(A, transitive=true) returns {B, C}
- Diamond: A→B, A→C, B→D, C→D — D appears once
- query_dependents is the reverse
- Node with no dependencies returns empty vec

**Step 2: Add trait methods**

```rust
    fn query_dependencies(&self, version: Version, node_id: &NodeId, transitive: bool) -> Result<Vec<Node>>;
    fn query_dependents(&self, version: Version, node_id: &NodeId, transitive: bool) -> Result<Vec<Node>>;
```

**Step 3: Implement with recursive Datalog**

Direct: single join. Transitive: recursive rule (see design doc).

**Step 4: Add proptest**

"Direct dependencies are a subset of transitive dependencies."

**Step 5: Run all tests, commit**

```bash
git add crates/core/src/store/ crates/core/tests/dependency_tests.rs
git commit -m "Implement dependency traversal: direct and transitive dependencies and dependents"
```

---

### Task 9: Constraint storage — tests, trait methods, CozoDB implementation

**Files:**
- Modify: `crates/core/src/store/mod.rs` (add trait methods)
- Modify: `crates/core/src/store/cozo.rs` (implement methods)
- Create: `crates/core/tests/constraint_tests.rs`

**Step 1: Write failing constraint tests**

Tests for:
- Add constraint, get_constraints returns it (round-trip)
- Multiple constraints per version all returned
- get_constraints for version with no constraints returns empty
- Optional fields (params, target) survive as None
- Constraints are version-scoped

**Step 2: Add trait methods**

```rust
    fn add_constraint(&mut self, version: Version, constraint: &Constraint) -> Result<()>;
    fn get_constraints(&self, version: Version) -> Result<Vec<Constraint>>;
```

**Step 3: Implement in CozoStore**

Same `:put` / pattern-match approach as nodes and edges.

**Step 4: Run all tests, commit**

```bash
git add crates/core/src/store/ crates/core/tests/constraint_tests.rs
git commit -m "Implement constraint storage: add_constraint, get_constraints"
```

---

### Task 10: Version compaction — tests, trait method, implementation

**Files:**
- Modify: `crates/core/src/store/mod.rs` (add trait method)
- Modify: `crates/core/src/store/cozo.rs` (implement)
- Create: `crates/core/tests/compact_tests.rs`

**Step 1: Write failing compact tests**

Tests for:
- Compact with keep=[2] preserves version 2 data, removes version 1
- Compact with empty keep removes all data
- Compact preserves snapshots, nodes, edges, and constraints for kept versions

**Step 2: Add trait method**

```rust
    fn compact(&mut self, keep_versions: &[Version]) -> Result<()>;
```

**Step 3: Implement using `:rm` operations**

Delete rows from all four relations where version is not in the keep list.

**Step 4: Run all tests, commit**

```bash
git add crates/core/src/store/ crates/core/tests/compact_tests.rs
git commit -m "Implement version compaction"
```

---

### Task 11: Validation module — cycle detection and referential integrity

**Files:**
- Create: `crates/core/src/validation.rs` (replace empty module)
- Modify: `crates/core/src/lib.rs` (update module)
- Create: `crates/core/tests/validation_tests.rs`

**Step 1: Write failing validation tests**

Tests for:
- Clean graph passes both validations
- Graph with a contains cycle (A contains B, B contains A) is detected
- Edge referencing non-existent node is flagged
- Self-referencing contains edge is detected

**Step 2: Implement validation functions**

```rust
//! Graph validation: structural invariants and referential integrity.

use crate::model::*;
use crate::store::{GraphStore, Result};

/// A cycle detected in the containment hierarchy.
#[derive(Debug, Clone)]
pub struct Cycle {
    /// Node IDs forming the cycle.
    pub node_ids: Vec<NodeId>,
}

/// A referential integrity error.
#[derive(Debug, Clone)]
pub struct IntegrityError {
    /// The edge with the invalid reference.
    pub edge_id: EdgeId,
    /// The missing node ID.
    pub missing_node_id: NodeId,
}

/// Check that contains edges form a DAG (no cycles).
pub fn validate_contains_acyclic(
    store: &impl GraphStore,
    version: Version,
) -> Result<Vec<Cycle>> {
    // Use CozoDB recursive query to detect cycles in contains edges
    todo!()
}

/// Check that all edge source/target references point to existing nodes.
pub fn validate_referential_integrity(
    store: &impl GraphStore,
    version: Version,
) -> Result<Vec<IntegrityError>> {
    // Query for edges where source or target node doesn't exist
    todo!()
}
```

Note: These functions operate on the `GraphStore` trait. For the CozoDB implementation, the validation logic will need to use the store's query capabilities. Consider whether to add a `run_validation_query` method to the trait or have validation functions work through existing trait methods.

**Step 3: Run all tests, commit**

```bash
git add crates/core/src/validation.rs crates/core/tests/validation_tests.rs crates/core/src/lib.rs
git commit -m "Implement validation: contains acyclicity and referential integrity checks"
```

---

### Task 12: Integration test with realistic graph + test fixtures

**Files:**
- Create: `crates/core/tests/helpers/mod.rs`
- Create: `crates/core/tests/integration_test.rs`

**Step 1: Create shared test fixtures**

`crates/core/tests/helpers/mod.rs`:

```rust
use svt_core::model::*;
use svt_core::store::{CozoStore, GraphStore};

/// Build a simple service with two components and a few units.
pub fn create_simple_service(store: &mut CozoStore, version: Version) {
    // /test-service (service)
    //   /test-service/handlers (component)
    //     /test-service/handlers/create (unit, function)
    //     /test-service/handlers/delete (unit, function)
    //   /test-service/models (component)
    //     /test-service/models/order (unit, struct)
    // + contains edges for hierarchy
    // + depends edge: handlers/create -> models/order
    todo!()
}

/// Build a layered architecture: api -> service -> repository -> database.
pub fn create_layered_architecture(store: &mut CozoStore, version: Version) {
    // /app (service)
    //   /app/api (component)
    //   /app/service (component)
    //   /app/repository (component)
    //   /app/database (component)
    // + contains edges
    // + depends edges following layer order
    todo!()
}
```

**Step 2: Write integration tests**

Tests that use the fixtures and exercise multiple trait methods together:
- Build simple service, navigate containment, query dependencies
- Build layered architecture, verify transitive dependencies follow layer order
- Create design + analysis snapshots, verify latest_version for each kind

**Step 3: Run all tests, commit**

```bash
git add crates/core/tests/
git commit -m "Add integration tests with realistic graph fixtures"
```

---

### Task 13: Final review and cleanup

**Step 1: Run full test suite**

Run: `cargo test -p svt-core`
Expected: all tests pass

**Step 2: Run clippy**

Run: `cargo clippy -p svt-core -- -D warnings`
Expected: no warnings

**Step 3: Run fmt check**

Run: `cargo fmt -p svt-core --check`
Expected: no formatting issues

**Step 4: Review doc comments**

Run: `cargo doc -p svt-core --no-deps`
Expected: builds without warnings

**Step 5: Commit any cleanup**

```bash
git add -A
git commit -m "Milestone 1 complete: core data model + CozoDB store with full test coverage"
```
