use crate::{embed::blob_to_f32, vec, Brain};
use kgx_core::{KgError, Result};

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

fn brute_force_search(
    brain: &Brain,
    query_emb: &[f32],
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    let mut stmt = brain
        .conn()
        .prepare("SELECT id, embedding FROM notes WHERE embedding IS NOT NULL")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let blob: Vec<u8> = r.get(1)?;
            Ok((id, blob))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let mut scored: Vec<(String, f32)> = Vec::new();
    for row in rows {
        let (id, blob) = row.map_err(|e| KgError::Brain(e.to_string()))?;
        scored.push((id, cosine(query_emb, &blob_to_f32(&blob))));
    }
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    scored.truncate(limit);
    Ok(scored)
}

pub fn vector_search(brain: &Brain, query_emb: &[f32], limit: usize) -> Result<Vec<(String, f32)>> {
    if vec::vec0_exists(brain.conn()) {
        return vec::knn_search(brain.conn(), query_emb, limit);
    }
    brute_force_search(brain, query_emb, limit)
}
