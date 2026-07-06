use crate::{commands::status, output::emit};
use kgx_tokens::aggregate::{summarize, GroupBy};
use std::time::Instant;

pub fn run(json: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;
    let snap = status::snapshot()?;
    let tokens_by_day = summarize(&root.join(".kg"), 30, GroupBy::Day)?;
    let data = serde_json::json!({
        "nodes": snap.nodes,
        "edges": snap.edges,
        "orphans": snap.orphans,
        "pending_diffs": snap.pending_diffs,
        "last_index": snap.last_index,
        "tokens_by_day": tokens_by_day
    });
    emit("dashboard", data, json, start, |d| {
        println!(
            "KGX dashboard\nnodes: {}\nedges: {}\norphans: {}\npending diffs: {}",
            d["nodes"], d["edges"], d["orphans"], d["pending_diffs"]
        )
    });
    Ok(())
}
