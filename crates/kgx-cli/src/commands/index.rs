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

    let stats = if rebuild_vectors || (incremental && !full) {
        let changed_ids = find_changed_ids(&brain, &notes)?;
        build_incremental(&mut brain, &notes, &changed_ids, embedder.as_ref())?
    } else {
        build_full(&mut brain, &notes, embedder.as_ref())?
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
    let approx_in: u32 = notes.iter().map(|n| (n.body.len() / 4) as u32).sum();
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
    use std::collections::BTreeSet;
    let existing: BTreeSet<String> = {
        let mut stmt = brain
            .conn()
            .prepare("SELECT id FROM notes")
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(|e| kgx_core::KgError::Brain(e.to_string()))?
    };

    let current: BTreeSet<String> = notes.iter().map(|n| n.fm.id.clone()).collect();

    let added: Vec<String> = current.difference(&existing).cloned().collect();
    let removed: Vec<String> = existing.difference(&current).cloned().collect();

    let mut changed = Vec::new();
    changed.extend(added);
    changed.extend(removed);
    let remaining: Vec<String> = current.intersection(&existing).cloned().collect();
    changed.extend(remaining);

    changed.sort();
    changed.dedup();
    Ok(changed)
}
