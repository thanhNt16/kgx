// crates/kgx-cli/src/commands/search.rs
use std::time::Instant;

use crate::output::emit;
use kgx_graph::Brain;
use kgx_retrieval::{search, Mode, SearchOpts};

pub fn run(json: bool, query: &str, mode: &str, limit: usize) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let embedder = kgx_llm::select::embedder_from_env();
    let m = match mode {
        "keyword" => Mode::Keyword,
        "semantic" => Mode::Semantic,
        _ => Mode::Hybrid,
    };
    let hits = search(
        &brain,
        embedder.as_ref(),
        query,
        SearchOpts {
            mode: m,
            limit,
            expand_ppr: true,
            filter_entities: true,
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
