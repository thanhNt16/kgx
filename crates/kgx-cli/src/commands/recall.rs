// crates/kgx-cli/src/commands/recall.rs
use std::collections::HashSet;
use std::time::Instant;

use crate::output::emit;
use kgx_graph::{query::neighbors, Brain};
use kgx_vault::scan::scan_vault;

/// Strip `[[` / `]]` from a wikilink string and return the inner text.
fn wikilink_inner(s: &str) -> &str {
    s.trim_start_matches("[[").trim_end_matches("]]")
}

pub fn run(json: bool, entity: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;

    let brain_path = root.join(".kg/brain.sqlite");
    if !brain_path.exists() {
        anyhow::bail!("brain not built — run `kg index --full` first");
    }

    let notes = scan_vault(&root)?;
    let brain = Brain::open(&brain_path)?;

    let entity_lower = entity.to_lowercase();

    // 1. Exact match on title or id.
    // 2. Case-insensitive alias match.
    let primary: Option<&kgx_core::types::Note> = notes.iter().find(|n| {
        n.fm.title == entity
            || n.fm.id == entity
            || n.fm
                .aliases
                .iter()
                .any(|a| a.to_lowercase() == entity_lower)
    });

    let neighbor_ids: Vec<String> = if let Some(note) = primary {
        neighbors(&brain, &note.fm.id, 2)?
    } else {
        // 3. Search notes whose links field mentions the entity (case-insensitive wikilink).
        let matching_ids: Vec<&str> = notes
            .iter()
            .filter(|n| {
                n.fm.links
                    .iter()
                    .any(|l| wikilink_inner(l).to_lowercase() == entity_lower)
            })
            .map(|n| n.fm.id.as_str())
            .collect();

        if matching_ids.is_empty() {
            anyhow::bail!("entity not found: {entity}");
        }

        // Collect neighbours for all matching notes, deduplicating.
        let mut seen: HashSet<String> = HashSet::new();
        let mut all: Vec<String> = Vec::new();
        for id in matching_ids {
            for nbr in neighbors(&brain, id, 2)? {
                if seen.insert(nbr.clone()) {
                    all.push(nbr);
                }
            }
        }
        all
    };

    let titles: Vec<String> = neighbor_ids
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
