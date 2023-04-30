use itertools::{repeat_n, Itertools};
use rusqlite::ToSql;
use rusqlite::{named_params, types::Null, Connection, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

#[derive(Debug)]
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

    pub fn get_many<'a>(
        &self,
        items: impl Iterator<Item = &'a String>,
        batch_size: Option<usize>,
    ) -> Result<HashMap<String, StateValue>> {
        let batch_size = batch_size.unwrap_or(7999);
        let mut res = HashMap::new();

        for chunk in &items.chunks(batch_size) {
            let mut vector: Vec<&dyn ToSql> = Vec::new();
            let chunk: Vec<_> = chunk.collect();
            for item in chunk {
                vector.push(item);
            }

            let params = repeat_n("?", vector.len()).join(", ");
            let query = "SELECT key, value from Cache WHERE key in (".to_owned() + &params + ")";
            let mut statement = self.conn.prepare_cached(&query)?;

            let mut rows = statement.query(&*vector)?;

            while let Some(row) = rows.next()? {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                let state_value: StateValue = serde_json::from_str(&value).unwrap();
                res.insert(key, state_value);
            }
        }
        Ok(res)
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

        for chunk in &items.chunks(7999) {
            let chunk: Vec<_> = chunk.collect();
            let raw_query = prepare_insert(chunk.len());
            let mut statement = transaction.prepare_cached(raw_query.as_str())?;

            let mut params = Vec::new();
            let mut vector = Vec::new();
            for (key, value) in &chunk {
                let value = serde_json::to_string(&value).unwrap();
                let value = value.replace(',', ", ").replace(':', ": ");
                vector.push((key, time, value));
            }
            for batch in vector.iter() {
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

    pub fn is_empty(&self) -> Result<bool> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT EXISTS (SELECT 1 FROM Cache)")?;

        let mut rows = statement.query(())?;
        match rows.next()? {
            None => Ok(true),
            Some(v) => Ok(v.get::<usize, usize>(0).unwrap() == 0),
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
}
