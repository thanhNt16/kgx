use std::time::Instant;

use crate::output::emit;
use kgx_core::Note;
use kgx_graph::{build::build_full, build::build_incremental, pagerank, Brain};
use kgx_tokens::record::{append, TokenRecord};

pub fn run(
    json: bool,
    full: bool,
    incremental: bool,
    rebuild_vectors: bool,
    do_pagerank: bool,
    communities: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let kg_dir = root.join(".kg");
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let mut brain = Brain::open(&kg_dir.join("brain.sqlite"))?;
    let embedder = kgx_llm::select::embedder_from_env();

    let (stats, embedded_ids): (_, std::collections::BTreeSet<String>) = if rebuild_vectors
        || (incremental && !full)
    {
        let changed_ids = find_changed_ids(&brain, &notes)?;
        let embedded_ids: std::collections::BTreeSet<String> =
            changed_ids.iter().cloned().collect();
        let s = build_incremental(&mut brain, &notes, &changed_ids, embedder.as_ref())?;
        (s, embedded_ids)
    } else {
        let all: std::collections::BTreeSet<String> = notes.iter().map(|n| n.fm.id.clone()).collect();
        let s = build_full(&mut brain, &notes, embedder.as_ref())?;
        (s, all)
    };

    if do_pagerank {
        pagerank::compute(&mut brain, 0.85, 30)?;
    }
    if communities {
        kgx_graph::leiden::detect(&mut brain, 42)?;
        let provider = kgx_llm::select::provider_from_env()?;
        let rt = tokio::runtime::Runtime::new()?;
        let summaries = rt.block_on(kgx_retrieval::community_summary::summarize_all(
            &brain,
            provider.as_ref(),
            &notes,
        ))?;
        let moc_dir = root.join("notes/moc");
        std::fs::create_dir_all(&moc_dir)?;
        for summary in summaries {
            let body = format!("{}\n\nMembers: {}", summary.summary, summary.member_count);
            let path = moc_dir.join(format!("community-{}.md", summary.community_id));
            std::fs::write(
                path,
                format!(
                    "---\ntype: moc\nid: {}\ntitle: \"{}\"\ntags: [entrypoint, community]\ncreated_by: agent\ncreated_via: cli\n---\n{}\n",
                    kgx_core::util::new_ulid(),
                    summary.title.replace('"', "\\\""),
                    body
                ),
            )?;
        }
    }
    let approx_in: u32 = notes
        .iter()
        .filter(|n| embedded_ids.contains(n.fm.id.as_str()))
        .map(|n| (n.body.len() / 4) as u32)
        .sum();
    append(
        &kg_dir,
        &TokenRecord {
            model: "kgx-embed".into(),
            operation: "embed".into(),
            command: "index".into(),
            input_tokens: approx_in,
            output_tokens: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
            correlation_id: kgx_core::util::new_ulid(),
            ts: kgx_core::util::now_iso(),
        },
    )?;
    std::fs::write(
        kg_dir.join("meta.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "last_index": kgx_core::util::now_iso(),
            "nodes": stats.nodes,
            "edges": stats.edges,
        }))?,
    )?;
    emit("index", stats, json, start, |s| {
        println!("\u{2714} indexed {} nodes, {} edges", s.nodes, s.edges)
    });
    Ok(())
}

fn find_changed_ids(brain: &Brain, notes: &[Note]) -> anyhow::Result<Vec<String>> {
    use std::collections::BTreeMap;
    // Pull the stored content fingerprint per note id. We hash body only
    // (raw_text), not title+body, because raw_text is what's persisted and
    // re-derivable here — see the inline note below on the title-edit
    // trade-off.
    let stored: BTreeMap<String, u64> = {
        let mut stmt = brain
            .conn()
            .prepare("SELECT id, raw_text FROM notes")
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                let id: String = r.get(0)?;
                let body: String = r.get::<_, String>(1).unwrap_or_default();
                // We can't recover the original title from raw_text alone
                // (raw_text is just the body), but body-only hashing is
                // sufficient to detect edits — title edits without a body
                // change are vanishingly rare and the next --full catches them.
                Ok((id, hash_str(&body)))
            })
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
        let mut m = BTreeMap::new();
        for r in rows {
            let (id, h) = r.map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
            m.insert(id, h);
        }
        m
    };

    let mut changed = Vec::new();
    for n in notes {
        let cur_hash = hash_str(&n.body);
        match stored.get(n.fm.id.as_str()) {
            None => changed.push(n.fm.id.clone()),      // new note
            Some(prev) if *prev != cur_hash => changed.push(n.fm.id.clone()), // edited
            Some(_) => { /* unchanged */ }
        }
    }
    // Removed notes (in brain, not in vault) don't need re-embedding —
    // they'll be pruned by build_incremental's full edge recompute, and
    // the next --full cleans them fully. Don't add them to changed (nothing
    // to embed).
    changed.sort();
    changed.dedup();
    Ok(changed)
}

fn hash_str(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
