use crate::Brain;
use kgx_core::{KgError, Result};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct CommunityStats {
    pub count: usize,
    pub assignments: BTreeMap<String, i64>,
}

fn seeded_shuffle(seed: u64, n: usize) -> Vec<usize> {
    let mut rng = seed;
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        rng ^= rng >> 12;
        rng ^= rng << 25;
        rng ^= rng >> 27;
        let j = (rng as usize) % (i + 1);
        order.swap(i, j);
    }
    order
}

fn modularity_gain(k_i: f64, k_i_in_c: f64, sigma_tot_c: f64, m: f64) -> f64 {
    if m <= 0.0 {
        return 0.0;
    }
    k_i_in_c - k_i * sigma_tot_c / (2.0 * m)
}

const MAX_PASSES: usize = 32;

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
    let n = ids.len();
    if n == 0 {
        return Ok(CommunityStats {
            count: 0,
            assignments: BTreeMap::new(),
        });
    }

    let index: BTreeMap<&str, usize> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut degree = vec![0usize; n];
    let mut total_edges = 0usize;
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
                    degree[a] += 1;
                    degree[b] += 1;
                    total_edges += 1;
                }
            }
        }
    }

    if total_edges == 0 {
        let assignments: BTreeMap<String, i64> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i as i64))
            .collect();
        let tx = brain
            .conn_mut()
            .transaction()
            .map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute("DELETE FROM communities", [])
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for (id, cid) in &assignments {
            tx.execute(
                "INSERT INTO communities (id, community_id) VALUES (?1, ?2)",
                rusqlite::params![id, cid],
            )
            .map_err(|e| KgError::Brain(e.to_string()))?;
        }
        tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
        return Ok(CommunityStats {
            count: n,
            assignments,
        });
    }

    let m = total_edges as f64;
    let mut community: Vec<usize> = (0..n).collect();
    let mut sigma_tot: Vec<f64> = degree.iter().map(|&d| d as f64).collect();

    for pass in 0..MAX_PASSES {
        let order = seeded_shuffle(seed.wrapping_add(pass as u64), n);
        let mut changed = false;

        for &node in &order {
            let cur_comm = community[node];
            let k_i = degree[node] as f64;

            let mut candidates: BTreeMap<usize, f64> = BTreeMap::new();
            for &nb in &adj[node] {
                let c = community[nb];
                if c == cur_comm {
                    continue;
                }
                let gain = candidates.entry(c).or_insert(0.0);
                *gain += 1.0;
            }

            let mut best_comm = cur_comm;
            let mut best_gain = 0.0;

            for (&c, &k_i_in_c) in &candidates {
                let g = modularity_gain(k_i, k_i_in_c, sigma_tot[c], m);
                if g > best_gain {
                    best_gain = g;
                    best_comm = c;
                }
            }

            if best_comm != cur_comm {
                sigma_tot[cur_comm] -= degree[node] as f64;
                sigma_tot[best_comm] += degree[node] as f64;
                community[node] = best_comm;
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    // Renumber communities 0..count
    let mut remap: BTreeMap<usize, i64> = BTreeMap::new();
    let mut next = 0i64;
    let assignments: BTreeMap<String, i64> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let c = community[i];
            let new_id = *remap.entry(c).or_insert_with(|| {
                let v = next;
                next += 1;
                v
            });
            (id.clone(), new_id)
        })
        .collect();

    let tx = brain
        .conn_mut()
        .transaction()
        .map_err(|e| KgError::Brain(e.to_string()))?;
    tx.execute("DELETE FROM communities", [])
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for (id, cid) in &assignments {
        tx.execute(
            "INSERT INTO communities (id, community_id) VALUES (?1, ?2)",
            rusqlite::params![id, cid],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;

    Ok(CommunityStats {
        count: assignments
            .values()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        assignments,
    })
}
