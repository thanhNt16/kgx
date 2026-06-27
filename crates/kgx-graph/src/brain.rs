use std::path::Path;
use rusqlite::Connection;
use kgx_core::{Result, KgError};
use crate::schema::SCHEMA;

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
        conn.execute_batch(SCHEMA).map_err(|e| KgError::Brain(e.to_string()))?;
        Ok(Brain { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}
