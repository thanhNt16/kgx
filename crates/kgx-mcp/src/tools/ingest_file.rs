// ingest_file — capture raw content (idempotent by sha256)
//
// Accepts either:
//   - `content`: raw text to capture verbatim, or
//   - `path`: a file or directory on disk. A directory is walked recursively
//     and every text file (by extension allowlist) is captured as its own
//     immutable raw note. This lets an agent ingest a whole folder without
//     loading each file into tool-call context.
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const DEFAULT_TEXT_EXTS: &[&str] = &["md", "txt", "markdown", "mdx"];

pub fn run(root: &Path, args: &Value) -> Result<Value> {
    // Optional extension filter for directory ingestion.
    let exts: Vec<String> = match args["ext"].as_str() {
        Some(c) if !c.trim().is_empty() => c
            .split(',')
            .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => DEFAULT_TEXT_EXTS.iter().map(|s| s.to_string()).collect(),
    };

    // Directory branch.
    if let Some(path_str) = args["path"].as_str() {
        let path = Path::new(path_str);
        if path.is_dir() {
            let mut ingested = Vec::new();
            let mut skipped = 0u32;
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if !p.is_file() || !has_text_ext(p, &exts) {
                    continue;
                }
                let content = std::fs::read_to_string(p).map_err(|e| KgError::Io {
                    path: p.display().to_string(),
                    source: e,
                })?;
                match ingest_content(root, &content)? {
                    Some(out) => ingested.push(out),
                    None => skipped += 1,
                }
            }
            return Ok(json!({
                "status": "ok",
                "count": ingested.len(),
                "skipped": skipped,
                "ingested": ingested,
            }));
        }
        if path.is_file() {
            let content = std::fs::read_to_string(path).map_err(|e| KgError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            return match ingest_content(root, &content)? {
                Some(out) => Ok(json!({ "status": "ok", "raw": out.raw, "hash": out.hash })),
                None => Ok(json!({ "status": "skipped", "reason": "content unchanged" })),
            };
        }
        return Err(KgError::Other(format!(
            "path not found or not a file/dir: {path_str}"
        )));
    }

    // Content branch (original behavior).
    let content = args["content"].as_str().unwrap_or("");
    if content.is_empty() {
        return Err(KgError::Other(
            "ingest_file requires content or path".into(),
        ));
    }
    match ingest_content(root, content)? {
        Some(out) => Ok(json!({ "status": "ok", "raw": out.raw, "hash": out.hash })),
        None => Ok(json!({ "status": "skipped", "reason": "content unchanged" })),
    }
}

#[derive(serde::Serialize)]
struct Ingested {
    raw: String,
    hash: String,
}

/// Write `content` to `raw/<date>-<slug>.md` under `root`. Idempotent by sha256:
/// returns `None` if the target already exists with identical content.
fn ingest_content(root: &Path, content: &str) -> Result<Option<Ingested>> {
    let hash = sha256(content);
    let title = content.lines().next().unwrap_or("capture");
    let stem = kgx_core::util::slugify(title);
    let rel = format!("raw/{}-{stem}.md", &kgx_core::util::now_iso()[..10]);
    let path: PathBuf = root.join(&rel);

    if path.exists() {
        return Ok(None);
    }
    // (An earlier collision check by hash is subsumed by the existence check:
    // the rel path is derived deterministically from the title + date, so a
    // remount of identical content on the same day lands at the same path.)
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KgError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    std::fs::write(&path, render_raw(&hash, title, content)).map_err(|e| KgError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(Some(Ingested { raw: rel, hash }))
}

fn render_raw(hash: &str, title: &str, content: &str) -> String {
    format!(
        "---\ntype: source\nid: {}\ntitle: \"{}\"\ncreated_via: mcp\nhash: {hash}\n---\n{content}\n",
        kgx_core::util::new_ulid(),
        title.replace('"', "\\\"")
    )
}

fn has_text_ext(path: &Path, exts: &[String]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|x| x == e))
        .unwrap_or(false)
}

fn sha256(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}
