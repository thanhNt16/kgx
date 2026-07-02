use crate::schema::SCHEMA_VERSION;
use kgx_core::{KgError, Result};
use rusqlite::Connection;

pub fn ensure_schema(conn: &Connection) -> Result<i32> {
    let has_table: bool = conn
        .query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;

    if !has_table {
        conn.execute_batch(crate::schema::SCHEMA)
            .map_err(|e| KgError::Brain(e.to_string()))?;
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![SCHEMA_VERSION, kgx_core::util::now_iso()],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
        return Ok(SCHEMA_VERSION);
    }

    let current: i32 = conn
        .query_row("SELECT COALESCE(max(version), 0) FROM schema_version", [], |r| {
            r.get(0)
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;

    if current < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![1, kgx_core::util::now_iso()],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    }

    Ok(current.max(1))
}
