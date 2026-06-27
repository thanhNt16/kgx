use std::collections::HashMap;
use kgx_core::{Result, KgError};
use kgx_graph::Brain;
pub fn personalized(brain: &Brain, seeds: &[String], damping: f32, iters: u32) -> Result<Vec<(String, f32)>> {
    let ids: Vec<String> = {
        let mut s = brain.conn().prepare("SELECT id FROM notes ORDER BY id").map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = s.query_map([], |r| r.get(0)).map_err(|e| KgError::Brain(e.to_string()))?
            .collect::<std::result::Result<_,_>>().map_err(|e| KgError::Brain(e.to_string()))?;
        rows
    };
    let n = ids.len().max(1);
    let index: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    {
        let mut s = brain.conn().prepare("SELECT src_id, dst_id FROM edges").map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = s.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?))).map_err(|e| KgError::Brain(e.to_string()))?;
        for row in rows {
            let (a, b) = row.map_err(|e| KgError::Brain(e.to_string()))?;
            if let (Some(&i), Some(&j)) = (index.get(a.as_str()), index.get(b.as_str())) {
                adj[i].push(j); adj[j].push(i);
            }
        }
    }
    let seed_set: Vec<usize> = seeds.iter().filter_map(|s| index.get(s.as_str()).copied()).collect();
    let teleport = if seed_set.is_empty() { vec![1.0 / n as f32; n] }
        else { let mut t = vec![0.0; n]; for &s in &seed_set { t[s] = 1.0 / seed_set.len() as f32; } t };
    let mut rank = teleport.clone();
    for _ in 0..iters {
        let mut next = vec![0.0; n];
        for i in 0..n { next[i] = (1.0 - damping) * teleport[i]; }
        for i in 0..n {
            if adj[i].is_empty() { continue; }
            let share = damping * rank[i] / adj[i].len() as f32;
            for &j in &adj[i] { next[j] += share; }
        }
        rank = next;
    }
    let mut out: Vec<(String, f32)> = ids.into_iter().zip(rank).collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    Ok(out)
}
