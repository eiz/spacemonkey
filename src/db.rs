use std::sync::Arc;

use anyhow::anyhow;
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{de::DeserializeOwned, Serialize};

pub type Database = Arc<DatabaseState>;

pub struct DatabaseState {
    conn: Mutex<Connection>,
}

impl DatabaseState {
    pub fn new() -> anyhow::Result<Self> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("missing $HOME"))?;
        let conn = Mutex::new(Connection::open(home_dir.join(".spacemonkey"))?);
        Ok(Self { conn })
    }

    pub fn update_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        Ok(conn.execute_batch(
            r#"
BEGIN;
CREATE TABLE IF NOT EXISTS keyval (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
COMMIT;
        "#,
        )?)
    }

    pub fn set<S: AsRef<str>, V: ?Sized + Serialize>(&self, key: S, val: &V) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        let mut key_set = conn.prepare_cached("INSERT INTO keyval (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?;
        key_set.execute([key.as_ref(), &serde_json::to_string(val)?])?;
        Ok(())
    }

    pub fn get<S: AsRef<str>, V: DeserializeOwned>(&self, key: S) -> Option<V> {
        let conn = self.conn.lock();
        let stmt = conn.prepare_cached("SELECT value FROM keyval WHERE key = ?");
        if let Ok(mut key_get) = stmt {
            key_get
                .query_row([key.as_ref()], |r| {
                    let val: String = r.get(0)?;
                    Ok(serde_json::from_str(&val)
                        .map_err(|_| rusqlite::Error::QueryReturnedNoRows)?)
                })
                .ok()
        } else {
            None
        }
    }
}

pub fn open() -> anyhow::Result<Database> {
    let result = Arc::new(DatabaseState::new()?);

    result.update_schema()?;
    Ok(result)
}
