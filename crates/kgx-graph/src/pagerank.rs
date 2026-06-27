use std::collections::HashMap;
use petgraph::graph::{DiGraph, NodeIndex};
use kgx_core::{Result, KgError};
use crate::Brain;

pub fn compute(brain: &mut Brain, damping: f32, iters: u32) -> Result<()> {
    let mut g: DiGraph<String, ()> = DiGraph::new();
    let mut idx: HashMap<String, NodeIndex> = HashMap::new();
    {
        let mut s = brain
            .conn()
            .prepare("SELECT id FROM notes ORDER BY id")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = s
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for r in rows {
            let id = r.map_err(|e| KgError::Brain(e.to_string()))?;
            let n = g.add_node(id.clone());
            idx.insert(id, n);
        }
    }
    {
        let mut s = brain
            .conn()
            .prepare("SELECT src_id, dst_id FROM edges")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = s
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for r in rows {
            let (a, b) = r.map_err(|e| KgError::Brain(e.to_string()))?;
            if let (Some(&x), Some(&y)) = (idx.get(&a), idx.get(&b)) {
                g.add_edge(x, y, ());
            }
        }
    }
    let n = g.node_count().max(1) as f32;
    let mut rank: HashMap<NodeIndex, f32> = g.node_indices().map(|i| (i, 1.0 / n)).collect();
    for _ in 0..iters {
        let mut next: HashMap<NodeIndex, f32> = g
            .node_indices()
            .map(|i| (i, (1.0 - damping) / n))
            .collect();
        for node in g.node_indices() {
            let out: Vec<_> = g
                .neighbors_directed(node, petgraph::Direction::Outgoing)
                .collect();
            if out.is_empty() {
                continue;
            }
            let share = damping * rank[&node] / out.len() as f32;
            for o in out {
                *next.get_mut(&o).unwrap() += share;
            }
        }
        rank = next;
    }
    let tx = brain
        .conn_mut()
        .transaction()
        .map_err(|e| KgError::Brain(e.to_string()))?;
    tx.execute("DELETE FROM pagerank", [])
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for (ni, score) in &rank {
        tx.execute(
            "INSERT INTO pagerank (id, score) VALUES (?1, ?2)",
            rusqlite::params![g[*ni], *score as f64],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(())
}
