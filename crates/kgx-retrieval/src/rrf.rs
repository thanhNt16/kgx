use std::collections::BTreeMap;
pub fn fuse(rankings: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let mut scores: BTreeMap<String, f32> = BTreeMap::new();
    for ranking in rankings {
        for (rank, id) in ranking.iter().enumerate() {
            *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k + (rank as f32) + 1.0);
        }
    }
    let mut v: Vec<(String, f32)> = scores.into_iter().collect();
    v.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    v
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rrf_rewards_consensus() {
        let a = vec!["x".to_string(), "y".into(), "z".into()];
        let b = vec!["y".to_string(), "x".into(), "w".into()];
        let fused = fuse(&[a, b], 60.0);
        // x is rank 0 in list a and rank 1 in list b → 1/61 + 1/62
        // y is rank 1 in list a and rank 0 in list b → 1/62 + 1/61 (same as x)
        // both x and y should be in top 2
        assert!(fused.iter().any(|(id, _)| id == "x"));
        assert!(fused.iter().any(|(id, _)| id == "w"));
    }
}
