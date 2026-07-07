// nl_query_memory — retrieval over the knowledge graph (no LLM synthesis).
//
// Returns matching context. Q&A synthesis is the agent harness's job — it
// reasons over the returned hits/community summaries and cites note ids.
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(root: &Path, args: &Value) -> Result<Value> {
    let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let embedder = kgx_llm::select::embedder_from_env();
    let query = args["query"].as_str().unwrap_or("");
    let limit = args["limit"].as_u64().unwrap_or(10) as usize;

    // `scope=global` widens retrieval to community summaries (maps-of-content)
    // before drilling into member notes. Both scopes return retrieved context
    // only — the harness synthesizes the answer.
    if matches!(args["scope"].as_str(), Some("global")) {
        let context = kgx_retrieval::global::global_context(&brain, query, embedder.as_ref(), 5)
            .map_err(|e| KgError::Other(e.to_string()))?;
        return Ok(json!({
            "scope": "global",
            "context": context,
            "query": query,
            "note": "synthesize the answer over `context`; cite note ids"
        }));
    }

    let mode = match args["mode"].as_str() {
        Some("keyword") => kgx_retrieval::Mode::Keyword,
        Some("semantic") => kgx_retrieval::Mode::Semantic,
        _ => kgx_retrieval::Mode::Hybrid,
    };
    let r = kgx_retrieval::Retrievers::new(embedder.as_ref());
    let hits = kgx_retrieval::search(
        &brain,
        &r,
        query,
        kgx_retrieval::SearchOpts {
            mode,
            limit,
            expand_ppr: true,
            filter_entities: true,
            rerank_graph: false,
            rerank_llm: false,
            rerank_topk: 0,
        },
    )
    .map_err(|e| KgError::Other(e.to_string()))?;
    Ok(json!({
        "scope": "local",
        "query": query,
        "hits": hits,
        "note": "synthesize the answer over `hits`; cite note ids"
    }))
}
