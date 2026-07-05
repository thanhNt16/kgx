use crate::embed::f32_to_blob;
use kgx_core::{KgError, Result};
use rusqlite::Connection;

/// Register the sqlite-vec extension globally.
/// Must be called once before any Brain connections are opened.
pub fn register_global() {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
            *const (),
            unsafe extern "C" fn(
                *mut rusqlite::ffi::sqlite3,
                *mut *const std::os::raw::c_char,
                *const rusqlite::ffi::sqlite3_api_routines,
            ) -> i32,
        >(
            sqlite_vec::sqlite3_vec_init as *const ()
        )));
    }
}

/// Create the vec0 virtual table if it doesn't exist.
pub fn ensure_vec0_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS notes_vec USING vec0(embedding float[384]);",
    )
    .map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(())
}

/// Insert or update an embedding for a note in the vec0 table.
/// Maps the string note ID to the integer rowid in the notes table.
pub fn upsert_embedding(conn: &Connection, note_id: &str, embedding: &[f32]) -> Result<()> {
    let blob = f32_to_blob(embedding);
    // vec0 virtual tables don't support INSERT OR REPLACE,
    // so delete first then insert
    conn.execute(
        "DELETE FROM notes_vec WHERE rowid IN (SELECT rowid FROM notes WHERE id = ?1)",
        rusqlite::params![note_id],
    )
    .map_err(|e| KgError::Brain(e.to_string()))?;
    conn.execute(
        "INSERT INTO notes_vec (rowid, embedding) \
         SELECT rowid, ?2 FROM notes WHERE id = ?1",
        rusqlite::params![note_id, blob],
    )
    .map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(())
}

/// Delete an embedding from the vec0 table by note ID.
pub fn delete_embedding(conn: &Connection, note_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM notes_vec WHERE rowid IN (SELECT rowid FROM notes WHERE id = ?1)",
        rusqlite::params![note_id],
    )
    .map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(())
}

/// KNN search via vec0: returns (note_id, distance) sorted by distance ascending.
pub fn knn_search(
    conn: &Connection,
    query_emb: &[f32],
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    let query_blob = f32_to_blob(query_emb);
    // Query vec0 directly for rowids + distances, then look up string IDs
    let vec_sql = format!(
        "SELECT rowid, distance FROM notes_vec WHERE embedding MATCH ?1 ORDER BY distance LIMIT {}",
        limit
    );
    let mut vec_stmt = conn
        .prepare(&vec_sql)
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let vec_rows: Vec<(i64, f32)> = vec_stmt
        .query_map(rusqlite::params![query_blob], |r| {
            let rowid: i64 = r.get(0)?;
            let distance: f32 = r.get(1)?;
            Ok((rowid, distance))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| KgError::Brain(e.to_string()))?;

    // Map rowids to note IDs
    let mut map_stmt = conn
        .prepare("SELECT id FROM notes WHERE rowid = ?1")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let mut results = Vec::with_capacity(vec_rows.len());
    for (rowid, distance) in vec_rows {
        if let Ok(id) = map_stmt.query_row(rusqlite::params![rowid], |r| r.get::<_, String>(0)) {
            // Convert distance to similarity (cosine distance → cosine similarity)
            let similarity = (1.0 - distance).max(0.0);
            results.push((id, similarity));
        }
    }
    Ok(results)
}

/// Check if the vec0 table exists in the database.
pub fn vec0_exists(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='notes_vec'",
        [],
        |r| r.get::<_, i64>(0),
    )
    .map(|c| c > 0)
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Brain;
    use tempfile::tempdir;

    fn insert_test_note(conn: &Connection, id: &str, title: &str) {
        conn.execute(
            "INSERT INTO notes (id, path, type, status, raw_text) VALUES (?1, ?2, 'fact', 'active', ?3)",
            rusqlite::params![id, format!("{}.md", id), title],
        )
        .unwrap();
    }

    #[test]
    fn vec0_table_creation() {
        register_global();
        let dir = tempdir().unwrap();
        let brain = Brain::open(&dir.path().join("test.sqlite")).unwrap();
        let conn = brain.conn();
        ensure_vec0_table(conn).unwrap();
        assert!(vec0_exists(conn));
    }

    #[test]
    fn knn_roundtrip() {
        register_global();
        let dir = tempdir().unwrap();
        let brain = Brain::open(&dir.path().join("test.sqlite")).unwrap();
        let conn = brain.conn();
        ensure_vec0_table(conn).unwrap();
        insert_test_note(conn, "note-a", "Note A");
        insert_test_note(conn, "note-b", "Note B");

        let emb: Vec<f32> = (0..384).map(|i| (i as f32) / 384.0).collect();
        upsert_embedding(conn, "note-a", &emb).unwrap();

        let similar_emb: Vec<f32> = (0..384).map(|i| (i as f32 + 0.01) / 384.0).collect();
        upsert_embedding(conn, "note-b", &similar_emb).unwrap();

        let results = knn_search(conn, &emb, 5).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "note-a");
        assert!(results[0].1 > 0.99); // similarity should be ~1.0 for identical vectors
    }
}
