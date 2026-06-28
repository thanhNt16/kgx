use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    llm::LlmRequest,
    util, Result,
};

/// Reads communities table; if empty, returns vec![].
/// When populated, produces a Resummarize diff per community.
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    // Query communities table
    let communities = query_communities(ctx.brain)?;
    if communities.is_empty() {
        return Ok(vec![]);
    }

    let mut diffs = Vec::new();
    for (community_id, member_ids) in communities {
        // Build context from community members
        let member_bodies: Vec<String> = ctx
            .notes
            .iter()
            .filter(|n| member_ids.contains(&n.fm.id))
            .map(|n| format!("{}: {}", n.fm.title, n.body))
            .collect();

        if member_bodies.is_empty() {
            continue;
        }

        let resp = ctx
            .provider
            .complete(LlmRequest {
                system: "Reply JSON {title: string, summary: string}".into(),
                prompt: format!("COMMUNITY_SUMMARY\n{}", member_bodies.join("\n---\n")),
                max_tokens: 512,
                temperature: 0.2,
            })
            .await?;

        let v: serde_json::Value = serde_json::from_str(&resp.text)
            .unwrap_or(serde_json::json!({"title": "Community", "summary": ""}));
        let summary = v["summary"].as_str().unwrap_or("").to_string();

        diffs.push(ProposedDiff {
            id: util::new_ulid(),
            pass: "community".into(),
            kind: DiffKind::Resummarize,
            severity: Severity::Info,
            rationale: format!("Community {} summary refresh", community_id),
            files: vec![FileChange {
                rel_path: format!("notes/moc/community-{}.md", community_id),
                before: None,
                after: Some(summary),
            }],
        });
    }

    Ok(diffs)
}

/// Returns map of community_id → list of note ids in that community.
fn query_communities(
    brain: &kgx_graph::Brain,
) -> Result<std::collections::BTreeMap<i64, Vec<String>>> {
    use kgx_core::KgError;
    let mut stmt = brain
        .conn()
        .prepare("SELECT id, community_id FROM communities ORDER BY community_id")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let cid: i64 = r.get(1)?;
            Ok((id, cid))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;

    let mut map: std::collections::BTreeMap<i64, Vec<String>> = Default::default();
    for row in rows {
        let (id, cid) = row.map_err(|e| KgError::Brain(e.to_string()))?;
        map.entry(cid).or_default().push(id);
    }
    Ok(map)
}
