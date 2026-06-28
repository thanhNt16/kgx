use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    llm::LlmRequest,
    util, Note, Result, Status,
};
use kgx_vault::write::render_note;

const COSINE_THRESHOLD: f32 = 0.92;

/// Compute cosine similarity between two vectors.
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < 1e-9 || nb < 1e-9 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Find near-duplicate notes (cosine > threshold), ask LLM MERGE,
/// keep canonical (lowest ULID), archive duplicate. ADD-only bias.
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let active: Vec<&Note> = ctx
        .notes
        .iter()
        .filter(|n| matches!(n.fm.status, Status::Active))
        .collect();

    if active.is_empty() {
        return Ok(vec![]);
    }

    // Compute embeddings for all active notes
    let bodies: Vec<String> = active.iter().map(|n| n.body.clone()).collect();
    let embeddings = ctx.embedder.embed(&bodies)?;

    let mut diffs = Vec::new();
    let mut merged: std::collections::BTreeSet<usize> = Default::default();

    for i in 0..active.len() {
        if merged.contains(&i) {
            continue;
        }
        for j in (i + 1)..active.len() {
            if merged.contains(&j) {
                continue;
            }
            let sim = cosine(&embeddings[i], &embeddings[j]);
            if sim < COSINE_THRESHOLD {
                continue;
            }
            // Ask LLM whether to merge
            let resp = ctx
                .provider
                .complete(LlmRequest {
                    system: "Reply JSON {merge: bool, keep: string, rationale: string}".into(),
                    prompt: format!(
                        "MERGE\nA id={} title={}\nB id={} title={}",
                        active[i].fm.id, active[i].fm.title, active[j].fm.id, active[j].fm.title
                    ),
                    max_tokens: 256,
                    temperature: 0.0,
                })
                .await?;
            let v: serde_json::Value =
                serde_json::from_str(&resp.text).unwrap_or(serde_json::json!({"merge": false}));

            // The MockProvider returns {merge: false} for MERGE prompts.
            // Only merge if LLM says so OR similarity is very high (>=0.99).
            let should_merge = v["merge"].as_bool().unwrap_or(false) || sim >= 0.99;
            if !should_merge {
                continue;
            }

            // Canonical = lowest ULID (oldest)
            let (canonical_idx, dup_idx) = if active[i].fm.id <= active[j].fm.id {
                (i, j)
            } else {
                (j, i)
            };
            let canonical = active[canonical_idx];
            let dup = active[dup_idx];

            let mut archived = (*dup).clone();
            archived.fm.status = Status::Archived;
            archived.fm.superseded_by = Some(canonical.fm.id.clone());

            diffs.push(ProposedDiff {
                id: util::new_ulid(),
                pass: "dedup".into(),
                kind: DiffKind::Merge,
                severity: Severity::Soft,
                rationale: v["rationale"]
                    .as_str()
                    .unwrap_or("near-duplicate detected")
                    .to_string(),
                files: vec![FileChange {
                    rel_path: dup.rel_path.display().to_string(),
                    before: Some(render_note(dup)),
                    after: Some(render_note(&archived)),
                }],
            });
            merged.insert(dup_idx);
        }
    }

    Ok(diffs)
}
