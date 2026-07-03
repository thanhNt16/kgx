use crate::migrate;
use crate::vec;
use kgx_core::{KgError, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::Once;

static VEC_INIT: Once = Once::new();

pub struct Brain {
    conn: Connection,
}

// SAFETY: rusqlite::Connection is Send but not Sync because its statement
// cache uses a RefCell internally. Brain is always owned exclusively by a
// single task/thread at a time (opened fresh per call, never shared via
// Arc across concurrent tasks), so a `&Brain` is never dereferenced from
// two threads simultaneously. This unblocks holding `&Brain` across await
// points in async callers (e.g. dream passes) under multi-threaded runtimes.
unsafe impl Sync for Brain {}

impl Brain {
    pub fn open(path: &Path) -> Result<Brain> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        VEC_INIT.call_once(vec::register_global);
        let conn = Connection::open(path).map_err(|e| KgError::Brain(e.to_string()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Brain> {
        VEC_INIT.call_once(vec::register_global);
        let conn = Connection::open_in_memory().map_err(|e| KgError::Brain(e.to_string()))?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Brain> {
        migrate::ensure_schema(&conn)?;
        vec::ensure_vec0_table(&conn)?;
        Ok(Brain { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
