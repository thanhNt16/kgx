// crates/kgx-cli/src/commands/search.rs
use std::time::Instant;

use crate::output::emit;
use kgx_core::llm::LlmProvider;
use kgx_graph::Brain;
use kgx_retrieval::{search, Mode, SearchOpts};

pub fn run(
    json: bool,
    query: &str,
    mode: &str,
    limit: usize,
    rerank_graph: bool,
    rerank_llm: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let embedder = kgx_llm::select::embedder_from_env();
    let m = match mode {
        "keyword" => Mode::Keyword,
        "semantic" => Mode::Semantic,
        _ => Mode::Hybrid,
    };
    let llm: Option<Box<dyn LlmProvider>> = if rerank_llm {
        Some(kgx_llm::select::provider_from_env()?)
    } else {
        None
    };
    let reranker = kgx_llm::select::reranker_from_env();
    let r = kgx_retrieval::Retrievers::new(embedder.as_ref())
        .with_llm(llm.as_deref())
        .with_reranker(reranker.as_deref());
    let hits = search(
        &brain,
        &r,
        query,
        SearchOpts {
            mode: m,
            limit,
            expand_ppr: true,
            filter_entities: false,
            rerank_graph,
            rerank_llm,
            rerank_topk: kgx_llm::select::rerank_topk_from_env(),
        },
    )?;
    emit(
        "search",
        serde_json::json!({"hits": hits}),
        json,
        start,
        |_| {
            for h in &hits {
                println!("{:.4} {} [{}]", h.score, h.id, h.signals.join(","));
            }
        },
    );
    Ok(())
}
