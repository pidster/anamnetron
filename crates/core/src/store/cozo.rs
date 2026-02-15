//! CozoDB implementation of the [`GraphStore`] trait.

use cozo::{DataValue, DbInstance, NamedRows, Num, ScriptMutability};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use super::{GraphStore, Result, StoreError};
use crate::model::*;

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
        Ok(store)
    }

    /// Initialise the database schema. Idempotent -- safe to call on an existing database.
    fn init_schema(&self) -> Result<()> {
        let queries = [
            "{ :create snapshots { version: Int => kind: String, commit_ref: String?, created_at: String, metadata: Json? } }",
            "{ :create nodes { id: String, version: Int => canonical_path: String, qualified_name: String?, kind: String, sub_kind: String, name: String, language: String?, provenance: String, source_ref: String?, metadata: Json? } }",
            "{ :create edges { id: String, version: Int => source: String, target: String, kind: String, provenance: String, metadata: Json? } }",
            "{ :create constraints { id: String, version: Int => kind: String, name: String, scope: String, target: String?, params: Json?, message: String, severity: String } }",
        ];

        for query in queries {
            // Ignore "already exists" errors -- makes init_schema idempotent
            let result = self
                .db
                .run_script(query, Default::default(), ScriptMutability::Mutable);
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

impl GraphStore for CozoStore {
    fn create_snapshot(&mut self, kind: SnapshotKind, commit_ref: Option<&str>) -> Result<Version> {
        // Get the next version number by finding the current max
        let result = self.run_query_immutable(
            "?[version] := *snapshots{version}
             :order -version
             :limit 1",
            Default::default(),
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
        let kind_str = serde_json::to_value(&kind)
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

        Ok(new_version)
    }

    fn list_snapshots(&self) -> Result<Vec<Snapshot>> {
        let result = self.run_query_immutable(
            "?[version, kind, commit_ref, created_at, metadata] := *snapshots{version, kind, commit_ref, created_at, metadata}
             :order version",
            Default::default(),
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
                    kind: serde_json::from_str(&format!(
                        "\"{}\"",
                        match &row[1] {
                            DataValue::Str(s) => s.as_ref(),
                            _ => "",
                        }
                    ))
                    .map_err(|e| StoreError::Internal(e.to_string()))?,
                    commit_ref: match &row[2] {
                        DataValue::Str(s) => Some(s.to_string()),
                        _ => None,
                    },
                    created_at: match &row[3] {
                        DataValue::Str(s) => s.to_string(),
                        _ => String::new(),
                    },
                    metadata: match &row[4] {
                        DataValue::Null => None,
                        v => serde_json::to_value(format!("{v:?}")).ok(),
                    },
                })
            })
            .collect()
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
            "?[version] := *snapshots{version, kind}, kind == $kind
             :order -version
             :limit 1",
            params,
        )?;

        Ok(result.rows.first().and_then(|row| match &row[0] {
            DataValue::Num(Num::Int(i)) => Some(*i as Version),
            _ => None,
        }))
    }
}
