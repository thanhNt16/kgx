pub mod deep_search;
pub mod dream;
pub mod get;
pub mod ingest_conversation;
pub mod ingest_file;
pub mod ingest_url;
pub mod nl_query;
pub mod query;
pub mod upsert;

use kgx_core::Result;
use serde_json::Value;
use std::path::Path;

pub fn tool_schemas() -> Value {
    serde_json::json!([
        {"name":"nl_query_memory","description":"Natural language hybrid search over the knowledge graph with Q&A","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"},"mode":{"type":"string"},"scope":{"type":"string"}},"required":["query"]}},
        {"name":"query_memory","description":"Structured query with filters: type, tag, status","inputSchema":{"type":"object","properties":{"note_type":{"type":"string"},"tag":{"type":"string"},"status":{"type":"string"},"limit":{"type":"integer"}}}},
        {"name":"deep_search_memory","description":"Progressive disclosure search — first pass finds clusters, second drills into top cluster","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"}},"required":["query"]}},
        {"name":"get_note","description":"Fetch a note by id","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}},
        {"name":"ingest_conversation","description":"Incremental conversation capture — append turns to a raw transcript, or finalize to compile durable facts/decisions","inputSchema":{"type":"object","properties":{"turns":{"type":"array","items":{"type":"object","properties":{"role":{"type":"string"},"content":{"type":"string"}},"required":["role","content"]}},"action":{"type":"string","enum":["incremental","finalize"]}},"required":["turns"]}},
        {"name":"ingest_file","description":"Ingest raw content or a file/folder into the vault immutably (idempotent by sha256). Pass `content` for text, or `path` (file or directory) to ingest from disk; a directory is walked recursively for text files.","inputSchema":{"type":"object","properties":{"content":{"type":"string"},"path":{"type":"string","description":"File or directory on disk to ingest (directory is walked recursively)"},"ext":{"type":"string","description":"Comma-separated extensions for directory ingest (default: md,txt,markdown,mdx)"}}}},
        {"name":"ingest_url","description":"Fetch a URL and ingest its content into the vault, with optional same-domain crawl depth","inputSchema":{"type":"object","properties":{"url":{"type":"string"},"depth":{"type":"integer","description":"Crawl depth: 0 = single page, 1 = page + direct links, etc. (default 0)"},"max_pages":{"type":"integer","description":"Maximum pages to fetch (default 50)"},"same_domain":{"type":"boolean","description":"Only follow same-domain links (default true)"}},"required":["url"]}},
        {"name":"upsert_note","description":"Create or update a note","inputSchema":{"type":"object","properties":{"type":{"type":"string"},"title":{"type":"string"},"body":{"type":"string"},"id":{"type":"string"},"source":{"type":"string","description":"Provenance wikilink, e.g. [[raw/2026-07-05-my-source]]"},"confidence":{"type":"string","enum":["high","medium","low"]},"links":{"type":"array","items":{"type":"string"},"description":"Wikilinks to related entities, e.g. [[Postgres]]"},"tags":{"type":"array","items":{"type":"string"}}},"required":["type","title","body"]}},
        {"name":"dream_step","description":"Run one bounded dream iteration, returns staged diffs","inputSchema":{"type":"object","properties":{"only":{"type":"string"},"max_iterations":{"type":"integer"}}}}
    ])
}

pub async fn dispatch(root: &Path, name: &str, args: &Value) -> Result<Value> {
    match name {
        "nl_query_memory" => nl_query::run(root, args).await,
        "query_memory" => query::run(root, args),
        "deep_search_memory" => deep_search::run(root, args),
        "get_note" => get::run(root, args),
        "ingest_conversation" => ingest_conversation::run(root, args).await,
        "ingest_file" => ingest_file::run(root, args),
        "ingest_url" => ingest_url::run(root, args).await,
        "upsert_note" => upsert::run(root, args),
        "dream_step" => dream::run(root, args).await,
        other => Err(kgx_core::KgError::NotFound(format!("tool {other}"))),
    }
}
