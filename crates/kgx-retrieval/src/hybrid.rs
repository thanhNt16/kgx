use crate::{ppr::personalized, rrf::fuse};
use kgx_core::{llm::Embedder, Result};
use kgx_graph::{
    knn::vector_search,
    query::{bm25_search, entity_ids},
    Brain,
};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    Keyword,
    Semantic,
    Hybrid,
}
#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub mode: Mode,
    pub limit: usize,
    pub expand_ppr: bool,
    /// Remove entity nodes from final results; they still participate in PPR as graph seeds.
    pub filter_entities: bool,
}
impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            mode: Mode::Hybrid,
            limit: 10,
            expand_ppr: true,
            filter_entities: true,
        }
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub id: String,
    pub score: f32,
    pub signals: Vec<String>,
}

pub fn search(
    brain: &Brain,
    embedder: &dyn Embedder,
    query: &str,
    opts: SearchOpts,
) -> Result<Vec<SearchHit>> {
    let mut rankings: Vec<Vec<String>> = Vec::new();
    let mut signals_for: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    // Retain BM25 top-5 with harmonic weights for PPR seeding.
    // Rank-1 hit gets weight 1.0, rank-2 → 0.5, rank-3 → 0.33, etc.
    // Uniform seeding treats rank-5 as equally confident as rank-1; harmonic weighting
    // concentrates PPR teleportation mass where BM25 is most confident (SPRIG finding).
    let mut bm25_weighted_seeds: Vec<(String, f32)> = Vec::new();
    if matches!(opts.mode, Mode::Keyword | Mode::Hybrid) {
        let bm = bm25_search(brain, query, 50)?;
        for (id, _) in &bm {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("bm25".into());
        }
        bm25_weighted_seeds = bm
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, (id, _))| (id.clone(), 1.0 / (i + 1) as f32))
            .collect();
        rankings.push(bm.into_iter().map(|(id, _)| id).collect());
    }
    if matches!(opts.mode, Mode::Semantic | Mode::Hybrid) {
        let q = embedder.embed(&[query.to_string()])?.remove(0);
        let vec = vector_search(brain, &q, 50)?;
        for (id, _) in &vec {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("vector".into());
        }
        rankings.push(vec.into_iter().map(|(id, _)| id).collect());
    }
    let mut fused = fuse(&rankings, 60.0);
    if opts.expand_ppr && !fused.is_empty() {
        // Prefer BM25-weighted seeds; fall back to fused top-5 with harmonic weights
        // in semantic-only mode.
        let seeds: Vec<(String, f32)> = if !bm25_weighted_seeds.is_empty() {
            bm25_weighted_seeds
        } else {
            fused
                .iter()
                .take(5)
                .enumerate()
                .map(|(i, (id, _))| (id.clone(), 1.0 / (i + 1) as f32))
                .collect()
        };
        let ppr = personalized(brain, &seeds, 0.85, 20)?;
        // Cap PPR contribution to 15 to rebalance RRF mass vs the BM25 list.
        for (id, _) in ppr.iter().take(15) {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("ppr".into());
        }
        fused = fuse(
            &[
                fused.into_iter().map(|(id, _)| id).collect(),
                ppr.into_iter().take(15).map(|(id, _)| id).collect(),
            ],
            60.0,
        );
    }
    if opts.filter_entities {
        let entities = entity_ids(brain)?;
        fused.retain(|(id, _)| !entities.contains(id));
    }
    Ok(fused
        .into_iter()
        .take(opts.limit)
        .map(|(id, score)| {
            let signals = signals_for.get(&id).cloned().unwrap_or_default();
            SearchHit { id, score, signals }
        })
        .collect())
}
