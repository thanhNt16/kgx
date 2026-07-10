// ingest_url — fetch URL content and ingest, with optional crawl depth
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(root: &Path, args: &Value) -> Result<Value> {
    let url = args["url"].as_str().unwrap_or("");
    if url.is_empty() {
        return Err(KgError::Other("ingest_url requires url".into()));
    }

    let depth = args["depth"].as_u64().unwrap_or(0) as u32;
    let max_pages = args["max_pages"].as_u64().unwrap_or(50) as u32;
    let same_domain = args.get("same_domain").and_then(|v| v.as_bool()).unwrap_or(true);

    let effective_depth = if same_domain { depth } else { 0 };

    let result = crate::url_crawl::crawl(url, effective_depth, max_pages, root).await?;

    Ok(json!({
        "status": "ok",
        "seed_url": url,
        "depth": effective_depth,
        "pages_captured": result.pages_captured,
        "pages_skipped": result.pages_skipped,
        "raw": result.raw_paths,
    }))
}
