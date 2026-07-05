use crate::output::emit;
use kgx_graph::Brain;
use std::time::Instant;

#[derive(Debug, serde::Serialize)]
pub struct StatusSnapshot {
    pub nodes: i64,
    pub edges: i64,
    pub orphans: usize,
    pub pending_diffs: usize,
    pub last_index: Option<String>,
    pub last_dream: Option<String>,
    pub embedder: String,
}

pub fn snapshot() -> anyhow::Result<StatusSnapshot> {
    let root = std::env::current_dir()?;
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let orphans = kgx_graph::links::orphans(&notes).len();
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let nodes = brain
        .conn()
        .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
        .unwrap_or(0);
    let edges = brain
        .conn()
        .query_row("SELECT count(*) FROM edges", [], |r| r.get(0))
        .unwrap_or(0);
    let pending_diffs = std::fs::read_to_string(root.join(".kg/staged_diffs.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
        .map(|v| v.len())
        .unwrap_or(0);
    let meta = std::fs::read_to_string(root.join(".kg/meta.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    Ok(StatusSnapshot {
        nodes,
        edges,
        orphans,
        pending_diffs,
        last_index: meta["last_index"].as_str().map(ToOwned::to_owned),
        last_dream: meta["last_dream"].as_str().map(ToOwned::to_owned),
        embedder: kgx_llm::select::embedder_label(),
    })
}

pub fn run(json: bool, _verbose: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let snap = snapshot()?;
    emit("status", snap, json, start, |s| {
        println!(
            "nodes={} edges={} orphans={} pending={} embedder={}",
            s.nodes, s.edges, s.orphans, s.pending_diffs, s.embedder
        )
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedder_label_is_never_empty() {
        assert!(!kgx_llm::select::embedder_label().is_empty());
    }
}
