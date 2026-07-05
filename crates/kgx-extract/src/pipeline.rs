use kgx_core::{
    llm::{LlmProvider, LlmRequest},
    util, Confidence, CreatedBy, CreatedVia, EntityType, Frontmatter, KgError, Note, NoteType,
    Result, Status,
};
use std::collections::BTreeMap;
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

struct ExtractedEntity {
    name: String,
    entity_type: Option<EntityType>,
}

pub async fn extract(
    provider: &dyn LlmProvider,
    source: &Note,
    intensity: Intensity,
) -> Result<ExtractResult> {
    let ponytail_intensity = match intensity {
        Intensity::Lite => kgx_ponytail::Intensity::Lite,
        Intensity::Full => kgx_ponytail::Intensity::Full,
        Intensity::Ultra => kgx_ponytail::Intensity::Ultra,
    };
    let ladder = kgx_ponytail::ladder_for(kgx_ponytail::Operation::Extract, ponytail_intensity);
    let prompt = crate::prompt::extract_prompt(&source.body, Some(ladder));
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
    let mut entity_pool: BTreeMap<String, ExtractedEntity> = BTreeMap::new();
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
        let mut links: Vec<String> = Vec::new();
        let mut relations: Vec<(String, String)> = Vec::new();
        for e in f["entities"].as_array().cloned().unwrap_or_default() {
            let (name, entity_type, rel) = if let Some(s) = e.as_str() {
                (s.to_string(), None, None)
            } else {
                let name = e["name"].as_str().unwrap_or("").trim().to_string();
                let entity_type = e["entity_type"].as_str().and_then(EntityType::parse);
                let rel = e["rel"].as_str().map(str::to_string);
                (name, entity_type, rel)
            };
            if name.is_empty() {
                continue;
            }
            links.push(format!("[[{name}]]"));
            if let Some(r) = rel.filter(|r| r != "mentions") {
                relations.push((name.clone(), r));
            }
            let entry = entity_pool
                .entry(util::slugify(&name))
                .or_insert(ExtractedEntity {
                    name,
                    entity_type: None,
                });
            if entry.entity_type.is_none() {
                entry.entity_type = entity_type;
            }
        }
        let mut extra: BTreeMap<String, serde_yaml::Value> = Default::default();
        if !relations.is_empty() {
            let seq: Vec<serde_yaml::Value> = relations
                .iter()
                .map(|(target, rel)| {
                    let mut m = serde_yaml::Mapping::new();
                    m.insert(
                        serde_yaml::Value::String("target".to_string()),
                        serde_yaml::Value::String(target.clone()),
                    );
                    m.insert(
                        serde_yaml::Value::String("rel".to_string()),
                        serde_yaml::Value::String(rel.clone()),
                    );
                    serde_yaml::Value::Mapping(m)
                })
                .collect();
            extra.insert("relations".to_string(), serde_yaml::Value::Sequence(seq));
        }
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
                extra,
            },
        });
    }
    for (slug, ent) in entity_pool {
        let id = util::new_ulid();
        notes.push(Note {
            rel_path: PathBuf::from(format!("notes/entities/{slug}.md")),
            body: ent.name.clone(),
            fm: Frontmatter {
                r#type: NoteType::Entity,
                id,
                title: ent.name,
                status: Status::Active,
                valid_from: Some(now[..10].to_string()),
                valid_to: None,
                recorded_at: Some(now.clone()),
                supersedes: vec![],
                superseded_by: None,
                source: Some(source_link.clone()),
                confidence: Confidence::Medium,
                sources_count: 1,
                tags: vec![],
                links: vec![],
                entity_type: ent.entity_type.map(|t| t.as_str().to_string()),
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
