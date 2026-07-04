// ingest_file — capture raw content (idempotent by sha256)
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn run(root: &Path, args: &Value) -> Result<Value> {
    let content = args["content"].as_str().unwrap_or("");
    if content.is_empty() {
        return Err(KgError::Other("ingest_file requires content".into()));
    }

    let hash = sha256(content);
    let title = content.lines().next().unwrap_or("capture");
    let stem = kgx_core::util::slugify(title);
    let rel = format!("raw/{}-{stem}.md", &kgx_core::util::now_iso()[..10]);
    let path = root.join(&rel);

    if path.exists() {
        return Ok(json!({"status": "skipped", "reason": "already exists", "raw": rel}));
    }

    if std::fs::read_to_string(&path).ok().map(|s| sha256(&s)) == Some(hash.clone()) {
        return Ok(json!({"status": "skipped", "reason": "content unchanged", "raw": rel}));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KgError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }

    std::fs::write(
        &path,
        format!(
            "---\ntype: source\nid: {}\ntitle: \"{}\"\ncreated_via: mcp\nhash: {hash}\n---\n{content}\n",
            kgx_core::util::new_ulid(),
            title.replace('"', "\\\"")
        ),
    )
    .map_err(|e| KgError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    Ok(json!({"status": "ok", "raw": rel, "hash": hash}))
}

fn sha256(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}
