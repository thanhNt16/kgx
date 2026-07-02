use crate::migrate;
use kgx_core::{KgError, Result};
use rusqlite::Connection;
use std::path::Path;

pub struct Brain {
    conn: Connection,
}

impl Brain {
    pub fn open(path: &Path) -> Result<Brain> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|e| KgError::Brain(e.to_string()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Brain> {
        let conn = Connection::open_in_memory().map_err(|e| KgError::Brain(e.to_string()))?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Brain> {
        migrate::ensure_schema(&conn)?;
        Ok(Brain { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
