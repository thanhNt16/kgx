use std::path::Path;

use kgx_core::{Edge, KgError, Note, Result};

use crate::build::BuildStats;
use crate::embed::f32_to_blob;

pub struct GraphBuffer {
    pub notes: Vec<Note>,
    pub edges: Vec<Edge>,
}

impl GraphBuffer {
    pub fn new(notes: Vec<Note>) -> Self {
        let edges = crate::build::derive_edges(&notes);
        GraphBuffer { notes, edges }
    }

    /// Bulk-write all notes and edges to a SQLite brain with maximal throughput.
    /// Strategy (mirrored from codebase-memory-mcp's graph_buffer + dump pipeline):
    ///   1. Drop indexes (faster INSERTs)
    ///   2. synchronous=OFF, 64 MB cache
    ///   3. Single transaction
    ///   4. Bulk INSERT notes + edges
    ///   5. Commit, recreate indexes, restore synchronous
    ///   6. vec0 inserts (must be after commit)
    pub fn write_to_sqlite(
        self,
        db_path: &Path,
        embedder: &dyn kgx_core::llm::Embedder,
    ) -> Result<BuildStats> {
        let node_count = self.notes.len();
        let edge_count = self.edges.len();
        let t0 = std::time::Instant::now();

        // --- Step 1: Generate embeddings ---
        let texts: Vec<String> = self
            .notes
            .iter()
            .map(|n| format!("{}\n{}", n.fm.title, n.body))
            .collect();
        let embeddings = embedder.embed(&texts)?;
        eprintln!("  TIMING step1 embed: {:?}", t0.elapsed());

        // --- Step 2: Open brain with full write optimizations ---
        let t1 = std::time::Instant::now();
        let mut conn = open_brain_bulk(db_path)?;
        eprintln!("  TIMING step2 open: {:?}", t1.elapsed());

        // --- Step 3: Drop indexes for bulk INSERT ---
        let t2 = std::time::Instant::now();
        conn.execute_batch(
            "DROP INDEX IF EXISTS idx_notes_entity_type;
             DROP INDEX IF EXISTS idx_notes_type;
             DROP INDEX IF EXISTS idx_edges_rel_type;
             DROP INDEX IF EXISTS idx_edges_src;
             DROP INDEX IF EXISTS idx_edges_dst;
             DROP INDEX IF EXISTS idx_edges_src_rel;",
        )
        .map_err(|e| KgError::Brain(format!("drop indexes: {e}")))?;
        eprintln!("  TIMING step3 drop indexes: {:?}", t2.elapsed());

        // --- Step 4: Delete existing data ---
        let t3 = std::time::Instant::now();
        conn.execute_batch(
            "DELETE FROM notes; DELETE FROM edges; DELETE FROM notes_fts; \
             DELETE FROM pagerank; DELETE FROM communities; DELETE FROM community_summaries; \
             DELETE FROM notes_vec; DELETE FROM sparse_postings;",
        )
        .map_err(|e| KgError::Brain(format!("delete: {e}")))?;
        eprintln!("  TIMING step4 delete: {:?}", t3.elapsed());

        // --- Step 5: Bulk INSERT notes ---
        let t4 = std::time::Instant::now();
        let tx = conn
            .transaction()
            .map_err(|e| KgError::Brain(format!("tx begin: {e}")))?;

        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO notes (id,path,title,type,status,valid_from,valid_to,recorded_at,tags,raw_text,embedding,entity_type)\
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                )
                .map_err(|e| KgError::Brain(e.to_string()))?;

            for (n, emb) in self.notes.iter().zip(&embeddings) {
                let tags = serde_json::to_string(&{
                    let mut t = n.fm.tags.clone();
                    t.sort();
                    t
                })
                .unwrap();
                let typ = serde_json::to_string(&n.fm.r#type)
                    .unwrap()
                    .trim_matches('"')
                    .to_string();
                let st = serde_json::to_string(&n.fm.status)
                    .unwrap()
                    .trim_matches('"')
                    .to_string();
                stmt.execute(rusqlite::params![
                    n.fm.id,
                    n.rel_path.display().to_string(),
                    n.fm.title,
                    typ,
                    st,
                    n.fm.valid_from,
                    n.fm.valid_to,
                    n.fm.recorded_at,
                    tags,
                    n.body,
                    f32_to_blob(emb),
                    n.fm.entity_type.as_deref()
                ])
                .map_err(|e| KgError::Brain(e.to_string()))?;
            }
        }
        eprintln!("  TIMING step5 insert notes: {:?}", t4.elapsed());

        // --- Step 6: Bulk INSERT edges ---
        let t5 = std::time::Instant::now();
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR IGNORE INTO edges (src_id,dst_id,rel_type,valid_from,valid_to) VALUES (?1,?2,?3,?4,?5)",
                )
                .map_err(|e| KgError::Brain(e.to_string()))?;

            for e in &self.edges {
                let rt = serde_json::to_string(&e.rel_type)
                    .unwrap()
                    .trim_matches('"')
                    .to_string();
                stmt.execute(rusqlite::params![
                    e.src_id,
                    e.dst_id,
                    rt,
                    e.valid_from,
                    e.valid_to
                ])
                .map_err(|e| KgError::Brain(e.to_string()))?;
            }
        }
        eprintln!("  TIMING step6 insert edges: {:?}", t5.elapsed());

        let t6 = std::time::Instant::now();
        tx.commit()
            .map_err(|e| KgError::Brain(format!("tx commit: {e}")))?;
        eprintln!("  TIMING step6b commit: {:?}", t6.elapsed());

        // --- Step 7: Recreate indexes ---
        let t7 = std::time::Instant::now();
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_notes_entity_type ON notes(entity_type);
             CREATE INDEX IF NOT EXISTS idx_notes_type ON notes(type);
             CREATE INDEX IF NOT EXISTS idx_edges_rel_type ON edges(rel_type);
             CREATE INDEX IF NOT EXISTS idx_edges_src ON edges(src_id);
             CREATE INDEX IF NOT EXISTS idx_edges_dst ON edges(dst_id);
             CREATE INDEX IF NOT EXISTS idx_edges_src_rel ON edges(src_id,rel_type);",
        )
        .map_err(|e| KgError::Brain(format!("create indexes: {e}")))?;
        eprintln!("  TIMING step7 create indexes: {:?}", t7.elapsed());

        // --- Step 8: vec0 inserts (must happen after commit) ---
        let t8 = std::time::Instant::now();
        drop(conn); // close before reopening for vec0
        let brain = crate::Brain::open(db_path)?;
        for (n, emb) in self.notes.iter().zip(&embeddings) {
            crate::vec::upsert_embedding(brain.conn(), &n.fm.id, emb)
                .map_err(|e| KgError::Brain(format!("vec0 insert: {e}")))?;
        }
        eprintln!("  TIMING step8 vec0: {:?}", t8.elapsed());

        // --- Step 9: FTS re-population ---
        let t9 = std::time::Instant::now();
        {
            let conn_final = crate::Brain::open(db_path)?;
            let mut stmt = conn_final
                .conn()
                .prepare("INSERT INTO notes_fts (id, raw_text, tags) VALUES (?1,?2,?3)")
                .map_err(|e| KgError::Brain(e.to_string()))?;
            for n in &self.notes {
                let tags = serde_json::to_string(&{
                    let mut t = n.fm.tags.clone();
                    t.sort();
                    t
                })
                .unwrap();
                stmt.execute(rusqlite::params![n.fm.id, n.body, tags])
                    .map_err(|e| KgError::Brain(e.to_string()))?;
            }
        }
        eprintln!("  TIMING step9 fts: {:?}", t9.elapsed());

        // --- Step 10: Write meta ---
        let t10 = std::time::Instant::now();
        write_bulk_meta(db_path, node_count, edge_count)?;
        eprintln!("  TIMING step10 meta: {:?}", t10.elapsed());

        eprintln!("  TIMING TOTAL: {:?}", t0.elapsed());

        Ok(BuildStats {
            nodes: node_count,
            edges: edge_count,
            embedded: embeddings.len(),
        })
    }
}

fn open_brain_bulk(db_path: &Path) -> Result<rusqlite::Connection> {
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    crate::vec::register_global();
    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| KgError::Brain(format!("open bulk: {e}")))?;

    // WAL mode + ultra-bulk pragmas
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=OFF;
         PRAGMA cache_size=-65536;
         PRAGMA temp_store=MEMORY;
         PRAGMA busy_timeout=30000;",
    )
    .map_err(|e| KgError::Brain(format!("bulk pragmas: {e}")))?;

    crate::migrate::ensure_schema(&conn)?;
    crate::vec::ensure_vec0_table(&conn)?;

    Ok(conn)
}

fn write_bulk_meta(db_path: &Path, node_count: usize, edge_count: usize) -> Result<()> {
    let mut brain = crate::Brain::open(db_path)?;
    let meta_tx = brain
        .conn_mut()
        .transaction()
        .map_err(|e| KgError::Brain(e.to_string()))?;
    meta_tx
        .execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('last_index', ?1)",
            rusqlite::params![kgx_core::util::now_iso()],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    meta_tx
        .execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('node_count', ?1)",
            rusqlite::params![node_count.to_string()],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    meta_tx
        .execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('edge_count', ?1)",
            rusqlite::params![edge_count.to_string()],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    meta_tx
        .execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('build_mode', ?1)",
            rusqlite::params!["bulk"],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    meta_tx
        .commit()
        .map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(())
}
