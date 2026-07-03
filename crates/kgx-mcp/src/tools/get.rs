// get_note — fetch note by id
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn run(root: &Path, args: &Value) -> Result<Value> {
    let notes = kgx_vault::scan::scan_vault(root)?;
    let id = args["id"].as_str().unwrap_or("");
    let note = notes
        .into_iter()
        .find(|n| n.fm.id == id)
        .ok_or_else(|| KgError::NotFound(id.into()))?;
    Ok(json!({
        "id": note.fm.id,
        "title": note.fm.title,
        "body": note.body,
        "type": format!("{:?}", note.fm.r#type),
        "path": note.rel_path.display().to_string()
    }))
}
