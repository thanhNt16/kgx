use crate::Brain;
use kgx_core::llm::{SparseEmbedder, SparseVec};
use kgx_core::{KgError, Note, Result};
use std::collections::HashMap;

pub fn replace_sparse(conn: &rusqlite::Connection, note_id: &str, sv: &SparseVec) -> Result<()> {
    conn.execute(
        "DELETE FROM sparse_postings WHERE note_id=?1",
        rusqlite::params![note_id],
    )
    .map_err(|e| KgError::Brain(e.to_string()))?;
    let mut stmt = conn
        .prepare(
            "INSERT OR REPLACE INTO sparse_postings (term_id, note_id, weight) VALUES (?1, ?2, ?3)",
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for (term, w) in sv {
        stmt.execute(rusqlite::params![term, note_id, w])
            .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    Ok(())
}

/// Dot-product scoring over the inverted index: one indexed lookup per
/// query term, accumulated in memory (queries have ~20–100 terms).
pub fn sparse_search(
    brain: &Brain,
    query_sparse: &SparseVec,
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    if query_sparse.is_empty() {
        return Ok(vec![]);
    }
    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut stmt = brain
        .conn()
        .prepare("SELECT note_id, weight FROM sparse_postings WHERE term_id=?1")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for (term, qw) in query_sparse {
        let rows = stmt
            .query_map(rusqlite::params![term], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)? as f32))
            })
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for row in rows {
            let (id, w) = row.map_err(|e| KgError::Brain(e.to_string()))?;
            *scores.entry(id).or_insert(0.0) += qw * w;
        }
    }
    let mut out: Vec<(String, f32)> = scores.into_iter().collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    out.truncate(limit);
    Ok(out)
}

/// Embed and store sparse postings for `notes`. Returns notes indexed.
pub fn index_sparse(brain: &Brain, notes: &[&Note], sparse: &dyn SparseEmbedder) -> Result<usize> {
    if notes.is_empty() {
        return Ok(0);
    }
    let texts: Vec<String> = notes
        .iter()
        .map(|n| format!("{}\n{}", n.fm.title, n.body))
        .collect();
    let vecs = sparse.embed_sparse(&texts)?;
    for (n, sv) in notes.iter().zip(&vecs) {
        replace_sparse(brain.conn(), &n.fm.id, sv)?;
    }
    Ok(notes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Brain;

    fn brain_with_temp() -> (Brain, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let brain = Brain::open(&dir.path().join("b.sqlite")).unwrap();
        (brain, dir)
    }

    #[test]
    fn dot_product_scoring_hand_computed() {
        let (brain, _dir) = brain_with_temp();
        replace_sparse(brain.conn(), "X", &vec![(1, 0.5), (2, 2.0)]).unwrap();
        replace_sparse(brain.conn(), "Y", &vec![(2, 1.0)]).unwrap();
        let hits = sparse_search(&brain, &vec![(1, 1.0), (2, 1.0)], 10).unwrap();
        assert_eq!(hits[0].0, "X");
        assert!((hits[0].1 - 2.5).abs() < 1e-6);
        assert_eq!(hits[1].0, "Y");
        assert!((hits[1].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn replace_overwrites_previous_postings() {
        let (brain, _dir) = brain_with_temp();
        replace_sparse(brain.conn(), "X", &vec![(1, 1.0)]).unwrap();
        replace_sparse(brain.conn(), "X", &vec![(9, 1.0)]).unwrap();
        assert!(sparse_search(&brain, &vec![(1, 1.0)], 10)
            .unwrap()
            .is_empty());
        assert_eq!(sparse_search(&brain, &vec![(9, 1.0)], 10).unwrap().len(), 1);
    }
}
