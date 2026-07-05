use kgx_core::{KgError, Result};
use kgx_graph::Brain;
use std::collections::{HashMap, VecDeque};

/// Personalized PageRank with rank-weighted seeds and hop-distance damping.
///
/// `seeds` is a slice of `(node_id, weight)` pairs. Weights need not sum to 1 —
/// they are normalized internally. Higher weight on rank-1 BM25 hits concentrates
/// PPR mass where text retrieval is most confident.
///
/// After PPR converges, each node's score is multiplied by `1 / (min_hop + 1)`
/// where `min_hop` is its BFS distance from the nearest seed. This pushes
/// graph-adjacent relevant nodes to rank 1 and suppresses distant graph noise.
pub fn personalized(
    brain: &Brain,
    seeds: &[(String, f32)],
    damping: f32,
    iters: u32,
) -> Result<Vec<(String, f32)>> {
    let ids: Vec<String> = {
        let mut s = brain
            .conn()
            .prepare("SELECT id FROM notes ORDER BY id")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = s
            .query_map([], |r| r.get(0))
            .map_err(|e| KgError::Brain(e.to_string()))?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| KgError::Brain(e.to_string()))?;
        rows
    };
    let n = ids.len().max(1);
    let index: HashMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    {
        let mut s = brain
            .conn()
            .prepare("SELECT src_id, dst_id FROM edges")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = s
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for row in rows {
            let (a, b) = row.map_err(|e| KgError::Brain(e.to_string()))?;
            if let (Some(&i), Some(&j)) = (index.get(a.as_str()), index.get(b.as_str())) {
                adj[i].push(j);
                adj[j].push(i);
            }
        }
    }

    // Resolve seed ids → (graph_index, weight), drop unknown ids.
    let seed_set: Vec<(usize, f32)> = seeds
        .iter()
        .filter_map(|(id, w)| index.get(id.as_str()).map(|&i| (i, *w)))
        .collect();

    // Build teleport vector with normalized weights (harmonic rank weights from caller).
    let teleport = if seed_set.is_empty() {
        vec![1.0 / n as f32; n]
    } else {
        let total: f32 = seed_set.iter().map(|(_, w)| w).sum();
        let mut t = vec![0.0f32; n];
        for &(idx, w) in &seed_set {
            t[idx] = w / total;
        }
        t
    };

    let mut rank = teleport.clone();
    for _ in 0..iters {
        let mut next = vec![0.0f32; n];
        for i in 0..n {
            next[i] = (1.0 - damping) * teleport[i];
        }
        for i in 0..n {
            if adj[i].is_empty() {
                continue;
            }
            let share = damping * rank[i] / adj[i].len() as f32;
            for &j in &adj[i] {
                next[j] += share;
            }
        }
        rank = next;
    }

    // BFS hop-distance damping: score *= 1/(hop+1).
    // Nodes 1 hop from a seed stay near full score; distant nodes are discounted.
    // Unreachable nodes (no path from any seed) are zeroed.
    if !seed_set.is_empty() {
        let mut hop_dist = vec![u32::MAX; n];
        let mut queue = VecDeque::new();
        for &(idx, _) in &seed_set {
            hop_dist[idx] = 0;
            queue.push_back(idx);
        }
        while let Some(node) = queue.pop_front() {
            let d = hop_dist[node];
            for &neighbor in &adj[node] {
                if hop_dist[neighbor] == u32::MAX {
                    hop_dist[neighbor] = d + 1;
                    queue.push_back(neighbor);
                }
            }
        }
        for i in 0..n {
            rank[i] *= if hop_dist[i] == u32::MAX {
                0.0
            } else {
                1.0 / (hop_dist[i] + 1) as f32
            };
        }
    }

    let mut out: Vec<(String, f32)> = ids.into_iter().zip(rank).collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    Ok(out)
}

/// Pool-scoped Personalized PageRank: runs PPR on a subgraph containing only the
/// given `scope` nodes and edges where both endpoints are in scope.
///
/// `scope` — candidate pool IDs (the retrieved set to rerank).
/// `seeds` — PPR teleport seeds with weights (e.g. BM25 top-5 harmonic weights).
///
/// Useful for "retrieve → rerank" pipelines where the first stage casts a wide
/// net and the second stage reranks via graph proximity.
pub fn personalized_scoped(
    brain: &Brain,
    scope: &[String],
    seeds: &[(String, f32)],
    damping: f32,
    iters: u32,
) -> Result<Vec<(String, f32)>> {
    let n = scope.len();
    if n == 0 {
        return Ok(vec![]);
    }

    // Map scope IDs to dense indices
    let index: HashMap<&str, usize> = scope
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    // Build subgraph adjacency for scope nodes only
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    {
        let mut s = brain
            .conn()
            .prepare("SELECT src_id, dst_id FROM edges")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = s
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for row in rows {
            let (a, b) = row.map_err(|e| KgError::Brain(e.to_string()))?;
            if let (Some(&i), Some(&j)) = (index.get(a.as_str()), index.get(b.as_str())) {
                adj[i].push(j);
                adj[j].push(i);
            }
        }
    }

    // Resolve seed ids → (graph_index, weight), drop seeds outside scope
    let seed_set: Vec<(usize, f32)> = seeds
        .iter()
        .filter_map(|(id, w)| index.get(id.as_str()).map(|&i| (i, *w)))
        .collect();

    let teleport = if seed_set.is_empty() {
        vec![1.0 / n as f32; n]
    } else {
        let total: f32 = seed_set.iter().map(|(_, w)| w).sum();
        let mut t = vec![0.0f32; n];
        for &(idx, w) in &seed_set {
            t[idx] = w / total;
        }
        t
    };

    let mut rank = teleport.clone();
    for _ in 0..iters {
        let mut next = vec![0.0f32; n];
        for i in 0..n {
            next[i] = (1.0 - damping) * teleport[i];
        }
        for i in 0..n {
            if adj[i].is_empty() {
                continue;
            }
            let share = damping * rank[i] / adj[i].len() as f32;
            for &j in &adj[i] {
                next[j] += share;
            }
        }
        rank = next;
    }

    // BFS hop-distance damping
    if !seed_set.is_empty() {
        let mut hop_dist = vec![u32::MAX; n];
        let mut queue = VecDeque::new();
        for &(idx, _) in &seed_set {
            hop_dist[idx] = 0;
            queue.push_back(idx);
        }
        while let Some(node) = queue.pop_front() {
            let d = hop_dist[node];
            for &neighbor in &adj[node] {
                if hop_dist[neighbor] == u32::MAX {
                    hop_dist[neighbor] = d + 1;
                    queue.push_back(neighbor);
                }
            }
        }
        for i in 0..n {
            rank[i] *= if hop_dist[i] == u32::MAX {
                0.0
            } else {
                1.0 / (hop_dist[i] + 1) as f32
            };
        }
    }

    let mut out: Vec<(String, f32)> = scope.iter().cloned().zip(rank).collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    Ok(out)
}

/// HippoRAG-style seed selection: entities whose embedding cosine to the
/// query is >= `threshold`, capped, weighted at half the BM25 harmonic
/// scale (0.5/(i+1)).
pub fn select_entity_seeds(
    scored: &[(String, f32)],
    threshold: f32,
    cap: usize,
) -> Vec<(String, f32)> {
    scored
        .iter()
        .filter(|(_, s)| *s >= threshold)
        .take(cap)
        .enumerate()
        .map(|(i, (id, _))| (id.clone(), 0.5 / (i + 1) as f32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_seeds_filter_cap_and_weight() {
        let scored = vec![
            ("e1".to_string(), 0.9),
            ("e2".to_string(), 0.7),
            ("e3".to_string(), 0.59),
        ];
        let seeds = select_entity_seeds(&scored, 0.60, 5);
        assert_eq!(seeds.len(), 2);
        assert_eq!(seeds[0], ("e1".to_string(), 0.5));
        assert_eq!(seeds[1], ("e2".to_string(), 0.25));
    }

    #[test]
    fn entity_seeds_respect_cap() {
        let scored: Vec<(String, f32)> = (0..10).map(|i| (format!("e{i}"), 0.9)).collect();
        assert_eq!(select_entity_seeds(&scored, 0.60, 5).len(), 5);
    }
}
