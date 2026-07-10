// crates/kgx-cli/src/commands/recall.rs
use std::collections::HashSet;
use std::time::Instant;

use crate::output::emit;
use kgx_graph::{query, Brain};
use kgx_vault::scan::scan_vault;

fn wikilink_inner(s: &str) -> &str {
    s.trim_start_matches("[[").trim_end_matches("]]")
}

pub fn run(json: bool, entity: &str, relations: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;

    let brain_path = root.join(".kg/brain.sqlite");
    if !brain_path.exists() {
        anyhow::bail!("brain not built — run `kg index --full` first");
    }

    let notes = scan_vault(&root)?;
    let brain = Brain::open(&brain_path)?;

    let entity_lower = entity.to_lowercase();

    let primary: Option<&kgx_core::types::Note> = notes.iter().find(|n| {
        n.fm.title == entity
            || n.fm.id == entity
            || n.fm
                .aliases
                .iter()
                .any(|a| a.to_lowercase() == entity_lower)
    });

    let neighbor_ids: Vec<String> = if let Some(note) = primary {
        query::neighbors(&brain, &note.fm.id, 2)?
    } else {
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

        let mut seen: HashSet<String> = HashSet::new();
        let mut all: Vec<String> = Vec::new();
        for id in matching_ids {
            for nbr in query::neighbors(&brain, id, 2)? {
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

    let data = if relations {
        let rel_edges: Vec<serde_json::Value> = if let Some(note) = primary {
            let edges = query::neighbors_with_relations(&brain, &note.fm.id, 2)?;
            edges
                .iter()
                .filter_map(|e| {
                    notes.iter().find(|n| n.fm.id == e.dst_id).map(|n| {
                        serde_json::json!({"target": n.fm.title, "rel": e.rel_type})
                    })
                })
                .collect()
        } else {
            vec![]
        };
        serde_json::json!({"entity": entity, "neighbors": titles, "relations": rel_edges})
    } else {
        serde_json::json!({"entity": entity, "neighbors": titles})
    };

    emit(
        "recall",
        data,
        json,
        start,
        |d| {
            println!("Entity: {}", d["entity"]);
            if let Some(neighbors) = d["neighbors"].as_array() {
                for t in neighbors {
                    if let Some(s) = t.as_str() {
                        println!("  - {s}");
                    }
                }
            }
            if let Some(rels) = d["relations"].as_array() {
                if !rels.is_empty() {
                    println!("Relations:");
                    for r in rels {
                        let target = r["target"].as_str().unwrap_or("?");
                        let rel = r["rel"].as_str().unwrap_or("?");
                        println!("  {rel} -> {target}");
                    }
                }
            }
        },
    );
    Ok(())
}
