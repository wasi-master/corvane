//! Persistence: one redb file holding JSON values keyed by string.
//! Typed accessors live in `corvane_core::persistence` (this crate stays
//! dependency-free so core can depend on it).

use std::path::{Path, PathBuf};

use redb::{Database, ReadableDatabase, TableDefinition};
use serde::{Serialize, de::DeserializeOwned};

const KV: TableDefinition<&str, &[u8]> = TableDefinition::new("kv");
const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] redb::Error),
    #[error("database error: {0}")]
    DbOpen(#[from] redb::DatabaseError),
    #[error("transaction error: {0}")]
    Txn(#[from] redb::TransactionError),
    #[error("table error: {0}")]
    Table(#[from] redb::TableError),
    #[error("storage error: {0}")]
    Storage(#[from] redb::StorageError),
    #[error("commit error: {0}")]
    Commit(#[from] redb::CommitError),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    db: Database,
    path: PathBuf,
}

impl Store {
    /// Open (or create) the database at `dir/corvane.redb`.
    pub fn open_in(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        Self::open(dir.join("corvane.redb"))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let db = Database::create(&path)?;
        let store = Self { db, path };
        store.ensure_schema()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn ensure_schema(&self) -> Result<()> {
        let current: Option<u32> = self.get("meta.schema_version")?;
        if current != Some(SCHEMA_VERSION) {
            tracing::info!(from = ?current, to = SCHEMA_VERSION, "initialising store schema");
            self.set("meta.schema_version", &SCHEMA_VERSION)?;
        }
        Ok(())
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let txn = self.db.begin_read()?;
        let table = match txn.open_table(KV) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        match table.get(key)? {
            Some(guard) => Ok(Some(serde_json::from_slice(guard.value())?)),
            None => Ok(None),
        }
    }

    pub fn set<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(KV)?;
            table.insert(key, bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn remove(&self, key: &str) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(KV)?;
            table.remove(key)?;
        }
        txn.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in(dir.path()).unwrap();
        let v: Option<String> = store.get("nope").unwrap();
        assert!(v.is_none());
        store.set("k", &vec![1u32, 2, 3]).unwrap();
        assert_eq!(store.get::<Vec<u32>>("k").unwrap(), Some(vec![1, 2, 3]));
        store.remove("k").unwrap();
        assert!(store.get::<Vec<u32>>("k").unwrap().is_none());
        assert_eq!(store.get::<u32>("meta.schema_version").unwrap(), Some(1));
    }
}
