use crate::Brain;
use kgx_core::{KgError, Result};

pub fn bm25_search(brain: &Brain, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
    // Sanitize query for FTS5: strip punctuation that causes syntax errors,
    // then collect non-empty tokens as individual terms.
    let sanitized: String = query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let tokens: Vec<&str> = sanitized.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(vec![]);
    }
    // OR semantics: BM25 scoring naturally rewards docs with more matching terms.
    // AND (the FTS5 default) kills recall on multi-token queries.
    let fts_query = tokens.join(" OR ");
    let mut stmt = brain
        .conn()
        .prepare(
            "SELECT id, bm25(notes_fts) AS score FROM notes_fts \
             WHERE notes_fts MATCH ?1 ORDER BY score LIMIT ?2",
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params![fts_query, limit as i64], |r| {
            let id: String = r.get(0)?;
            let score: f64 = r.get(1)?;
            Ok((id, -score as f32))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| KgError::Brain(e.to_string()))
}

pub fn entity_ids(brain: &Brain) -> Result<std::collections::HashSet<String>> {
    let mut stmt = brain
        .conn()
        .prepare("SELECT id FROM notes WHERE type = 'entity'")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    rows.collect::<std::result::Result<_, _>>()
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
