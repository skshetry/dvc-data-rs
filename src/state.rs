use itertools::Itertools;
use rusqlite::ToSql;
use rusqlite::{Connection, named_params, types::Null};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::iter::repeat_n;
use std::path::Path;
use std::time::SystemTime;
use thiserror::Error as ThisError;

use crate::timeutils::unix_time;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StateHash {
    #[serde(rename = "md5")]
    pub oid: String,
}

const MODE_TEXT: u8 = 1;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StateValue {
    pub checksum: String,
    pub size: u64,
    pub hash_info: StateHash,
}

#[derive(Debug)]
pub struct State {
    conn: Connection,
}

#[derive(ThisError, Debug)]
pub enum StateError {
    #[error("failed to create directory for state file")]
    FailedToCreateDirectory(#[from] std::io::Error),
    #[error(transparent)]
    SQLiteError(#[from] rusqlite::Error),
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}

impl State {
    pub fn open(path: &Path) -> Result<Self, StateError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        Self {
            conn: Connection::open(path)?,
        }
        .instantiate()
    }

    pub fn instantiate(self) -> Result<Self, StateError> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS Cache (
                    rowid INTEGER PRIMARY KEY,
                    key BLOB,
                    raw INTEGER,
                    store_time REAL,
                    expire_time REAL,
                    access_time REAL,
                    access_count INTEGER DEFAULT 0,
                    tag BLOB,
                    size INTEGER DEFAULT 0,
                    mode INTEGER DEFAULT 0,
                    filename TEXT,
                    value BLOB
                )",
            (),
        )?;
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS Cache_key_raw ON Cache(key, raw)",
            (),
        )?;
        self.conn.pragma_update(None, "synchronous", "NORMAL")?;
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(self)
    }

    pub fn open_in_memory() -> Result<Self, StateError> {
        Self {
            conn: Connection::open_in_memory()?,
        }
        .instantiate()
    }

    pub fn get(&self, key: &str) -> Result<Option<StateValue>, StateError> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT value FROM Cache WHERE key = :key and raw = 1")?;
        let mut rows = statement.query_map(named_params! {":key": key}, |row| row.get("value"))?;
        if let Some(result) = rows.next() {
            let value: String = result?;
            let state_value: StateValue = serde_json::from_str(&value)?;
            Ok(Some(state_value))
        } else {
            Ok(None)
        }
    }

    pub fn get_many<'a>(
        &self,
        items: impl Iterator<Item = &'a String>,
        batch_size: Option<usize>,
    ) -> Result<HashMap<String, StateValue>, StateError> {
        let batch_size = batch_size.unwrap_or(7999);
        let mut res = HashMap::new();

        for chunk in &items.chunks(batch_size) {
            let chunk: Vec<_> = chunk.collect();
            let mut vector: Vec<&dyn ToSql> = Vec::with_capacity(chunk.len());
            for item in chunk {
                vector.push(item);
            }

            let params = repeat_n("?", vector.len()).collect::<Vec<_>>().join(", ");

            let query = "SELECT key, value from Cache WHERE key in (".to_owned()
                + &params
                + ")"
                + " and raw = 1";
            let mut statement = self.conn.prepare_cached(&query)?;

            let mut rows = statement.query(&*vector)?;

            while let Some(row) = rows.next()? {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                let state_value: StateValue = serde_json::from_str(&value)?;
                res.insert(key, state_value);
            }
        }
        Ok(res)
    }

    pub fn set(&self, key: &str, value: &StateValue) -> Result<(), StateError> {
        let mut statement = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO Cache(
            key, raw, store_time, expire_time, access_time, tag, mode, filename, value)
            VALUES (:key, :raw, :store_time, :expire_time, :access_time, :tag, :mode, :filename, :value)
            ON CONFLICT(key, raw) DO UPDATE SET value = excluded.value"
        )?;
        let time = unix_time(SystemTime::now());
        let value = serde_json::to_string(&value)?;
        statement.execute(named_params! {
            ":key": key,
            ":raw": 1,
            ":store_time": time,
            ":expire_time": Null,
            ":access_time": time,
            ":tag": Null,
            ":mode": MODE_TEXT,
            ":filename": Null,
            ":value": value,
        })?;
        Ok(())
    }

    pub fn set_many(
        &self,
        items: impl Iterator<Item = (String, StateValue)>,
    ) -> Result<(), StateError> {
        let mut items = items.peekable();
        if items.peek().is_none() {
            return Ok(());
        }

        let time = unix_time(SystemTime::now());
        let transaction = self.conn.unchecked_transaction()?;

        for chunk in &items.chunks(7999) {
            let chunk: Vec<_> = chunk.collect();
            let raw_query = prepare_insert(chunk.len());
            let mut statement = transaction.prepare_cached(raw_query.as_str())?;

            let mut params = Vec::with_capacity(chunk.len() * 4);
            let mut vector = Vec::with_capacity(chunk.len());
            for (key, value) in &chunk {
                let value = serde_json::to_string(&value)?;
                vector.push((key, time, value));
            }
            for batch in &vector {
                params.push(&batch.0 as &dyn ToSql);
                params.push(&batch.1 as &dyn ToSql);
                params.push(&batch.1 as &dyn ToSql);
                params.push(&batch.2 as &dyn ToSql);
            }
            statement.execute(&*params)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn is_empty(&self) -> Result<bool, StateError> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT EXISTS (SELECT 1 FROM Cache)")?;

        let mut rows = statement.query(())?;
        if let Some(row) = rows.next()? {
            Ok(row.get::<usize, usize>(0)? == 0)
        } else {
            Ok(true)
        }
    }
}

pub fn prepare_insert(batch_size: usize) -> String {
    let params = repeat_n("(?, 1, ?, NULL, ?, NULL, 1, NULL, ?)", batch_size).join(", ");

    "INSERT OR REPLACE INTO Cache(
    key, raw, store_time, expire_time, access_time, tag, mode, filename, value)
    VALUES "
        .to_owned()
        + &params
        + " ON CONFLICT(key, raw) DO UPDATE SET value = excluded.value"
}
