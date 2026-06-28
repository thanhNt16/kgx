use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    llm::LlmRequest,
    util, Note, NoteType, Result, Status,
};
use kgx_vault::write::render_note;

/// Find pairs of active facts sharing a tag where one is strictly newer.
/// Ask the LLM whether contradiction exists; if hard/scope, emit a Supersede diff.
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let mut diffs = Vec::new();
    let facts: Vec<&Note> = ctx
        .notes
        .iter()
        .filter(|n| matches!(n.fm.r#type, NoteType::Fact) && matches!(n.fm.status, Status::Active))
        .collect();

    for (i, a) in facts.iter().enumerate() {
        for b in facts.iter().skip(i + 1) {
            if !share_subject(a, b) {
                continue;
            }
            let (older, newer) = order_by_valid_from(a, b);
            if !llm_supersedes(ctx, older, newer).await? {
                continue;
            }
            let mut updated = (*older).clone();
            updated.fm.status = Status::Superseded;
            let today = &util::now_iso()[..10];
            updated.fm.valid_to = Some(
                newer
                    .fm
                    .valid_from
                    .clone()
                    .unwrap_or_else(|| today.to_string()),
            );
            updated.fm.superseded_by = Some(newer.fm.id.clone());
            diffs.push(ProposedDiff {
                id: util::new_ulid(),
                pass: "supersession".into(),
                kind: DiffKind::Supersede,
                severity: Severity::Soft,
                rationale: format!(
                    "'{}' superseded by newer '{}'",
                    older.fm.title, newer.fm.title
                ),
                files: vec![FileChange {
                    rel_path: older.rel_path.display().to_string(),
                    before: Some(render_note(older)),
                    after: Some(render_note(&updated)),
                }],
            });
        }
    }

    diffs.sort_by(|x, y| x.files[0].rel_path.cmp(&y.files[0].rel_path));
    Ok(diffs)
}

fn share_subject(a: &Note, b: &Note) -> bool {
    a.fm.tags.iter().any(|t| b.fm.tags.contains(t))
}

fn order_by_valid_from<'a>(a: &'a Note, b: &'a Note) -> (&'a Note, &'a Note) {
    let av = a.fm.valid_from.clone().unwrap_or_default();
    let bv = b.fm.valid_from.clone().unwrap_or_default();
    if av <= bv {
        (a, b)
    } else {
        (b, a)
    }
}

async fn llm_supersedes(ctx: &DreamContext<'_>, older: &Note, newer: &Note) -> Result<bool> {
    let prompt = format!("CONTRADICTION\nOLD: {}\nNEW: {}", older.body, newer.body);
    let resp = ctx
        .provider
        .complete(LlmRequest {
            system: "Reply JSON {verdict: agree|soft|scope|hard, rationale}".into(),
            prompt,
            max_tokens: 256,
            temperature: 0.0,
        })
        .await?;
    let v: serde_json::Value =
        serde_json::from_str(&resp.text).unwrap_or(serde_json::json!({"verdict": "agree"}));
    Ok(matches!(
        v["verdict"].as_str(),
        Some("hard") | Some("scope")
    ))
}
