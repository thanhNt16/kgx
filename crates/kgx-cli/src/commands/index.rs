use std::time::Instant;

use crate::output::emit;
use kgx_graph::{build::build_full, embed::MockEmbedder, pagerank, Brain};
use kgx_tokens::record::{append, TokenRecord};

pub fn run(
    json: bool,
    full: bool,
    _incremental: bool,
    do_pagerank: bool,
    _communities: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let kg_dir = root.join(".kg");
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let mut brain = Brain::open(&kg_dir.join("brain.sqlite"))?;
    let embedder = MockEmbedder::new();
    let stats = build_full(&mut brain, &notes, &embedder)?;
    let _ = full;
    if do_pagerank {
        pagerank::compute(&mut brain, 0.85, 30)?;
    }
    let approx_in: u32 = notes.iter().map(|n| (n.body.len() / 4) as u32).sum();
    append(
        &kg_dir,
        &TokenRecord {
            model: "mock-embed".into(),
            operation: "embed".into(),
            command: "index".into(),
            input_tokens: approx_in,
            output_tokens: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
            correlation_id: kgx_core::util::new_ulid(),
            ts: kgx_core::util::now_iso(),
        },
    )?;
    std::fs::write(
        kg_dir.join("meta.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "last_index": kgx_core::util::now_iso(),
            "nodes": stats.nodes,
            "edges": stats.edges,
        }))?,
    )?;
    emit("index", stats, json, start, |s| {
        println!("\u{2714} indexed {} nodes, {} edges", s.nodes, s.edges)
    });
    Ok(())
}
