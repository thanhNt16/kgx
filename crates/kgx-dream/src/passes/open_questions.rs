use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    util, Note, NoteType, Result, Status,
};
use kgx_retrieval::{search, SearchOpts};
use kgx_vault::write::render_note;

/// Find type:question notes. If search returns ≥1 relevant fact → propose Archive.
/// Detect gaps → propose AddNote for unanswered questions.
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let questions: Vec<&Note> = ctx
        .notes
        .iter()
        .filter(|n| {
            matches!(n.fm.r#type, NoteType::Question) && matches!(n.fm.status, Status::Active)
        })
        .collect();

    let mut diffs = Vec::new();

    for q in questions {
        let query = sanitize_query(&q.body);
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
            },
        )?;

        // Filter hits to active facts (not the question itself)
        let answering_facts: Vec<_> = hits
            .iter()
            .filter(|h| h.id != q.fm.id)
            .filter(|h| {
                ctx.notes
                    .iter()
                    .any(|n| n.fm.id == h.id && matches!(n.fm.r#type, NoteType::Fact))
            })
            .collect();

        if !answering_facts.is_empty() {
            // Question is answered — archive it
            let mut archived = q.clone();
            archived.fm.status = Status::Archived;

            let citation_ids: Vec<String> = answering_facts.iter().map(|h| h.id.clone()).collect();

            diffs.push(ProposedDiff {
                id: util::new_ulid(),
                pass: "open_questions".into(),
                kind: DiffKind::Archive,
                severity: Severity::Info,
                rationale: format!(
                    "Question '{}' answered by facts: {}",
                    q.fm.title,
                    citation_ids.join(", ")
                ),
                files: vec![FileChange {
                    rel_path: q.rel_path.display().to_string(),
                    before: Some(render_note(q)),
                    after: Some(render_note(&archived)),
                }],
            });
        }
        // Gap detection: if no answering facts found, no AddNote emitted
        // (AddNote for new question notes is a future enhancement)
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
