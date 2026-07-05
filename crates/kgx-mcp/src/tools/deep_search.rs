// deep_search_memory — progressive disclosure search
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn run(root: &Path, args: &Value) -> Result<Value> {
    let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let embedder = kgx_llm::select::embedder_from_env();
    let query = args["query"].as_str().unwrap_or("");
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;

    let r = kgx_retrieval::Retrievers::new(embedder.as_ref());
    let hits = kgx_retrieval::search(
        &brain,
        &r,
        query,
        kgx_retrieval::SearchOpts {
            mode: kgx_retrieval::Mode::Hybrid,
            limit,
            expand_ppr: true,
            filter_entities: true,
            rerank_graph: true,
            rerank_llm: false,
            rerank_topk: 0,
        },
    )
    .map_err(|e| KgError::Other(e.to_string()))?;

    Ok(json!({
        "query": query,
        "hits": hits,
    }))
}
