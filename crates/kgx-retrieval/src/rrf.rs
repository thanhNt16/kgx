use std::collections::BTreeMap;
pub fn fuse(rankings: &[Vec<String>], k: f32) -> Vec<(String, f32)> {
    let ks: Vec<f32> = std::iter::repeat(k).take(rankings.len()).collect();
    fuse_multi_k(rankings, &ks)
}

/// Fuse multiple rankings where each ranking has its own k value.
/// Higher k = less weight per rank position (dilutes that ranking's contribution).
pub fn fuse_multi_k(rankings: &[Vec<String>], ks: &[f32]) -> Vec<(String, f32)> {
    let mut scores: BTreeMap<String, f32> = BTreeMap::new();
    for (ranking_idx, ranking) in rankings.iter().enumerate() {
        let k = ks.get(ranking_idx).copied().unwrap_or(60.0);
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
        assert!(fused.iter().any(|(id, _)| id == "x"));
        assert!(fused.iter().any(|(id, _)| id == "w"));
    }
    #[test]
    fn higher_k_dilutes_ranking() {
        let a = vec!["x".to_string(), "y".into()];
        let b = vec!["x".to_string(), "y".into()];
        let fused_high = fuse_multi_k(&[a.clone(), b.clone()], &[300.0, 60.0]);
        let fused_eq = fuse_multi_k(&[a, b], &[60.0, 60.0]);
        // With higher k for first ranking, x should have lower score in fused_high
        let score_high = fused_high
            .iter()
            .find(|(id, _)| id == "x")
            .map(|(_, s)| *s)
            .unwrap();
        let score_eq = fused_eq
            .iter()
            .find(|(id, _)| id == "x")
            .map(|(_, s)| *s)
            .unwrap();
        assert!(score_high < score_eq);
    }
}
