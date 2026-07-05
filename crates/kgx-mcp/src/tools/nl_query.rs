// nl_query_memory — combined search + Q&A
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(root: &Path, args: &Value) -> Result<Value> {
    let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let embedder = kgx_llm::select::embedder_from_env();
    let query = args["query"].as_str().unwrap_or("");

    match args["scope"].as_str() {
        Some("global") => {
            let context =
                kgx_retrieval::global::global_context(&brain, query, embedder.as_ref(), 5)
                    .map_err(|e| KgError::Other(e.to_string()))?;
            let provider = kgx_llm::select::provider_from_env()?;
            let resp = provider
                .complete(kgx_core::llm::LlmRequest {
                    system: "Answer from context, cite ids".into(),
                    prompt: format!("ANSWER_QUESTION\nContext:\n{context}\nQuestion: {query}"),
                    max_tokens: 1024,
                    temperature: 0.0,
                })
                .await?;
            Ok(serde_json::from_str(&resp.text)
                .unwrap_or_else(|_| json!({"answer": resp.text, "citations": []})))
        }
        _ => {
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
                    limit: args["limit"].as_u64().unwrap_or(10) as usize,
                    expand_ppr: true,
                    filter_entities: true,
                    rerank_graph: false,
                    rerank_llm: false,
                    rerank_topk: 0,
                },
            )
            .map_err(|e| KgError::Other(e.to_string()))?;
            Ok(json!(hits))
        }
    }
}
