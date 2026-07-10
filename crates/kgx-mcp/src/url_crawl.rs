use kgx_core::{KgError, Result};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct CrawlResult {
    pub pages_captured: u32,
    pub pages_skipped: u32,
    pub raw_paths: Vec<String>,
}

const MEDIA_EXTS: &[&str] = &[
    "pdf", "png", "jpg", "jpeg", "gif", "svg", "css", "js", "woff", "woff2",
    "ico", "mp4", "webm", "zip", "tar", "gz",
];

fn is_media_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let path = lower.split('?').next().unwrap_or(&lower);
    MEDIA_EXTS.iter().any(|ext| path.ends_with(&format!(".{ext}")))
}

fn same_domain(seed: &url::Url, target: &url::Url) -> bool {
    seed.host_str() == target.host_str()
}

fn convert_html_to_markdown(html: &str) -> String {
    // Try pandoc first
    if let Ok(pandoc_path) = kgx_convert::pandoc::resolve_pandoc() {
        let dir = tempfile::tempdir().ok();
        if let Some(dir) = dir {
            let html_path = dir.path().join("input.html");
            if std::fs::write(&html_path, html).is_ok() {
                if let Ok(md) = kgx_convert::pandoc::convert(&html_path) {
                    return md;
                }
            }
        }
    }
    // Fallback: strip HTML tags
    strip_html_tags(html)
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if !in_tag && c == '<' {
            in_tag = true;
            // Check for <script> or <style> — skip entirely
            let remaining = &lower[i..];
            if remaining.starts_with("<script") || remaining.starts_with("<style") {
                let close_tag = if remaining.starts_with("<script") { "</script>" } else { "</style>" };
                if let Some(pos) = lower[i..].find(close_tag) {
                    i += pos + close_tag.len();
                    in_tag = false;
                    continue;
                }
            }
        } else if in_tag && c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
        i += 1;
    }
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_links(html: &str, base_url: &url::Url) -> Vec<String> {
    let fragment = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("a[href]").unwrap();
    fragment
        .select(&selector)
        .filter_map(|el| el.value().attr("href"))
        .filter_map(|href| base_url.join(href).ok())
        .map(|u| u.to_string())
        .collect()
}

fn capture_page(root: &Path, url: &str, content: &str) -> Result<String> {
    let today = &kgx_core::util::now_iso()[..10];
    let title = content
        .lines()
        .next()
        .unwrap_or("web-capture")
        .trim()
        .chars()
        .take(60)
        .collect::<String>();
    let slug = kgx_core::util::slugify(&title);
    let rel = format!("raw/{}-{slug}.md", today);
    let path = root.join(&rel);

    if path.exists() {
        return Ok(rel); // idempotent skip
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
            "---\ntype: source\nid: {}\ntitle: \"{}\"\nsource: {url}\ncreated_via: mcp\n---\n{content}\n",
            kgx_core::util::new_ulid(),
            title.replace('"', "\\\"")
        ),
    )
    .map_err(|e| KgError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    Ok(rel)
}

pub async fn crawl(
    seed_url: &str,
    depth: u32,
    max_pages: u32,
    root: &Path,
) -> Result<CrawlResult> {
    let seed = url::Url::parse(seed_url)
        .map_err(|e| KgError::Other(format!("invalid URL: {e}")))?;

    let delay_ms = std::env::var("KGX_CRAWL_DELAY_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(500);

    let mut visited: HashSet<String> = HashSet::new();
    let mut raw_paths: Vec<String> = Vec::new();
    let mut pages_captured = 0u32;
    let mut pages_skipped = 0u32;

    // BFS queue: (url, current_depth)
    let mut queue: Vec<(String, u32)> = vec![(seed_url.to_string(), 0)];

    while let Some((current_url, current_depth)) = queue.pop() {
        if pages_captured >= max_pages {
            break;
        }
        if visited.contains(&current_url) {
            continue;
        }
        visited.insert(current_url.clone());

        if is_media_url(&current_url) {
            pages_skipped += 1;
            continue;
        }

        let resp = match reqwest::get(&current_url).await {
            Ok(r) => r,
            Err(_) => {
                pages_skipped += 1;
                continue;
            }
        };

        let html = match resp.text().await {
            Ok(t) => t,
            Err(_) => {
                pages_skipped += 1;
                continue;
            }
        };

        let markdown = convert_html_to_markdown(&html);
        match capture_page(root, &current_url, &markdown) {
            Ok(rel) => {
                raw_paths.push(rel);
                pages_captured += 1;
            }
            Err(_) => {
                pages_skipped += 1;
            }
        }

        // Enqueue same-domain links if we haven't reached max depth
        if current_depth < depth {
            let links = extract_links(&html, &seed);
            for link in links {
                if pages_captured + queue.len() as u32 >= max_pages {
                    break;
                }
                if let Ok(link_url) = url::Url::parse(&link) {
                    if same_domain(&seed, &link_url)
                        && !visited.contains(&link)
                        && !is_media_url(&link)
                    {
                        queue.push((link, current_depth + 1));
                    }
                }
            }
        }

        if !queue.is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    }

    Ok(CrawlResult {
        pages_captured,
        pages_skipped,
        raw_paths,
    })
}
