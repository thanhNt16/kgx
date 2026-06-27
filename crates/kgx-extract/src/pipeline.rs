use kgx_core::{
    llm::{LlmProvider, LlmRequest},
    util, Confidence, CreatedBy, CreatedVia, Frontmatter, KgError, Note, NoteType, Result, Status,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum Intensity {
    Lite,
    Full,
    Ultra,
}

#[derive(Debug)]
pub struct ExtractResult {
    pub notes: Vec<Note>,
    pub tokens: (u32, u32),
}

pub async fn extract(
    provider: &dyn LlmProvider,
    source: &Note,
    _intensity: Intensity,
) -> Result<ExtractResult> {
    let prompt = crate::prompt::extract_prompt(&source.body, None);
    let resp = provider
        .complete(LlmRequest {
            system: crate::prompt::EXTRACT_SYSTEM.into(),
            prompt,
            max_tokens: 1024,
            temperature: 0.0,
        })
        .await?;
    let v: serde_json::Value = serde_json::from_str(&resp.text)
        .map_err(|e| KgError::Llm(format!("bad extract json: {e}")))?;
    let stem = source
        .rel_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("source");
    let source_link = format!("[[raw/{stem}]]");
    let now = util::now_iso();
    let mut notes = Vec::new();
    for f in v["facts"].as_array().cloned().unwrap_or_default() {
        let title = f["title"].as_str().unwrap_or("").trim().to_string();
        if title.is_empty() {
            continue;
        }
        let body = f["body"].as_str().unwrap_or(&title).trim().to_string();
        let conf = match f["confidence"].as_str().unwrap_or("medium") {
            "high" => Confidence::High,
            "low" => Confidence::Low,
            _ => Confidence::Medium,
        };
        let links: Vec<String> = f["entities"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|e| e.as_str())
            .map(|e| format!("[[{e}]]"))
            .collect();
        let id = util::new_ulid();
        notes.push(Note {
            rel_path: PathBuf::from(format!("notes/facts/{}.md", util::slugify(&title))),
            body: body.clone(),
            fm: Frontmatter {
                r#type: NoteType::Fact,
                id,
                title,
                status: Status::Active,
                valid_from: Some(now[..10].to_string()),
                valid_to: None,
                recorded_at: Some(now.clone()),
                supersedes: vec![],
                superseded_by: None,
                source: Some(source_link.clone()),
                confidence: conf,
                sources_count: 1,
                tags: vec![],
                links,
                entity_type: None,
                aliases: vec![],
                created_by: CreatedBy::Agent,
                created_via: CreatedVia::Cli,
                extra: Default::default(),
            },
        });
    }
    Ok(ExtractResult {
        notes,
        tokens: (resp.input_tokens, resp.output_tokens),
    })
}
