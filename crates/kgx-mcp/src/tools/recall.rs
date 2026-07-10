// recall_entity — retrieve an entity's graph neighborhood
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn run(root: &Path, args: &Value) -> Result<Value> {
    let entity = args["entity"].as_str().unwrap_or("");
    if entity.is_empty() {
        return Err(KgError::Other("recall_entity requires entity".into()));
    }
    let include_relations = args
        .get("relations")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let brain_path = root.join(".kg/brain.sqlite");
    if !brain_path.exists() {
        return Err(KgError::Other(
            "brain not built — run `kg index --full` first".into(),
        ));
    }

    let notes = kgx_vault::scan::scan_vault(root)?;
    let brain = kgx_graph::Brain::open(&brain_path)?;

    let entity_lower = entity.to_lowercase();
    let primary = notes.iter().find(|n| {
        n.fm.title == entity
            || n.fm.id == entity
            || n.fm
                .aliases
                .iter()
                .any(|a| a.to_lowercase() == entity_lower)
    });

    let entity_note = if let Some(n) = primary {
        n
    } else {
        return Err(KgError::NotFound(format!("entity not found: {entity}")));
    };

    let neighbor_ids = kgx_graph::query::neighbors(&brain, &entity_note.fm.id, 2)?;
    let titles: Vec<String> = neighbor_ids
        .iter()
        .filter_map(|id| notes.iter().find(|n| n.fm.id == *id))
        .map(|n| n.fm.title.clone())
        .collect();

    let mut data = json!({
        "entity": entity,
        "neighbors": titles,
    });

    if include_relations {
        let edges = kgx_graph::query::neighbors_with_relations(&brain, &entity_note.fm.id, 2)?;
        let rels: Vec<Value> = edges
            .iter()
            .filter_map(|e| {
                notes
                    .iter()
                    .find(|n| n.fm.id == e.dst_id)
                    .map(|n| json!({"target": n.fm.title, "rel": e.rel_type}))
            })
            .collect();
        data["relations"] = json!(rels);
    }

    Ok(data)
}
