//! CozoDB implementation of the [`GraphStore`] trait.

use cozo::{DataValue, DbInstance, NamedRows, Num, ScriptMutability};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use super::{GraphStore, Result, StoreError};
use crate::model::*;

/// Current schema version supported by this binary.
///
/// Increment this whenever the schema changes. Each increment requires a
/// corresponding migration step in [`CozoStore::migrate`].
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

/// CozoDB-backed graph store.
///
/// Supports both in-memory (for tests) and persistent (SQLite-backed) modes.
pub struct CozoStore {
    db: DbInstance,
}

impl fmt::Debug for CozoStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CozoStore").finish_non_exhaustive()
    }
}

impl CozoStore {
    /// Create a new in-memory store. Useful for testing.
    pub fn new_in_memory() -> Result<Self> {
        let db = DbInstance::new("mem", "", Default::default())
            .map_err(|e| StoreError::Internal(e.to_string()))?;
        let store = Self { db };
        store.init_schema()?;
        store.migrate()?;
        Ok(store)
    }

    /// Create a new persistent store backed by SQLite at the given path.
    pub fn new_persistent(path: &Path) -> Result<Self> {
        let db = DbInstance::new(
            "sqlite",
            path.to_str().unwrap_or_default(),
            Default::default(),
        )
        .map_err(|e| StoreError::Internal(e.to_string()))?;
        let store = Self { db };
        store.init_schema()?;
        store.migrate()?;
        Ok(store)
    }

    /// Initialise the database schema. Idempotent -- safe to call on an existing database.
    fn init_schema(&self) -> Result<()> {
        let queries = [
            "{ :create metadata { key: String => value: String } }",
            "{ :create snapshots { version: Int => kind: String, commit_ref: String?, created_at: String, metadata: Json? } }",
            "{ :create nodes { id: String, version: Int => canonical_path: String, qualified_name: String?, kind: String, sub_kind: String, name: String, language: String?, provenance: String, source_ref: String?, metadata: Json? } }",
            "{ :create edges { id: String, version: Int => source: String, target: String, kind: String, provenance: String, metadata: Json? } }",
            "{ :create constraints { id: String, version: Int => kind: String, name: String, scope: String, target: String?, params: Json?, message: String, severity: String } }",
            "{ :create file_manifest { path: String, version: Int => hash: String, unit_name: String, language: String } }",
            "{ :create projects { id: String => name: String, created_at: String, description: String?, metadata: Json? } }",
            "{ :create snapshot_projects { version: Int, project_id: String } }",
        ];

        for query in queries {
            // Ignore "already exists" errors -- makes init_schema idempotent
            let result = self
                .db
                .run_script(query, Default::default(), ScriptMutability::Mutable);
            if let Err(e) = &result {
                let msg = e.to_string();
                if !msg.contains("already exists") && !msg.contains("conflicts") {
                    return Err(StoreError::Internal(msg));
                }
            }
        }
        Ok(())
    }

    /// Get the current schema version from the metadata relation.
    ///
    /// Returns `Ok(0)` if the metadata relation or key is missing (pre-migration store).
    pub fn schema_version(&self) -> Result<u32> {
        let result = self.run_query_immutable(
            "?[value] := *metadata{key, value}, key == 'schema_version'",
            Default::default(),
        );

        match result {
            Ok(rows) => match rows.rows.first() {
                Some(row) => {
                    let s = req_str(&row[0])?;
                    s.parse::<u32>().map_err(|_| {
                        StoreError::CorruptStore(format!("invalid schema_version value: {s}"))
                    })
                }
                None => Ok(0),
            },
            Err(_) => Ok(0), // metadata relation doesn't exist yet
        }
    }

    /// Set the schema version in the metadata relation.
    fn set_schema_version(&self, version: u32) -> Result<()> {
        let mut params = BTreeMap::new();
        params.insert("key".to_string(), DataValue::Str("schema_version".into()));
        params.insert(
            "value".to_string(),
            DataValue::Str(version.to_string().into()),
        );
        self.run_query(
            "?[key, value] <- [[$key, $value]]
             :put metadata { key => value }",
            params,
        )?;
        Ok(())
    }

    /// Run forward-only schema migrations.
    ///
    /// If the store's schema version is newer than what this binary supports,
    /// returns [`StoreError::SchemaMismatch`]. Otherwise, runs any pending
    /// migrations sequentially from the current version to [`CURRENT_SCHEMA_VERSION`].
    fn migrate(&self) -> Result<()> {
        let current = self.schema_version()?;

        if current > CURRENT_SCHEMA_VERSION {
            return Err(StoreError::SchemaMismatch {
                expected: CURRENT_SCHEMA_VERSION,
                found: current,
            });
        }

        if current < CURRENT_SCHEMA_VERSION {
            // v0 → v1: metadata relation already created by init_schema, just set version
            if current < 1 {
                self.set_schema_version(1)?;
            }

            // v1 → v2: add projects and snapshot_projects relations,
            // insert "default" project, tag all existing snapshots with "default"
            if current < 2 {
                self.migrate_v1_to_v2()?;
                self.set_schema_version(2)?;
            }

            // v2 → v3: make snapshot_projects key composite (version, project_id)
            // so multiple projects can have the same version number independently.
            if current < 3 {
                self.migrate_v2_to_v3()?;
                self.set_schema_version(3)?;
            }
        }

        Ok(())
    }

    /// Migration from v1 to v2: add project scoping.
    ///
    /// 1. Creates `projects` and `snapshot_projects` relations (already in init_schema).
    /// 2. Inserts a "default" project.
    /// 3. Tags all existing snapshots with `project_id = "default"`.
    fn migrate_v1_to_v2(&self) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        // Insert the default project
        let mut params = BTreeMap::new();
        params.insert(
            "id".to_string(),
            DataValue::Str(crate::model::DEFAULT_PROJECT_ID.into()),
        );
        params.insert("name".to_string(), DataValue::Str("Default Project".into()));
        params.insert("created_at".to_string(), DataValue::Str(now.into()));
        params.insert("description".to_string(), DataValue::Null);
        params.insert("metadata".to_string(), DataValue::Null);

        self.run_query(
            "?[id, name, created_at, description, metadata] <- [[$id, $name, $created_at, $description, $metadata]]
             :put projects { id => name, created_at, description, metadata }",
            params,
        )?;

        // Tag all existing snapshots with the default project
        // Note: v3 migration will recreate this relation with composite key,
        // but we still need to insert data here for the v1→v2 step.
        self.run_query(
            "?[version, project_id] := *snapshots{version}, project_id = 'default'
             :put snapshot_projects { version, project_id }",
            Default::default(),
        )?;

        Ok(())
    }

    /// Migration from v2 to v3: make `snapshot_projects` key composite.
    ///
    /// The old schema `{ version: Int => project_id: String }` only allows one
    /// project per version number.  The new schema `{ version: Int, project_id: String }`
    /// allows each project to have independent version numbering.
    fn migrate_v2_to_v3(&self) -> Result<()> {
        // 1. Read existing rows
        let existing = self.run_query_immutable(
            "?[version, project_id] := *snapshot_projects{version, project_id}",
            Default::default(),
        )?;

        // 2. Drop the old relation
        self.run_query("::remove snapshot_projects", Default::default())?;

        // 3. Create with composite key
        self.run_query(
            "{ :create snapshot_projects { version: Int, project_id: String } }",
            Default::default(),
        )?;

        // 4. Re-insert existing data
        if !existing.rows.is_empty() {
            self.run_query(
                "?[version, project_id] <- $rows
                 :put snapshot_projects { version, project_id }",
                BTreeMap::from([(
                    "rows".to_string(),
                    DataValue::List(existing.rows.into_iter().map(DataValue::List).collect()),
                )]),
            )?;
        }

        Ok(())
    }

    /// Run a CozoScript query and return the result.
    fn run_query(&self, query: &str, params: BTreeMap<String, DataValue>) -> Result<NamedRows> {
        self.db
            .run_script(query, params, ScriptMutability::Mutable)
            .map_err(|e| StoreError::Internal(e.to_string()))
    }

    /// Count the number of nodes in a given version.
    fn count_nodes(&self, version: Version) -> Result<usize> {
        let params = BTreeMap::from([("version".to_string(), DataValue::from(version as i64))]);
        let result = self.run_query_immutable("?[id] := *nodes{id, version: $version}", params)?;
        Ok(result.rows.len())
    }

    /// Count the number of edges in a given version.
    fn count_edges(&self, version: Version) -> Result<usize> {
        let params = BTreeMap::from([("version".to_string(), DataValue::from(version as i64))]);
        let result = self.run_query_immutable("?[id] := *edges{id, version: $version}", params)?;
        Ok(result.rows.len())
    }

    /// Run a read-only CozoScript query.
    fn run_query_immutable(
        &self,
        query: &str,
        params: BTreeMap<String, DataValue>,
    ) -> Result<NamedRows> {
        self.db
            .run_script(query, params, ScriptMutability::Immutable)
            .map_err(|e| StoreError::Internal(e.to_string()))
    }
}

/// Serialize a serde-serializable enum value to its JSON string representation.
fn enum_to_str<T: serde::Serialize>(value: &T) -> Result<String> {
    let v = serde_json::to_value(value).map_err(|e| StoreError::Internal(e.to_string()))?;
    Ok(v.as_str().unwrap_or_default().to_string())
}

/// Deserialize a JSON string value into a serde-deserializable enum.
fn str_to_enum<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(&format!("\"{s}\"")).map_err(|e| StoreError::Internal(e.to_string()))
}

/// Convert an optional String field from a DataValue.
fn opt_str(val: &DataValue) -> Option<String> {
    match val {
        DataValue::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

/// Convert a required String field from a DataValue.
///
/// Returns an error if the value is not a string, rather than silently returning
/// an empty string.
fn req_str(val: &DataValue) -> Result<String> {
    match val {
        DataValue::Str(s) => Ok(s.to_string()),
        _ => Err(StoreError::Internal(format!(
            "expected string, got: {val:?}"
        ))),
    }
}

/// Convert a `DataValue` to a `serde_json::Value`.
///
/// Used by [`opt_json`] to recursively convert CozoDB values to JSON.
fn datavalue_to_json(val: &DataValue) -> serde_json::Value {
    match val {
        DataValue::Null => serde_json::Value::Null,
        DataValue::Bool(b) => serde_json::Value::Bool(*b),
        DataValue::Num(Num::Int(i)) => serde_json::json!(*i),
        DataValue::Num(Num::Float(f)) => serde_json::json!(*f),
        DataValue::Str(s) => serde_json::Value::String(s.to_string()),
        DataValue::List(items) => {
            serde_json::Value::Array(items.iter().map(datavalue_to_json).collect())
        }
        DataValue::Json(j) => j.0.clone(),
        other => serde_json::Value::String(format!("{other:?}")),
    }
}

/// Convert a DataValue to an optional serde_json::Value for metadata.
fn opt_json(val: &DataValue) -> Option<serde_json::Value> {
    match val {
        DataValue::Null => None,
        _ => Some(datavalue_to_json(val)),
    }
}

/// Convert a DataValue to a DataValue suitable for storing optional String fields.
fn opt_to_dv(val: &Option<String>) -> DataValue {
    match val {
        Some(s) => DataValue::Str(s.clone().into()),
        None => DataValue::Null,
    }
}

/// Convert a metadata value to a DataValue for storage.
fn json_to_dv(val: &Option<serde_json::Value>) -> DataValue {
    match val {
        Some(v) => DataValue::Json(cozo::JsonData(v.clone())),
        None => DataValue::Null,
    }
}

/// Parse a row from the nodes relation into a Node struct.
///
/// Expected column order: id, canonical_path, qualified_name, kind, sub_kind,
/// name, language, provenance, source_ref, metadata
fn row_to_node(row: &[DataValue]) -> Result<Node> {
    Ok(Node {
        id: req_str(&row[0])?,
        canonical_path: req_str(&row[1])?,
        qualified_name: opt_str(&row[2]),
        kind: str_to_enum(&req_str(&row[3])?)?,
        sub_kind: req_str(&row[4])?,
        name: req_str(&row[5])?,
        language: opt_str(&row[6]),
        provenance: str_to_enum(&req_str(&row[7])?)?,
        source_ref: opt_str(&row[8]),
        metadata: opt_json(&row[9]),
    })
}

/// Parse a row from the edges relation into an Edge struct.
///
/// Expected column order: id, source, target, kind, provenance, metadata
fn row_to_edge(row: &[DataValue]) -> Result<Edge> {
    Ok(Edge {
        id: req_str(&row[0])?,
        source: req_str(&row[1])?,
        target: req_str(&row[2])?,
        kind: str_to_enum(&req_str(&row[3])?)?,
        provenance: str_to_enum(&req_str(&row[4])?)?,
        metadata: opt_json(&row[5]),
    })
}

impl GraphStore for CozoStore {
    fn create_project(&mut self, project: &crate::model::Project) -> Result<()> {
        crate::model::validate_project_id(&project.id).map_err(StoreError::InvalidProjectId)?;

        // Check for duplicates
        if self.project_exists(&project.id)? {
            return Err(StoreError::DuplicateProject(project.id.clone()));
        }

        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Str(project.id.clone().into()));
        params.insert(
            "name".to_string(),
            DataValue::Str(project.name.clone().into()),
        );
        params.insert(
            "created_at".to_string(),
            DataValue::Str(project.created_at.clone().into()),
        );
        params.insert("description".to_string(), opt_to_dv(&project.description));
        params.insert("metadata".to_string(), json_to_dv(&project.metadata));

        self.run_query(
            "?[id, name, created_at, description, metadata] <- [[$id, $name, $created_at, $description, $metadata]]
             :put projects { id => name, created_at, description, metadata }",
            params,
        )?;

        Ok(())
    }

    fn list_projects(&self) -> Result<Vec<crate::model::Project>> {
        let result = self.run_query_immutable(
            "?[id, name, created_at, description, metadata] := *projects{id, name, created_at, description, metadata}
             :order id",
            Default::default(),
        )?;

        result
            .rows
            .iter()
            .map(|row| {
                Ok(crate::model::Project {
                    id: req_str(&row[0])?,
                    name: req_str(&row[1])?,
                    created_at: req_str(&row[2])?,
                    description: opt_str(&row[3]),
                    metadata: opt_json(&row[4]),
                })
            })
            .collect()
    }

    fn get_project(&self, project_id: &str) -> Result<Option<crate::model::Project>> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Str(project_id.into()));

        let result = self.run_query_immutable(
            "?[id, name, created_at, description, metadata] := *projects{id, name, created_at, description, metadata}, id == $id",
            params,
        )?;

        match result.rows.first() {
            Some(row) => Ok(Some(crate::model::Project {
                id: req_str(&row[0])?,
                name: req_str(&row[1])?,
                created_at: req_str(&row[2])?,
                description: opt_str(&row[3]),
                metadata: opt_json(&row[4]),
            })),
            None => Ok(None),
        }
    }

    fn project_exists(&self, project_id: &str) -> Result<bool> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Str(project_id.into()));

        let result = self.run_query_immutable("?[id] := *projects{id}, id == $id", params)?;

        Ok(!result.rows.is_empty())
    }

    fn create_snapshot(
        &mut self,
        project_id: &str,
        kind: SnapshotKind,
        commit_ref: Option<&str>,
    ) -> Result<Version> {
        // Get the next version number scoped to this project
        let mut version_params = BTreeMap::new();
        version_params.insert("project_id".to_string(), DataValue::Str(project_id.into()));
        let result = self.run_query_immutable(
            "?[version] := *snapshot_projects{version, project_id}, project_id == $project_id
             :order -version
             :limit 1",
            version_params,
        )?;

        let max_version: i64 = result
            .rows
            .first()
            .and_then(|row| match row.first() {
                Some(DataValue::Num(Num::Int(i))) => Some(*i),
                _ => None,
            })
            .unwrap_or(0);

        let new_version = (max_version + 1) as Version;
        let kind_str = serde_json::to_value(kind)
            .map_err(|e| StoreError::Internal(e.to_string()))?
            .as_str()
            .unwrap_or_default()
            .to_string();

        let now = chrono::Utc::now().to_rfc3339();

        let mut params = BTreeMap::new();
        params.insert("version".to_string(), DataValue::from(new_version as i64));
        params.insert("kind".to_string(), DataValue::Str(kind_str.into()));
        params.insert(
            "commit_ref".to_string(),
            match commit_ref {
                Some(r) => DataValue::Str(r.to_string().into()),
                None => DataValue::Null,
            },
        );
        params.insert("created_at".to_string(), DataValue::Str(now.into()));
        params.insert("metadata".to_string(), DataValue::Null);

        self.run_query(
            "?[version, kind, commit_ref, created_at, metadata] <- [[$version, $kind, $commit_ref, $created_at, $metadata]]
             :put snapshots { version => kind, commit_ref, created_at, metadata }",
            params,
        )?;

        // Also insert into snapshot_projects
        let mut sp_params = BTreeMap::new();
        sp_params.insert("version".to_string(), DataValue::from(new_version as i64));
        sp_params.insert("project_id".to_string(), DataValue::Str(project_id.into()));

        self.run_query(
            "?[version, project_id] <- [[$version, $project_id]]
             :put snapshot_projects { version, project_id }",
            sp_params,
        )?;

        Ok(new_version)
    }

    fn list_snapshots(&self, project_id: &str) -> Result<Vec<Snapshot>> {
        let mut params = BTreeMap::new();
        params.insert("project_id".to_string(), DataValue::Str(project_id.into()));

        let result = self.run_query_immutable(
            "?[version, kind, commit_ref, created_at, metadata] :=
                *snapshot_projects{version, project_id}, project_id == $project_id,
                *snapshots{version, kind, commit_ref, created_at, metadata}
             :order version",
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| {
                Ok(Snapshot {
                    version: match &row[0] {
                        DataValue::Num(Num::Int(i)) => *i as Version,
                        _ => 0,
                    },
                    kind: str_to_enum(&req_str(&row[1])?)?,
                    commit_ref: opt_str(&row[2]),
                    created_at: req_str(&row[3])?,
                    metadata: opt_json(&row[4]),
                    project_id: project_id.to_string(),
                })
            })
            .collect()
    }

    fn latest_version(&self, project_id: &str, kind: SnapshotKind) -> Result<Option<Version>> {
        let kind_str = serde_json::to_value(kind)
            .map_err(|e| StoreError::Internal(e.to_string()))?
            .as_str()
            .unwrap_or_default()
            .to_string();

        let mut params = BTreeMap::new();
        params.insert("kind".to_string(), DataValue::Str(kind_str.into()));
        params.insert("project_id".to_string(), DataValue::Str(project_id.into()));

        let result = self.run_query_immutable(
            "?[version] :=
                *snapshot_projects{version, project_id}, project_id == $project_id,
                *snapshots{version, kind}, kind == $kind
             :order -version
             :limit 1",
            params,
        )?;

        Ok(result.rows.first().and_then(|row| match &row[0] {
            DataValue::Num(Num::Int(i)) => Some(*i as Version),
            _ => None,
        }))
    }

    fn add_node(&mut self, version: Version, node: &Node) -> Result<()> {
        let kind_str = enum_to_str(&node.kind)?;
        let prov_str = enum_to_str(&node.provenance)?;

        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Str(node.id.clone().into()));
        params.insert("version".to_string(), DataValue::from(version as i64));
        params.insert(
            "canonical_path".to_string(),
            DataValue::Str(node.canonical_path.clone().into()),
        );
        params.insert(
            "qualified_name".to_string(),
            opt_to_dv(&node.qualified_name),
        );
        params.insert("kind".to_string(), DataValue::Str(kind_str.into()));
        params.insert(
            "sub_kind".to_string(),
            DataValue::Str(node.sub_kind.clone().into()),
        );
        params.insert("name".to_string(), DataValue::Str(node.name.clone().into()));
        params.insert("language".to_string(), opt_to_dv(&node.language));
        params.insert("provenance".to_string(), DataValue::Str(prov_str.into()));
        params.insert("source_ref".to_string(), opt_to_dv(&node.source_ref));
        params.insert("metadata".to_string(), json_to_dv(&node.metadata));

        self.run_query(
            "?[id, version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] <- [[$id, $version, $canonical_path, $qualified_name, $kind, $sub_kind, $name, $language, $provenance, $source_ref, $metadata]]
             :put nodes { id, version => canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata }",
            params,
        )?;

        Ok(())
    }

    fn add_nodes_batch(&mut self, version: Version, nodes: &[Node]) -> Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }

        let mut rows: Vec<DataValue> = Vec::with_capacity(nodes.len());
        for node in nodes {
            let kind_str = enum_to_str(&node.kind)?;
            let prov_str = enum_to_str(&node.provenance)?;
            rows.push(DataValue::List(vec![
                DataValue::Str(node.id.clone().into()),
                DataValue::from(version as i64),
                DataValue::Str(node.canonical_path.clone().into()),
                opt_to_dv(&node.qualified_name),
                DataValue::Str(kind_str.into()),
                DataValue::Str(node.sub_kind.clone().into()),
                DataValue::Str(node.name.clone().into()),
                opt_to_dv(&node.language),
                DataValue::Str(prov_str.into()),
                opt_to_dv(&node.source_ref),
                json_to_dv(&node.metadata),
            ]));
        }

        let mut params = BTreeMap::new();
        params.insert("rows".to_string(), DataValue::List(rows));

        self.run_query(
            "?[id, version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] <- $rows
             :put nodes { id, version => canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata }",
            params,
        )?;

        Ok(())
    }

    fn get_node(&self, version: Version, id: &NodeId) -> Result<Option<Node>> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Str(id.clone().into()));
        params.insert("version".to_string(), DataValue::from(version as i64));

        let result = self.run_query_immutable(
            "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] := *nodes{id, version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata}, id == $id, version == $version",
            params,
        )?;

        match result.rows.first() {
            Some(row) => Ok(Some(row_to_node(row)?)),
            None => Ok(None),
        }
    }

    fn get_node_by_path(&self, version: Version, canonical_path: &str) -> Result<Option<Node>> {
        let mut params = BTreeMap::new();
        params.insert(
            "path".to_string(),
            DataValue::Str(canonical_path.to_string().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let result = self.run_query_immutable(
            "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] := *nodes{id, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata}, canonical_path == $path",
            params,
        )?;

        match result.rows.first() {
            Some(row) => Ok(Some(row_to_node(row)?)),
            None => Ok(None),
        }
    }

    fn add_edge(&mut self, version: Version, edge: &Edge) -> Result<()> {
        let kind_str = enum_to_str(&edge.kind)?;
        let prov_str = enum_to_str(&edge.provenance)?;

        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Str(edge.id.clone().into()));
        params.insert("version".to_string(), DataValue::from(version as i64));
        params.insert(
            "source".to_string(),
            DataValue::Str(edge.source.clone().into()),
        );
        params.insert(
            "target".to_string(),
            DataValue::Str(edge.target.clone().into()),
        );
        params.insert("kind".to_string(), DataValue::Str(kind_str.into()));
        params.insert("provenance".to_string(), DataValue::Str(prov_str.into()));
        params.insert("metadata".to_string(), json_to_dv(&edge.metadata));

        self.run_query(
            "?[id, version, source, target, kind, provenance, metadata] <- [[$id, $version, $source, $target, $kind, $provenance, $metadata]]
             :put edges { id, version => source, target, kind, provenance, metadata }",
            params,
        )?;

        Ok(())
    }

    fn add_edges_batch(&mut self, version: Version, edges: &[Edge]) -> Result<()> {
        if edges.is_empty() {
            return Ok(());
        }

        let mut rows: Vec<DataValue> = Vec::with_capacity(edges.len());
        for edge in edges {
            let kind_str = enum_to_str(&edge.kind)?;
            let prov_str = enum_to_str(&edge.provenance)?;
            rows.push(DataValue::List(vec![
                DataValue::Str(edge.id.clone().into()),
                DataValue::from(version as i64),
                DataValue::Str(edge.source.clone().into()),
                DataValue::Str(edge.target.clone().into()),
                DataValue::Str(kind_str.into()),
                DataValue::Str(prov_str.into()),
                json_to_dv(&edge.metadata),
            ]));
        }

        let mut params = BTreeMap::new();
        params.insert("rows".to_string(), DataValue::List(rows));

        self.run_query(
            "?[id, version, source, target, kind, provenance, metadata] <- $rows
             :put edges { id, version => source, target, kind, provenance, metadata }",
            params,
        )?;

        Ok(())
    }

    fn get_edges(
        &self,
        version: Version,
        node_id: &NodeId,
        direction: Direction,
        kind: Option<EdgeKind>,
    ) -> Result<Vec<Edge>> {
        let mut params = BTreeMap::new();
        params.insert(
            "node_id".to_string(),
            DataValue::Str(node_id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let kind_filter = match &kind {
            Some(k) => {
                let kind_str = enum_to_str(k)?;
                params.insert("kind_filter".to_string(), DataValue::Str(kind_str.into()));
                ", kind == $kind_filter"
            }
            None => "",
        };

        let query = match direction {
            Direction::Outgoing => format!(
                "?[id, source, target, kind, provenance, metadata] := *edges{{id, version, source, target, kind, provenance, metadata}}, version == $version, source == $node_id{kind_filter}"
            ),
            Direction::Incoming => format!(
                "?[id, source, target, kind, provenance, metadata] := *edges{{id, version, source, target, kind, provenance, metadata}}, version == $version, target == $node_id{kind_filter}"
            ),
            Direction::Both => format!(
                "?[id, source, target, kind, provenance, metadata] := *edges{{id, version, source, target, kind, provenance, metadata}}, version == $version, source == $node_id{kind_filter}
                 ?[id, source, target, kind, provenance, metadata] := *edges{{id, version, source, target, kind, provenance, metadata}}, version == $version, target == $node_id{kind_filter}"
            ),
        };

        let result = self.run_query_immutable(&query, params)?;

        result.rows.iter().map(|row| row_to_edge(row)).collect()
    }

    fn get_children(&self, version: Version, node_id: &NodeId) -> Result<Vec<Node>> {
        let mut params = BTreeMap::new();
        params.insert(
            "parent_id".to_string(),
            DataValue::Str(node_id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let result = self.run_query_immutable(
            "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                *edges{version, source, target, kind: edge_kind},
                version == $version, source == $parent_id, edge_kind == 'contains',
                *nodes{id: target, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                id = target",
            params,
        )?;

        result.rows.iter().map(|row| row_to_node(row)).collect()
    }

    fn get_parent(&self, version: Version, node_id: &NodeId) -> Result<Option<Node>> {
        let mut params = BTreeMap::new();
        params.insert(
            "child_id".to_string(),
            DataValue::Str(node_id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let result = self.run_query_immutable(
            "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                *edges{version, source, target, kind: edge_kind},
                version == $version, target == $child_id, edge_kind == 'contains',
                *nodes{id: source, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                id = source",
            params,
        )?;

        match result.rows.first() {
            Some(row) => Ok(Some(row_to_node(row)?)),
            None => Ok(None),
        }
    }

    fn query_ancestors(&self, version: Version, node_id: &NodeId) -> Result<Vec<Node>> {
        let mut params = BTreeMap::new();
        params.insert(
            "start_id".to_string(),
            DataValue::Str(node_id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let result = self.run_query_immutable(
            "ancestor[node_id] := *edges{version, source, target, kind}, version == $version, kind == 'contains', target == $start_id, node_id = source
             ancestor[node_id] := ancestor[child], *edges{version, source, target, kind}, version == $version, kind == 'contains', target == child, node_id = source
             ?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                ancestor[ancestor_id],
                *nodes{id: ancestor_id, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                id = ancestor_id",
            params,
        )?;

        result.rows.iter().map(|row| row_to_node(row)).collect()
    }

    fn query_descendants(
        &self,
        version: Version,
        node_id: &NodeId,
        filter: Option<&NodeFilter>,
    ) -> Result<Vec<Node>> {
        let mut params = BTreeMap::new();
        params.insert(
            "start_id".to_string(),
            DataValue::Str(node_id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let mut filter_clauses = String::new();
        if let Some(f) = filter {
            if let Some(kind) = &f.kind {
                let kind_str = enum_to_str(kind)?;
                params.insert("filter_kind".to_string(), DataValue::Str(kind_str.into()));
                filter_clauses.push_str(", kind == $filter_kind");
            }
            if let Some(sub_kind) = &f.sub_kind {
                params.insert(
                    "filter_sub_kind".to_string(),
                    DataValue::Str(sub_kind.clone().into()),
                );
                filter_clauses.push_str(", sub_kind == $filter_sub_kind");
            }
            if let Some(language) = &f.language {
                params.insert(
                    "filter_language".to_string(),
                    DataValue::Str(language.clone().into()),
                );
                filter_clauses.push_str(", language == $filter_language");
            }
        }

        let query = format!(
            "descendant[node_id] := *edges{{version, source, target, kind: edge_kind}}, version == $version, edge_kind == 'contains', source == $start_id, node_id = target
             descendant[node_id] := descendant[parent], *edges{{version, source, target, kind: edge_kind}}, version == $version, edge_kind == 'contains', source == parent, node_id = target
             ?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                descendant[desc_id],
                *nodes{{id: desc_id, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata}},
                id = desc_id{filter_clauses}"
        );

        let result = self.run_query_immutable(&query, params)?;

        result.rows.iter().map(|row| row_to_node(row)).collect()
    }

    fn query_dependencies(
        &self,
        version: Version,
        node_id: &NodeId,
        transitive: bool,
    ) -> Result<Vec<Node>> {
        let mut params = BTreeMap::new();
        params.insert(
            "start_id".to_string(),
            DataValue::Str(node_id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let query = if transitive {
            "dep[node_id] := *edges{version, source, target, kind}, version == $version, kind == 'depends', source == $start_id, node_id = target
             dep[node_id] := dep[mid], *edges{version, source, target, kind}, version == $version, kind == 'depends', source == mid, node_id = target
             ?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                dep[dep_id],
                *nodes{id: dep_id, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                id = dep_id"
        } else {
            "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                *edges{version, source, target, kind: edge_kind},
                version == $version, source == $start_id, edge_kind == 'depends',
                *nodes{id: target, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                id = target"
        };

        let result = self.run_query_immutable(query, params)?;
        result.rows.iter().map(|row| row_to_node(row)).collect()
    }

    fn query_dependents(
        &self,
        version: Version,
        node_id: &NodeId,
        transitive: bool,
    ) -> Result<Vec<Node>> {
        let mut params = BTreeMap::new();
        params.insert(
            "start_id".to_string(),
            DataValue::Str(node_id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));

        let query = if transitive {
            "dep[node_id] := *edges{version, source, target, kind}, version == $version, kind == 'depends', target == $start_id, node_id = source
             dep[node_id] := dep[mid], *edges{version, source, target, kind}, version == $version, kind == 'depends', target == mid, node_id = source
             ?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                dep[dep_id],
                *nodes{id: dep_id, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                id = dep_id"
        } else {
            "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                *edges{version, source, target, kind: edge_kind},
                version == $version, target == $start_id, edge_kind == 'depends',
                *nodes{id: source, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                id = source"
        };

        let result = self.run_query_immutable(query, params)?;
        result.rows.iter().map(|row| row_to_node(row)).collect()
    }

    fn add_constraint(&mut self, version: Version, constraint: &Constraint) -> Result<()> {
        let severity_str = enum_to_str(&constraint.severity)?;

        let mut params = BTreeMap::new();
        params.insert(
            "id".to_string(),
            DataValue::Str(constraint.id.clone().into()),
        );
        params.insert("version".to_string(), DataValue::from(version as i64));
        params.insert(
            "kind".to_string(),
            DataValue::Str(constraint.kind.clone().into()),
        );
        params.insert(
            "name".to_string(),
            DataValue::Str(constraint.name.clone().into()),
        );
        params.insert(
            "scope".to_string(),
            DataValue::Str(constraint.scope.clone().into()),
        );
        params.insert("target".to_string(), opt_to_dv(&constraint.target));
        params.insert("params".to_string(), json_to_dv(&constraint.params));
        params.insert(
            "message".to_string(),
            DataValue::Str(constraint.message.clone().into()),
        );
        params.insert("severity".to_string(), DataValue::Str(severity_str.into()));

        self.run_query(
            "?[id, version, kind, name, scope, target, params, message, severity] <- [[$id, $version, $kind, $name, $scope, $target, $params, $message, $severity]]
             :put constraints { id, version => kind, name, scope, target, params, message, severity }",
            params,
        )?;

        Ok(())
    }

    fn get_constraints(&self, version: Version) -> Result<Vec<Constraint>> {
        let mut params = BTreeMap::new();
        params.insert("version".to_string(), DataValue::from(version as i64));

        let result = self.run_query_immutable(
            "?[id, kind, name, scope, target, params, message, severity] := *constraints{id, version, kind, name, scope, target, params, message, severity}, version == $version",
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| {
                Ok(Constraint {
                    id: req_str(&row[0])?,
                    kind: req_str(&row[1])?,
                    name: req_str(&row[2])?,
                    scope: req_str(&row[3])?,
                    target: opt_str(&row[4]),
                    params: opt_json(&row[5]),
                    message: req_str(&row[6])?,
                    severity: str_to_enum(&req_str(&row[7])?)?,
                })
            })
            .collect()
    }

    fn compact(&mut self, project_id: &str, keep_versions: &[Version]) -> Result<()> {
        // Get versions belonging to this project that are NOT in keep list
        let keep_list: Vec<DataValue> = keep_versions
            .iter()
            .map(|v| DataValue::from(*v as i64))
            .collect();

        let mut params = BTreeMap::new();
        params.insert("keep".to_string(), DataValue::List(keep_list));
        params.insert("project_id".to_string(), DataValue::Str(project_id.into()));

        // Delete nodes not in keep list (scoped to project's versions)
        // Note: snapshot_projects must remain intact until all entity deletions
        // are complete, since they are used to resolve which versions belong to
        // this project.
        self.run_query(
            "to_remove[id, version] := *nodes{id, version}, *snapshot_projects{version, project_id}, project_id == $project_id, not keep_set[version]
             keep_set[v] := v in $keep
             ?[id, version] := to_remove[id, version]
             :rm nodes {id, version}",
            params.clone(),
        )?;

        // Delete edges not in keep list
        self.run_query(
            "to_remove[id, version] := *edges{id, version}, *snapshot_projects{version, project_id}, project_id == $project_id, not keep_set[version]
             keep_set[v] := v in $keep
             ?[id, version] := to_remove[id, version]
             :rm edges {id, version}",
            params.clone(),
        )?;

        // Delete constraints not in keep list
        self.run_query(
            "to_remove[id, version] := *constraints{id, version}, *snapshot_projects{version, project_id}, project_id == $project_id, not keep_set[version]
             keep_set[v] := v in $keep
             ?[id, version] := to_remove[id, version]
             :rm constraints {id, version}",
            params.clone(),
        )?;

        // Delete file_manifest entries not in keep list
        self.run_query(
            "to_remove[path, version] := *file_manifest{path, version}, *snapshot_projects{version, project_id}, project_id == $project_id, not keep_set[version]
             keep_set[v] := v in $keep
             ?[path, version] := to_remove[path, version]
             :rm file_manifest {path, version}",
            params.clone(),
        )?;

        // Delete snapshots for this project not in keep list
        self.run_query(
            "to_remove[version] := *snapshot_projects{version, project_id}, project_id == $project_id, not keep_set[version]
             keep_set[v] := v in $keep
             ?[version] := to_remove[version]
             :rm snapshots {version}",
            params.clone(),
        )?;

        // Delete snapshot_projects entries last (other deletions depend on this table)
        self.run_query(
            "to_remove[version, project_id] := *snapshot_projects{version, project_id}, project_id == $project_id, not keep_set[version]
             keep_set[v] := v in $keep
             ?[version, project_id] := to_remove[version, project_id]
             :rm snapshot_projects {version, project_id}",
            params,
        )?;

        Ok(())
    }

    fn get_all_nodes(&self, version: Version) -> Result<Vec<Node>> {
        let query = "?[id, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] := *nodes{id, version: $version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata}";
        let params = BTreeMap::from([("version".to_string(), DataValue::from(version as i64))]);
        let result = self.run_query_immutable(query, params)?;
        result.rows.iter().map(|row| row_to_node(row)).collect()
    }

    fn get_all_edges(&self, version: Version, kind: Option<EdgeKind>) -> Result<Vec<Edge>> {
        let mut params = BTreeMap::new();
        params.insert("version".to_string(), DataValue::from(version as i64));

        let query = match &kind {
            Some(k) => {
                let kind_str = enum_to_str(k)?;
                params.insert("edge_kind".to_string(), DataValue::Str(kind_str.into()));
                "?[id, source, target, kind, provenance, metadata] := *edges{id, version, source, target, kind, provenance, metadata}, version == $version, kind == $edge_kind"
            }
            None => {
                "?[id, source, target, kind, provenance, metadata] := *edges{id, version, source, target, kind, provenance, metadata}, version == $version"
            }
        };

        let result = self.run_query_immutable(query, params)?;
        result.rows.iter().map(|row| row_to_edge(row)).collect()
    }

    fn add_file_manifest(&mut self, version: Version, entries: &[FileManifestEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let mut rows: Vec<DataValue> = Vec::with_capacity(entries.len());
        for entry in entries {
            rows.push(DataValue::List(vec![
                DataValue::Str(entry.path.clone().into()),
                DataValue::from(version as i64),
                DataValue::Str(entry.hash.clone().into()),
                DataValue::Str(entry.unit_name.clone().into()),
                DataValue::Str(entry.language.clone().into()),
            ]));
        }

        let mut params = BTreeMap::new();
        params.insert("rows".to_string(), DataValue::List(rows));

        self.run_query(
            "?[path, version, hash, unit_name, language] <- $rows
             :put file_manifest { path, version => hash, unit_name, language }",
            params,
        )?;

        Ok(())
    }

    fn get_file_manifest(&self, version: Version) -> Result<Vec<FileManifestEntry>> {
        let params = BTreeMap::from([("version".to_string(), DataValue::from(version as i64))]);
        let result = self.run_query_immutable(
            "?[path, hash, unit_name, language] := *file_manifest{path, version: $version, hash, unit_name, language}",
            params,
        )?;

        result
            .rows
            .iter()
            .map(|row| {
                Ok(FileManifestEntry {
                    path: req_str(&row[0])?,
                    hash: req_str(&row[1])?,
                    unit_name: req_str(&row[2])?,
                    language: req_str(&row[3])?,
                })
            })
            .collect()
    }

    fn copy_nodes(&mut self, from_version: Version, to_version: Version) -> Result<usize> {
        let mut params = BTreeMap::new();
        params.insert(
            "from_version".to_string(),
            DataValue::from(from_version as i64),
        );
        params.insert("to_version".to_string(), DataValue::from(to_version as i64));

        // First count how many nodes we'll copy
        let count = self.count_nodes(from_version)?;

        if count == 0 {
            return Ok(0);
        }

        self.run_query(
            "?[id, version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata] :=
                *nodes{id, version: $from_version, canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata},
                version = $to_version
             :put nodes { id, version => canonical_path, qualified_name, kind, sub_kind, name, language, provenance, source_ref, metadata }",
            params,
        )?;

        Ok(count)
    }

    fn copy_edges(&mut self, from_version: Version, to_version: Version) -> Result<usize> {
        let mut params = BTreeMap::new();
        params.insert(
            "from_version".to_string(),
            DataValue::from(from_version as i64),
        );
        params.insert("to_version".to_string(), DataValue::from(to_version as i64));

        // First count how many edges we'll copy
        let count = self.count_edges(from_version)?;

        if count == 0 {
            return Ok(0);
        }

        self.run_query(
            "?[id, version, source, target, kind, provenance, metadata] :=
                *edges{id, version: $from_version, source, target, kind, provenance, metadata},
                version = $to_version
             :put edges { id, version => source, target, kind, provenance, metadata }",
            params,
        )?;

        Ok(count)
    }

    fn store_info(&self) -> Result<super::StoreInfo> {
        let schema_version = self.schema_version()?;
        let projects = self.list_projects()?;

        // Get all snapshots across all projects
        let all_snapshots_result = self.run_query_immutable(
            "?[version, kind, commit_ref, created_at, metadata, project_id] :=
                *snapshots{version, kind, commit_ref, created_at, metadata},
                *snapshot_projects{version, project_id}
             :order version",
            Default::default(),
        )?;

        let summaries: Vec<super::SnapshotSummary> = all_snapshots_result
            .rows
            .iter()
            .map(|row| {
                let version = match &row[0] {
                    DataValue::Num(Num::Int(i)) => *i as Version,
                    _ => 0,
                };
                let node_count = self.count_nodes(version)?;
                let edge_count = self.count_edges(version)?;
                Ok(super::SnapshotSummary {
                    version,
                    kind: str_to_enum(&req_str(&row[1])?)?,
                    commit_ref: opt_str(&row[2]),
                    node_count,
                    edge_count,
                    created_at: req_str(&row[3])?,
                    project_id: req_str(&row[5])?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let project_summaries: Vec<super::ProjectSummary> = projects
            .iter()
            .map(|p| {
                let count = summaries.iter().filter(|s| s.project_id == p.id).count();
                super::ProjectSummary {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    snapshot_count: count,
                }
            })
            .collect();

        Ok(super::StoreInfo {
            schema_version,
            snapshot_count: summaries.len(),
            snapshots: summaries,
            project_count: project_summaries.len(),
            projects: project_summaries,
        })
    }
}
