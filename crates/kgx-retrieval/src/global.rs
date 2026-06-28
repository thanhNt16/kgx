use kgx_core::{llm::Embedder, KgError, Result};
use kgx_graph::Brain;
use std::cmp::Reverse;

pub fn global_context(
    brain: &Brain,
    query: &str,
    _embedder: &dyn Embedder,
    limit: usize,
) -> Result<String> {
    let mut rows: Vec<(i64, String, String)> = {
        let mut stmt = brain
            .conn()
            .prepare("SELECT community_id, title, summary FROM community_summaries")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| KgError::Brain(e.to_string()))?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| KgError::Brain(e.to_string()))?;
        rows
    };
    let q = query.to_lowercase();
    rows.sort_by_key(|(_, title, summary)| {
        let hay = format!("{title} {summary}").to_lowercase();
        Reverse(
            q.split_whitespace()
                .filter(|word| hay.contains(*word))
                .count(),
        )
    });
    Ok(rows
        .into_iter()
        .take(limit)
        .map(|(id, title, summary)| format!("[community {id}] {title}: {summary}"))
        .collect::<Vec<_>>()
        .join("\n"))
}
