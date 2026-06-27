// crates/kgx-cli/src/commands/recall.rs
use std::time::Instant;

use crate::output::emit;
use kgx_graph::{query::neighbors, Brain};
use kgx_vault::scan::scan_vault;

pub fn run(json: bool, entity: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let notes = scan_vault(&root)?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let note = notes
        .iter()
        .find(|n| n.fm.title == entity || n.fm.id == entity)
        .ok_or_else(|| anyhow::anyhow!("entity not found: {entity}"))?;
    let nbrs = neighbors(&brain, &note.fm.id, 2)?;
    let titles: Vec<String> = nbrs
        .iter()
        .filter_map(|id| notes.iter().find(|n| n.fm.id == *id))
        .map(|n| n.fm.title.clone())
        .collect();
    emit(
        "recall",
        serde_json::json!({"entity": entity, "neighbors": titles}),
        json,
        start,
        |_| {
            println!("Entity: {entity}");
            for t in &titles {
                println!("  - {t}");
            }
        },
    );
    Ok(())
}
