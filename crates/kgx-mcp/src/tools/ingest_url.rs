// ingest_url — fetch URL content and ingest
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(root: &Path, args: &Value) -> Result<Value> {
    let url = args["url"].as_str().unwrap_or("");
    if url.is_empty() {
        return Err(KgError::Other("ingest_url requires url".into()));
    }

    let content = reqwest::get(url)
        .await
        .map_err(|e| KgError::Other(format!("fetch failed: {e}")))?
        .text()
        .await
        .map_err(|e| KgError::Other(format!("read body failed: {e}")))?;

    let title = content.lines().next().unwrap_or("web-capture");
    let stem = kgx_core::util::slugify(title);
    let rel = format!("raw/{}-{stem}.md", &kgx_core::util::now_iso()[..10]);
    let path = root.join(&rel);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KgError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }

    std::fs::write(
        &path,
        format!(
            "---\ntype: source\nid: {}\ntitle: \"{}\"\nsource: {url}\ncreated_via: mcp\n---\n{content}\n",
            kgx_core::util::new_ulid(),
            title.replace('"', "\\\"")
        ),
    )
    .map_err(|e| KgError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    Ok(json!({"status": "ok", "raw": rel, "url": url}))
}
