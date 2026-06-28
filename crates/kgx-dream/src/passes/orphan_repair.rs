use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    util, Result,
};
use kgx_graph::links::orphans;
use kgx_retrieval::{search, SearchOpts};

/// Find orphan notes and propose AddLink diffs inserting [[wikilinks]] into the body.
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let orphan_ids = orphans(ctx.notes);
    if orphan_ids.is_empty() {
        return Ok(vec![]);
    }

    // Build id→note map
    let note_by_id: std::collections::BTreeMap<&str, &kgx_core::Note> =
        ctx.notes.iter().map(|n| (n.fm.id.as_str(), n)).collect();

    let mut diffs = Vec::new();

    for orphan_id in &orphan_ids {
        let orphan = match note_by_id.get(orphan_id.as_str()) {
            Some(n) => n,
            None => continue,
        };

        // Search for related notes using the orphan's body
        let query = sanitize_query(&orphan.body);
        if query.is_empty() {
            continue;
        }

        let hits = search(
            ctx.brain,
            ctx.embedder,
            &query,
            SearchOpts {
                mode: kgx_retrieval::Mode::Hybrid,
                limit: 5,
                expand_ppr: false,
                filter_entities: true,
                rerank_graph: false,
                rerank_llm: false,
            },
            None,
        )?;

        // Collect candidate link targets (exclude self)
        let candidates: Vec<&kgx_core::Note> = hits
            .iter()
            .filter(|h| h.id != *orphan_id)
            .filter_map(|h| note_by_id.get(h.id.as_str()).copied())
            .take(3)
            .collect();

        if candidates.is_empty() {
            continue;
        }

        // Build updated body with wikilinks appended
        let new_links: String = candidates
            .iter()
            .map(|n| format!("[[{}]]", n.fm.title))
            .collect::<Vec<_>>()
            .join(" ");

        let new_body = format!("{}\n\nSee also: {}", orphan.body.trim_end(), new_links);
        let mut updated = (*orphan).clone();
        updated.body = new_body;

        // Render the updated note
        let after_text = kgx_vault::write::render_note(&updated);

        diffs.push(ProposedDiff {
            id: util::new_ulid(),
            pass: "orphan_repair".into(),
            kind: DiffKind::AddLink,
            severity: Severity::Info,
            rationale: format!(
                "Orphan '{}' linked to {} related notes",
                orphan.fm.title,
                candidates.len()
            ),
            files: vec![FileChange {
                rel_path: orphan.rel_path.display().to_string(),
                before: Some(kgx_vault::write::render_note(orphan)),
                after: Some(after_text),
            }],
        });
    }

    Ok(diffs)
}

fn sanitize_query(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
