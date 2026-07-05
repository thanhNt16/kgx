use crate::{ppr::personalized, ppr::personalized_scoped, rrf::fuse_multi_k};
use kgx_core::{
    llm::{Embedder, LlmProvider, LlmRequest, Reranker, SparseEmbedder},
    Result,
};
use kgx_graph::{
    knn::vector_search,
    query::{bm25_search, entity_ids, extract_tokens, like_search, tag_expansion},
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
    /// When true, keyword mode uses a two-stage retrieve → graph rerank pipeline
    /// instead of the fused RRF approach.
    pub rerank_graph: bool,
    /// When true, results from the fused RRF pipeline are reranked by the LLM
    /// (semantic relevance scoring).
    pub rerank_llm: bool,
    /// How many fused candidates to pass through the cross-encoder reranker.
    /// 0 disables the rerank stage entirely.
    pub rerank_topk: usize,
}
impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            mode: Mode::Hybrid,
            limit: 10,
            expand_ppr: true,
            filter_entities: false,
            rerank_graph: false,
            rerank_llm: false,
            rerank_topk: 30,
        }
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub id: String,
    pub score: f32,
    pub signals: Vec<String>,
}

/// Bundle of model handles for the search pipeline. Only `embedder` is
/// required; every optional stage degrades to a no-op when absent.
pub struct Retrievers<'a> {
    pub embedder: &'a dyn Embedder,
    pub llm: Option<&'a dyn LlmProvider>,
    pub reranker: Option<&'a dyn Reranker>,
    pub sparse: Option<&'a dyn SparseEmbedder>,
}

impl<'a> Retrievers<'a> {
    pub fn new(embedder: &'a dyn Embedder) -> Self {
        Self {
            embedder,
            llm: None,
            reranker: None,
            sparse: None,
        }
    }
    pub fn with_llm(mut self, llm: Option<&'a dyn LlmProvider>) -> Self {
        self.llm = llm;
        self
    }
    pub fn with_reranker(mut self, reranker: Option<&'a dyn Reranker>) -> Self {
        self.reranker = reranker;
        self
    }
    pub fn with_sparse(mut self, sparse: Option<&'a dyn SparseEmbedder>) -> Self {
        self.sparse = sparse;
        self
    }
}

pub fn search(
    brain: &Brain,
    r: &Retrievers,
    query: &str,
    opts: SearchOpts,
) -> Result<Vec<SearchHit>> {
    // Retrieve → graph rerank pipeline (bypasses fused RRF)
    if opts.rerank_graph && matches!(opts.mode, Mode::Keyword) {
        return search_rerank_graph(brain, query, &opts);
    }

    let mut rankings: Vec<Vec<String>> = Vec::new();
    let mut ks: Vec<f32> = Vec::new();
    let mut signals_for: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut bm25_weighted_seeds: Vec<(String, f32)> = Vec::new();
    if matches!(opts.mode, Mode::Keyword | Mode::Hybrid) {
        // Ranking 1: BM25 FTS5
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
        let bm_ids: Vec<String> = bm.iter().map(|(id, _)| id.clone()).collect();
        rankings.push(bm_ids.clone());
        ks.push(60.0);

        // Ranking 2: LIKE substring
        let tokens = extract_tokens(query);
        if tokens.len() > 1 {
            let like = like_search(brain, &tokens, 50)?;
            if !like.is_empty() {
                for (id, _) in &like {
                    signals_for
                        .entry(id.clone())
                        .or_default()
                        .push("like".into());
                }
                rankings.push(like.into_iter().map(|(id, _)| id).collect());
                ks.push(60.0);
            }
        }

        // Ranking 3: Tag-based expansion
        // Diluted weight (k=300) so tag contribution is ~0.003-0.005 per item,
        // enough to boost orphans via shared tags without overwhelming primary signals.
        // Cap at 5 items to limit noise from generic tags.
        let source: Vec<String> = bm_ids.iter().take(10).cloned().collect();
        let source_set: std::collections::HashSet<&str> =
            source.iter().map(|s| s.as_str()).collect();
        let like_set: std::collections::HashSet<&str> = rankings
            .get(1)
            .map(|r| r.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        if let Ok(tag) = tag_expansion(brain, &source, 50) {
            let tag: Vec<(String, f32)> = tag
                .into_iter()
                .filter(|(id, _)| {
                    !source_set.contains(id.as_str()) && like_set.contains(id.as_str())
                })
                .take(5)
                .collect();
            if !tag.is_empty() {
                for (id, _) in &tag {
                    signals_for
                        .entry(id.clone())
                        .or_default()
                        .push("tag".into());
                }
                rankings.push(tag.into_iter().map(|(id, _)| id).collect());
                ks.push(300.0);
            }
        }

        // Ranking 4: SPLADE sparse (learned term expansion).
        if let Some(sparse) = r.sparse {
            match sparse.embed_sparse(&[query.to_string()]) {
                Ok(mut qv) if !qv.is_empty() => {
                    let q = qv.remove(0);
                    if let Ok(sp) = kgx_graph::sparse::sparse_search(brain, &q, 50) {
                        if !sp.is_empty() {
                            for (id, _) in &sp {
                                signals_for
                                    .entry(id.clone())
                                    .or_default()
                                    .push("sparse".into());
                            }
                            rankings.push(sp.into_iter().map(|(id, _)| id).collect());
                            ks.push(60.0);
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("warning: sparse query embed failed, stage skipped: {e}"),
            }
        }
    }
    let mut query_emb: Option<Vec<f32>> = None;
    if matches!(opts.mode, Mode::Semantic | Mode::Hybrid) && r.embedder.is_semantic() {
        let q = r.embedder.embed(&[query.to_string()])?.remove(0);
        query_emb = Some(q.clone());
        let vec = vector_search(brain, &q, 50)?;
        for (id, _) in &vec {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("vector".into());
        }
        rankings.push(vec.into_iter().map(|(id, _)| id).collect());
        ks.push(60.0);
    }
    let mut fused = fuse_multi_k(&rankings, &ks);
    if opts.expand_ppr && !fused.is_empty() && kgx_graph::query::has_edges(brain)? {
        let mut seeds: Vec<(String, f32)> = if !bm25_weighted_seeds.is_empty() {
            bm25_weighted_seeds
        } else {
            fused
                .iter()
                .take(5)
                .enumerate()
                .map(|(i, (id, _))| (id.clone(), 1.0 / (i + 1) as f32))
                .collect()
        };
        if let Some(q) = &query_emb {
            if let Ok(scored) = kgx_graph::knn::entity_scores(brain, q) {
                seeds.extend(crate::ppr::select_entity_seeds(&scored, 0.60, 5));
            }
        }
        let ppr = personalized(brain, &seeds, 0.85, 20)?;
        for (id, _) in ppr.iter().take(15) {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("ppr".into());
        }
        fused = fuse_multi_k(
            &[
                fused.into_iter().map(|(id, _)| id).collect(),
                ppr.into_iter().take(15).map(|(id, _)| id).collect(),
            ],
            &[60.0, 60.0],
        );
    }
    if opts.filter_entities {
        let entities = entity_ids(brain)?;
        fused.retain(|(id, _)| !entities.contains(id));
    }

    // Stage 4: local cross-encoder rerank of the fused head.
    if let Some(reranker) = r.reranker {
        let head = opts.rerank_topk.min(fused.len());
        fused = crate::rerank::apply_rerank(reranker, brain, query, fused, opts.rerank_topk)?;
        for (id, _) in fused.iter().take(head) {
            signals_for
                .entry(id.clone())
                .or_default()
                .push("rerank".into());
        }
    }

    // Optional LLM reranker: reorder fused results by LLM relevance scores.
    if opts.rerank_llm {
        if let Some(llm) = r.llm {
            fused = llm_rerank(brain, llm, query, &fused, opts.limit)?;
        }
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

/// Two-stage retrieve → graph rerank pipeline for keyword mode.
///
/// Stage 1: BM25 + LIKE with wider limits (100 each), unioned into a pool.
/// Stage 2: Pool-scoped PPR from BM25 top-5 harmonic-weighted seeds, with hop damping.
fn search_rerank_graph(brain: &Brain, query: &str, opts: &SearchOpts) -> Result<Vec<SearchHit>> {
    use std::collections::BTreeMap;

    // Stage 1: Retrieve wide pool
    let bm25 = bm25_search(brain, query, 100)?;
    let mut pool: BTreeMap<String, f32> = BTreeMap::new();
    for (id, _) in &bm25 {
        pool.insert(id.clone(), 0.0);
    }
    let tokens = extract_tokens(query);
    if tokens.len() > 1 {
        if let Ok(like) = like_search(brain, &tokens, 100) {
            for (id, _) in &like {
                pool.entry(id.clone()).or_insert(0.0);
            }
        }
    }

    // Stage 2: Rerank via pool-scoped PPR
    let pool_ids: Vec<String> = pool.keys().cloned().collect();
    let seeds: Vec<(String, f32)> = bm25
        .iter()
        .take(5)
        .enumerate()
        .map(|(i, (id, _))| (id.clone(), 1.0 / (i + 1) as f32))
        .collect();
    let ppr = personalized_scoped(brain, &pool_ids, &seeds, 0.85, 20)?;
    let ppr_map: BTreeMap<String, f32> = ppr.into_iter().collect();

    let mut hits: Vec<SearchHit> = pool_ids
        .into_iter()
        .filter_map(|id| {
            ppr_map.get(&id).map(|&score| SearchHit {
                id,
                score,
                signals: vec!["retrieve".into(), "graph".into()],
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits.truncate(opts.limit);
    Ok(hits)
}

/// LLM reranker: take the top candidates from fused RRF, ask the LLM to score each
/// for relevance to the query, then reorder by LLM score.
///
/// Prompt format includes each note's title + first ~300 chars of body + tags,
/// followed by a request for 0-5 relevance scores for each candidate.
fn llm_rerank(
    brain: &Brain,
    llm: &dyn LlmProvider,
    query: &str,
    fused: &[(String, f32)],
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    let top_k = fused.iter().take(limit.max(20)).collect::<Vec<_>>();
    if top_k.is_empty() {
        return Ok(fused.to_vec());
    }

    // Fetch note content for each candidate
    let mut candidates: Vec<(String, String)> = Vec::new();
    for (id, _) in &top_k {
        let body: String = brain
            .conn()
            .query_row(
                "SELECT raw_text FROM notes WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
        candidates.push((id.clone(), body));
    }

    // Build prompt
    let mut prompt = String::from("RERANK\n");
    prompt.push_str(&format!("Query: {query}\n\nCandidates:\n"));
    for (i, (id, body)) in candidates.iter().enumerate() {
        let snippet: String = body.chars().take(300).collect();
        prompt.push_str(&format!("[{i}] [{id}] {snippet}\n"));
    }
    prompt.push_str("\nReturn a JSON object with scores 0-5 for each candidate ");
    prompt.push_str("indicating relevance to the query, e.g. ");
    prompt.push_str(r#"{"scores": [{"idx": 0, "score": 4}, {"idx": 1, "score": 2}]}"#);

    let req = LlmRequest {
        system: "You are a relevance judge. Score how relevant each candidate is to the query."
            .into(),
        prompt,
        max_tokens: 1024,
        temperature: 0.0,
    };

    let rt = tokio::runtime::Runtime::new().map_err(|e| kgx_core::KgError::Llm(e.to_string()))?;
    let resp = rt
        .block_on(llm.complete(req))
        .map_err(|e| kgx_core::KgError::Llm(e.to_string()))?;

    // Parse LLM response for scores. Handle markdown code fences and free-text prefixes.
    let raw = resp.text.trim();
    // Strip markdown code fences if present
    let json_str = raw
        .strip_prefix("```json")
        .or_else(|| raw.strip_prefix("```"))
        .map(|s| s.strip_suffix("```").unwrap_or(s))
        .unwrap_or(raw);
    let json_str = json_str.trim();
    let mut llm_scores: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(scores) = val["scores"].as_array() {
            for entry in scores {
                let idx = entry["idx"].as_u64().unwrap_or(u64::MAX) as usize;
                let score = entry["score"].as_f64().unwrap_or(0.0) as f32;
                if idx < top_k.len() {
                    llm_scores.insert(idx, score);
                }
            }
        }
    }

    // Reorder: candidates with LLM score first (descending), then fill with unscored
    let mut reranked: Vec<(String, f32)> = top_k
        .iter()
        .enumerate()
        .map(|(i, (id, _))| {
            let score = llm_scores.get(&i).copied().unwrap_or(0.0);
            (id.clone(), score)
        })
        .collect();
    reranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    Ok(reranked)
}
