use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    llm::LlmRequest,
    util, Note, NoteType, Result, Status,
};

/// For same-subject active fact pairs, classify via LLM.
/// Emits FlagContradiction with mapped severity (hard→Hard, scope→Scope, soft→Soft).
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let mut diffs = Vec::new();
    let facts: Vec<&Note> = ctx
        .notes
        .iter()
        .filter(|n| matches!(n.fm.r#type, NoteType::Fact) && matches!(n.fm.status, Status::Active))
        .collect();

    for (i, a) in facts.iter().enumerate() {
        for b in facts.iter().skip(i + 1) {
            if !a.fm.tags.iter().any(|t| b.fm.tags.contains(t)) {
                continue;
            }
            let resp = ctx
                .provider
                .complete(LlmRequest {
                    system: "Reply JSON {verdict: agree|soft|scope|hard, rationale}".into(),
                    prompt: format!("CONTRADICTION\nA: {}\nB: {}", a.body, b.body),
                    max_tokens: 256,
                    temperature: 0.0,
                })
                .await?;
            let v: serde_json::Value =
                serde_json::from_str(&resp.text).unwrap_or(serde_json::json!({"verdict": "agree"}));
            let sev = match v["verdict"].as_str() {
                Some("hard") => Severity::Hard,
                Some("scope") => Severity::Scope,
                Some("soft") => Severity::Soft,
                _ => continue,
            };
            diffs.push(ProposedDiff {
                id: util::new_ulid(),
                pass: "contradiction".into(),
                kind: DiffKind::FlagContradiction,
                severity: sev,
                rationale: v["rationale"]
                    .as_str()
                    .unwrap_or("conflict detected")
                    .to_string(),
                files: vec![
                    FileChange {
                        rel_path: a.rel_path.display().to_string(),
                        before: None,
                        after: None,
                    },
                    FileChange {
                        rel_path: b.rel_path.display().to_string(),
                        before: None,
                        after: None,
                    },
                ],
            });
        }
    }

    diffs.sort_by(|x, y| x.id.cmp(&y.id));
    Ok(diffs)
}
