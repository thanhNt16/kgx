use kgx_core::{llm::Reranker, KgError, Result};
use kgx_graph::Brain;

/// Rerank the top `topk` fused candidates with a cross-encoder; keep the
/// remainder below them in fused order.
pub fn apply_rerank(
    reranker: &dyn Reranker,
    brain: &Brain,
    query: &str,
    fused: Vec<(String, f32)>,
    topk: usize,
) -> Result<Vec<(String, f32)>> {
    if fused.is_empty() || topk == 0 {
        return Ok(fused);
    }
    let head_len = topk.min(fused.len());
    let (head, tail) = fused.split_at(head_len);

    let mut docs: Vec<(String, String)> = Vec::with_capacity(head_len);
    for (id, _) in head {
        let body: String = brain
            .conn()
            .query_row(
                "SELECT raw_text FROM notes WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let snippet: String = body.chars().take(512).collect();
        docs.push((id.clone(), snippet));
    }

    let scores = reranker.rerank(query, &docs)?;
    let mut reranked: Vec<(String, f32)> = docs
        .into_iter()
        .zip(scores)
        .map(|((id, _), s)| (id, s))
        .collect();
    reranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    reranked.extend(tail.iter().cloned());
    Ok(reranked)
}
