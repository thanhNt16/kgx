use kgx_core::{Result, KgError};
use crate::Brain;

pub fn bm25_search(brain: &Brain, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
    let mut stmt = brain
        .conn()
        .prepare(
            "SELECT id, bm25(notes_fts) AS score FROM notes_fts \
             WHERE notes_fts MATCH ?1 ORDER BY score LIMIT ?2",
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params![query, limit as i64], |r| {
            let id: String = r.get(0)?;
            let score: f64 = r.get(1)?;
            Ok((id, -score as f32))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| KgError::Brain(e.to_string()))
}

pub fn neighbors(brain: &Brain, id: &str, hops: u32) -> Result<Vec<String>> {
    use std::collections::BTreeSet;
    let mut frontier: BTreeSet<String> = BTreeSet::from([id.to_string()]);
    let mut seen = frontier.clone();
    for _ in 0..hops {
        let mut next = BTreeSet::new();
        for node in &frontier {
            let mut stmt = brain
                .conn()
                .prepare(
                    "SELECT dst_id FROM edges WHERE src_id=?1 \
                     UNION SELECT src_id FROM edges WHERE dst_id=?1",
                )
                .map_err(|e| KgError::Brain(e.to_string()))?;
            let rows = stmt
                .query_map([node], |r| r.get::<_, String>(0))
                .map_err(|e| KgError::Brain(e.to_string()))?;
            for r in rows {
                let n = r.map_err(|e| KgError::Brain(e.to_string()))?;
                if seen.insert(n.clone()) {
                    next.insert(n);
                }
            }
        }
        frontier = next;
    }
    let mut out: Vec<String> = seen.into_iter().filter(|n| n != id).collect();
    out.sort();
    Ok(out)
}
