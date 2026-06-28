use kgx_core::{
    llm::{LlmProvider, LlmRequest},
    KgError, Note, Result,
};
use kgx_graph::Brain;
use std::collections::BTreeMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommunitySummary {
    pub community_id: i64,
    pub title: String,
    pub summary: String,
    pub member_count: usize,
}

pub async fn summarize_all(
    brain: &Brain,
    provider: &dyn LlmProvider,
    notes: &[Note],
) -> Result<Vec<CommunitySummary>> {
    let mut members: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    {
        let mut stmt = brain
            .conn()
            .prepare("SELECT id, community_id FROM communities ORDER BY community_id, id")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for row in rows {
            let (id, cid) = row.map_err(|e| KgError::Brain(e.to_string()))?;
            members.entry(cid).or_default().push(id);
        }
    }
    let by_id: BTreeMap<&str, &Note> = notes.iter().map(|n| (n.fm.id.as_str(), n)).collect();
    let mut out = Vec::new();
    for (cid, ids) in members {
        let body = ids
            .iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .map(|n| format!("- {}: {}", n.fm.title, n.body))
            .collect::<Vec<_>>()
            .join("\n");
        let resp = provider
            .complete(LlmRequest {
                system: "Summarize this community as JSON {title, summary}.".into(),
                prompt: format!("COMMUNITY_SUMMARY\n{body}"),
                max_tokens: 512,
                temperature: 0.0,
            })
            .await?;
        let fallback = serde_json::json!({
            "title": format!("Community {cid}"),
            "summary": resp.text
        });
        let value: serde_json::Value = serde_json::from_str(&resp.text).unwrap_or(fallback);
        out.push(CommunitySummary {
            community_id: cid,
            title: value["title"]
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("Community {cid}")),
            summary: value["summary"].as_str().unwrap_or("").to_string(),
            member_count: ids.len(),
        });
    }
    brain
        .conn()
        .execute("DELETE FROM community_summaries", [])
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for summary in &out {
        brain
            .conn()
            .execute(
                "INSERT INTO community_summaries (community_id, title, summary, member_count) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    summary.community_id,
                    summary.title,
                    summary.summary,
                    summary.member_count as i64
                ],
            )
            .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    Ok(out)
}
