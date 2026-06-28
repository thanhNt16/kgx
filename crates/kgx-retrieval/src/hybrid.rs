use crate::{ppr::personalized, rrf::fuse};
use kgx_core::{llm::Embedder, Result};
use kgx_graph::{knn::vector_search, query::bm25_search, Brain};

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
}
impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            mode: Mode::Hybrid,
            limit: 10,
            expand_ppr: true,
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
    if matches!(opts.mode, Mode::Keyword | Mode::Hybrid) {
        let bm = bm25_search(brain, query, 50)?;
        for (id, _) in &bm {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("bm25".into());
        }
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
        let seeds: Vec<String> = fused.iter().take(5).map(|(id, _)| id.clone()).collect();
        let ppr = personalized(brain, &seeds, 0.85, 20)?;
        for (id, _) in ppr.iter().take(50) {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("ppr".into());
        }
        fused = fuse(
            &[
                fused.into_iter().map(|(id, _)| id).collect(),
                ppr.into_iter().map(|(id, _)| id).collect(),
            ],
            60.0,
        );
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
