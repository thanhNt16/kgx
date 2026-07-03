// query_memory — structured query with filters
use serde_json::{json, Value};
use std::path::Path;

pub fn run(root: &Path, args: &Value) -> Result<Value, kgx_core::KgError> {
    let notes = kgx_vault::scan::scan_vault(root)?;
    let mut results: Vec<Value> = notes
        .iter()
        .filter(|n| {
            if let Some(typ) = args["note_type"].as_str().filter(|s| !s.is_empty()) {
                if format!("{:?}", n.fm.r#type).to_lowercase() != typ.to_lowercase() {
                    return false;
                }
            }
            if let Some(tag) = args["tag"].as_str().filter(|s| !s.is_empty()) {
                if !n.fm.tags.iter().any(|t| t.contains(tag)) {
                    return false;
                }
            }
            if let Some(status) = args["status"].as_str().filter(|s| !s.is_empty()) {
                if format!("{:?}", n.fm.status).to_lowercase() != status.to_lowercase() {
                    return false;
                }
            }
            true
        })
        .take(args["limit"].as_u64().unwrap_or(10) as usize)
        .map(|n| {
            json!({
                "id": n.fm.id,
                "title": n.fm.title,
                "type": format!("{:?}", n.fm.r#type),
                "status": format!("{:?}", n.fm.status),
                "tags": n.fm.tags,
                "path": n.rel_path.display().to_string(),
            })
        })
        .collect();
    results.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(json!(results))
}
