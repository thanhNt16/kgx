// crates/kgx-cli/src/commands/query.rs
use std::time::Instant;

use crate::output::emit;
use kgx_vault::scan::scan_vault;

pub fn run(
    json: bool,
    note_type: Option<String>,
    entity_type: Option<String>,
    tag: Option<String>,
    status: Option<String>,
    limit: usize,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;
    let notes = scan_vault(&root)?;

    let mut results: Vec<serde_json::Value> = Vec::new();
    for n in notes.iter() {
        if let Some(ref nt) = note_type {
            if !format!("{:?}", n.fm.r#type)
                .to_lowercase()
                .contains(&nt.to_lowercase())
            {
                continue;
            }
        }
        if let Some(ref et) = entity_type {
            if n.fm.entity_type.as_deref() != Some(et.as_str()) {
                continue;
            }
        }
        if let Some(ref t) = tag {
            if !n.fm.tags.iter().any(|x| x.contains(t.as_str())) {
                continue;
            }
        }
        if let Some(ref s) = status {
            if !format!("{:?}", n.fm.status)
                .to_lowercase()
                .contains(&s.to_lowercase())
            {
                continue;
            }
        }
        results.push(serde_json::json!({
            "id": n.fm.id,
            "title": n.fm.title,
            "type": format!("{:?}", n.fm.r#type).to_lowercase(),
            "entity_type": n.fm.entity_type,
            "status": format!("{:?}", n.fm.status).to_lowercase(),
            "tags": n.fm.tags,
            "path": n.rel_path.display().to_string(),
        }));
        if results.len() >= limit {
            break;
        }
    }

    emit(
        "query",
        serde_json::json!({"results": results, "count": results.len()}),
        json,
        start,
        |d| {
            let count = d["count"].as_u64().unwrap_or(0);
            println!("{count} note(s)");
            if let Some(arr) = d["results"].as_array() {
                for r in arr {
                    let title = r["title"].as_str().unwrap_or("?");
                    let id = r["id"].as_str().unwrap_or("?");
                    println!("  {id} {title}");
                }
            }
        },
    );
    Ok(())
}
