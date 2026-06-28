use crate::Brain;
use kgx_core::{KgError, Result};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct CommunityStats {
    pub count: usize,
    pub assignments: BTreeMap<String, i64>,
}

pub fn detect(brain: &mut Brain, seed: u64) -> Result<CommunityStats> {
    let ids: Vec<String> = {
        let mut stmt = brain
            .conn()
            .prepare("SELECT id FROM notes ORDER BY id")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| KgError::Brain(e.to_string()))?
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| KgError::Brain(e.to_string()))?;
        rows
    };
    let index: BTreeMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); ids.len()];
    {
        let mut stmt = brain
            .conn()
            .prepare("SELECT src_id, dst_id FROM edges")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for row in rows {
            let (src, dst) = row.map_err(|e| KgError::Brain(e.to_string()))?;
            if let (Some(&a), Some(&b)) = (index.get(src.as_str()), index.get(dst.as_str())) {
                if a != b {
                    adj[a].push(b);
                    adj[b].push(a);
                }
            }
        }
    }

    let _ = seed;
    let mut comp = vec![-1i64; ids.len()];
    let mut next = 0i64;
    for start in 0..ids.len() {
        if comp[start] != -1 {
            continue;
        }
        comp[start] = next;
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            let mut neighbors = adj[node].clone();
            neighbors.sort_unstable();
            for neighbor in neighbors {
                if comp[neighbor] == -1 {
                    comp[neighbor] = next;
                    stack.push(neighbor);
                }
            }
        }
        next += 1;
    }

    let tx = brain
        .conn_mut()
        .transaction()
        .map_err(|e| KgError::Brain(e.to_string()))?;
    tx.execute("DELETE FROM communities", [])
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let mut assignments = BTreeMap::new();
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "INSERT INTO communities (id, community_id) VALUES (?1, ?2)",
            rusqlite::params![id, comp[i]],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
        assignments.insert(id.clone(), comp[i]);
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(CommunityStats {
        count: next as usize,
        assignments,
    })
}
