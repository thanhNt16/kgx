// crates/kgx-cli/src/commands/capture.rs
use std::io::Read;
use std::time::Instant;

use crate::output::emit;
use kgx_core::util;

pub fn run(json: bool, from: &str, kind: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;

    let content = match from {
        "-" => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            s
        }
        path if std::path::Path::new(path).exists() => std::fs::read_to_string(path)?,
        url if url.starts_with("http") => {
            anyhow::bail!("url capture requires --features net (Phase 6)")
        }
        other => anyhow::bail!("cannot read source: {other}"),
    };

    let today = &util::now_iso()[..10];
    let title = content
        .lines()
        .next()
        .unwrap_or("capture")
        .chars()
        .take(60)
        .collect::<String>();
    let slug = util::slugify(&title);
    let raw_rel = format!("raw/{today}-{slug}.md");
    let raw_path = root.join(&raw_rel);

    if raw_path.exists() {
        let existing = std::fs::read_to_string(&raw_path)?;
        if !existing.contains(&content) {
            anyhow::bail!("raw immutability: {raw_rel} exists with different content");
        }
    } else {
        let id = util::new_ulid();
        std::fs::create_dir_all(raw_path.parent().unwrap())?;
        std::fs::write(
            &raw_path,
            format!(
                "---\ntype: source\nid: {id}\ntitle: \"{title}\"\ncreated_by: human\ncreated_via: cli\n---\n{content}\n"
            ),
        )?;
    }

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

    emit(
        "capture",
        serde_json::json!({"raw": raw_rel, "source_note": src_rel, "kind": kind}),
        json,
        start,
        |_| println!("captured -> {raw_rel}"),
    );
    Ok(())
}
