// crates/kgx-cli/src/commands/capture.rs
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use crate::output::emit;
use kgx_core::util;

/// Default extensions captured when ingesting a directory.
const DEFAULT_TEXT_EXTS: &[&str] = &[
    "md", "txt", "markdown", "mdx", "pdf", "docx", "pptx", "odt", "epub", "html", "htm", "xlsx",
    "xls",
];

pub fn run(
    json: bool,
    from: &str,
    kind: &str,
    exts_csv: Option<&str>,
    _depth: u32,
    _max_pages: u32,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;

    if from.starts_with("http://") || from.starts_with("https://") {
        anyhow::bail!("URL capture requires kgx-mcp url_crawl module (Task 7). Use ingest_url MCP tool for now.");
    }

    // Directory branch: walk recursively, capture each matching file.
    if Path::new(from).is_dir() {
        let exts = parse_exts(exts_csv);
        let mut captured: Vec<String> = Vec::new();
        let mut skipped = 0u32;
        for entry in walkdir::WalkDir::new(from)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !has_text_ext(path, &exts) && !kgx_convert::is_document_ext(ext) {
                continue;
            }
            match capture_file(&root, path, kind) {
                Ok(Some(c)) => captured.push(c.raw_rel),
                Ok(None) => skipped += 1,
                Err(e) => {
                    eprintln!("skip {}: {e}", path.display());
                    skipped += 1;
                }
            }
        }
        emit(
            "capture",
            serde_json::json!({
                "from": from,
                "kind": kind,
                "captured": captured.len(),
                "skipped": skipped,
                "raw": captured,
            }),
            json,
            start,
            |_| {
                println!("captured {} file(s) (skipped {skipped})", captured.len());
            },
        );
        return Ok(());
    }

    // Single-source branch (file path or "-" stdin).
    let (raw_rel, src_rel, status) = if from == "-" {
        let mut content = String::new();
        std::io::stdin().read_to_string(&mut content)?;
        match capture_one_returning(&root, &content, kind)? {
            Some(c) => (c.raw_rel, c.src_rel, "ok"),
            None => ("(skipped)".to_string(), "(skipped)".to_string(), "skipped"),
        }
    } else if Path::new(from).exists() {
        let path = Path::new(from);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if kgx_convert::is_document_ext(ext)
            && ext != "md"
            && ext != "txt"
            && ext != "markdown"
            && ext != "mdx"
        {
            // Document format — convert first
            let converted = kgx_convert::convert(path).map_err(|e| anyhow::anyhow!("{e}"))?;
            match capture_one_returning(&root, &converted.markdown, kind)? {
                Some(c) => (c.raw_rel, c.src_rel, "ok"),
                None => ("(skipped)".to_string(), "(skipped)".to_string(), "skipped"),
            }
        } else {
            let content = std::fs::read_to_string(path)?;
            match capture_one_returning(&root, &content, kind)? {
                Some(c) => (c.raw_rel, c.src_rel, "ok"),
                None => ("(skipped)".to_string(), "(skipped)".to_string(), "skipped"),
            }
        }
    } else {
        anyhow::bail!("cannot read source: {from}");
    };

    emit(
        "capture",
        serde_json::json!({"raw": raw_rel, "source_note": src_rel, "kind": kind, "status": status}),
        json,
        start,
        |_| println!("captured -> {raw_rel}"),
    );
    Ok(())
}

fn capture_file(root: &Path, path: &Path, kind: &str) -> anyhow::Result<Option<Captured>> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if kgx_convert::is_document_ext(ext)
        && ext != "md"
        && ext != "txt"
        && ext != "markdown"
        && ext != "mdx"
    {
        let converted = kgx_convert::convert(path).map_err(|e| anyhow::anyhow!("{e}"))?;
        capture_one_returning(root, &converted.markdown, kind)
    } else {
        let content = std::fs::read_to_string(path)?;
        capture_one_returning(root, &content, kind)
    }
}

struct Captured {
    raw_rel: String,
    src_rel: String,
}

/// Capture one source string to `raw/` + a `notes/sources/` pointer note.
/// Returns `None` if the raw already exists with identical content (idempotent skip).
fn capture_one_returning(
    root: &Path,
    content: &str,
    kind: &str,
) -> anyhow::Result<Option<Captured>> {
    let today = &util::now_iso()[..10];
    let title = title_of(content);
    let slug = util::slugify(&title);
    let raw_rel = format!("raw/{today}-{slug}.md");
    let raw_path = root.join(&raw_rel);

    if raw_path.exists() {
        let existing = std::fs::read_to_string(&raw_path)?;
        if !existing.contains(content) {
            anyhow::bail!("raw immutability: {raw_rel} exists with different content");
        }
        return Ok(None); // unchanged — idempotent
    }

    let id = util::new_ulid();
    if let Some(parent) = raw_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &raw_path,
        format!(
            "---\ntype: source\nid: {id}\ntitle: \"{title}\"\ncreated_by: human\ncreated_via: cli\n---\n{content}\n"
        ),
    )?;

    // Source pointer note
    let sid = util::new_ulid();
    let src_rel = format!("notes/sources/{slug}.md");
    let raw_stem = raw_rel.trim_end_matches(".md");
    let source_link = format!("[[{raw_stem}]]");
    std::fs::create_dir_all(root.join("notes/sources"))?;
    std::fs::write(
        root.join(&src_rel),
        format!(
            "---\ntype: source\nid: {sid}\ntitle: \"{title}\"\nsource: \"{source_link}\"\ncreated_by: agent\ncreated_via: cli\n---\nCaptured {kind} source.\n"
        ),
    )?;
    Ok(Some(Captured { raw_rel, src_rel }))
}

fn title_of(content: &str) -> String {
    content
        .lines()
        .next()
        .unwrap_or("capture")
        .trim_start_matches('#')
        .trim()
        .chars()
        .take(60)
        .collect::<String>()
}

fn parse_exts(csv: Option<&str>) -> Vec<String> {
    match csv {
        Some(c) if !c.trim().is_empty() => c
            .split(',')
            .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => DEFAULT_TEXT_EXTS.iter().map(|s| s.to_string()).collect(),
    }
}

fn has_text_ext(path: &Path, exts: &[String]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|x| x == e))
        .unwrap_or(false)
}
