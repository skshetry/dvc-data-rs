use rusqlite::{named_params, types::Null, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::fs;

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

pub fn as_fractional_seconds(dur: Duration) -> f64 {
    dur.as_secs() as f64 + dur.subsec_nanos() as f64 / 1_000_000_000.0
}

pub struct State {
    conn: Connection,
}

impl State {
    pub fn open(path: &PathBuf) -> Result<Self> {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        State {
            conn: Connection::open(path)?,
        }
        .instantiate()
    }

    pub fn instantiate(self) -> Result<Self> {
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
            "CREATE UNIQUE INDEX IF NOT EXISTS Cache_key ON Cache(key)",
            (),
        )?;
        self.conn.pragma_update(None, "synchronous", "NORMAL")?;
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(self)
    }

    pub fn open_in_memory() -> Result<Self> {
        State {
            conn: Connection::open_in_memory()?,
        }
        .instantiate()
    }

    pub fn get(&self, key: String) -> Result<Option<StateValue>> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT value FROM Cache WHERE key = :key")?;
        let mut rows = statement.query_map(named_params! {":key": key}, |row| {
            let value: String = row.get("value")?;
            let state_value: StateValue = serde_json::from_str(&value).unwrap();
            Ok(state_value)
        })?;
        match rows.next() {
            None => Ok(None),
            Some(value) => Ok(value.ok()),
        }
    }

    pub fn set(&self, key: String, value: &StateValue) -> Result<()> {
        let mut statement = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO Cache(
            key, raw, store_time, expire_time, access_time, tag, mode, filename, value)
            VALUES (:key, :raw, :store_time, :expire_time, :access_time, :tag, :mode, :filename, :value)",
        )?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let time = as_fractional_seconds(now);
        let value = serde_json::to_string(value).unwrap();
        let value = value.replace(',', ", ").replace(':', ": ");
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

    pub fn set_many(&self, items: impl Iterator<Item = (String, StateValue)>) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let time = as_fractional_seconds(now);
        let transaction = self.conn.unchecked_transaction()?;
        let mut statement = transaction.prepare_cached(
            "INSERT OR REPLACE INTO Cache(
            key, raw, store_time, expire_time, access_time, tag, mode, filename, value)
            VALUES (:key, :raw, :store_time, :expire_time, :access_time, :tag, :mode, :filename, :value)",
        )?;
        for (key, value) in items {
            let value = serde_json::to_string(&value).unwrap();
            let value = value.replace(',', ", ").replace(':', ": ");
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
        }
        drop(statement);
        transaction.commit()?;
        Ok(())
    }
}
