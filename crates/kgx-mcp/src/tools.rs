use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn tool_schemas() -> Value {
    json!([
        {"name":"search_notes","description":"Hybrid search over the knowledge graph","inputSchema":{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"},"mode":{"type":"string"}},"required":["query"]}},
        {"name":"get_note","description":"Fetch a note by id","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}},
        {"name":"upsert_note","description":"Create or update a note","inputSchema":{"type":"object","properties":{"type":{"type":"string"},"title":{"type":"string"},"body":{"type":"string"},"id":{"type":"string"}},"required":["type","title","body"]}},
        {"name":"ask_question","description":"Hybrid Q&A with citations","inputSchema":{"type":"object","properties":{"question":{"type":"string"},"scope":{"type":"string"}},"required":["question"]}},
        {"name":"capture_raw","description":"Ingest raw content into raw/ immutably","inputSchema":{"type":"object","properties":{"content":{"type":"string"},"kind":{"type":"string"}},"required":["content"]}},
        {"name":"dream_step","description":"Run one bounded dream iteration, returns staged diffs","inputSchema":{"type":"object","properties":{"only":{"type":"string"},"max_iterations":{"type":"integer"}}}}
    ])
}

pub async fn dispatch(root: &Path, name: &str, args: &Value) -> Result<Value> {
    match name {
        "search_notes" => {
            let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))?;
            let embedder = kgx_llm::select::embedder_from_env();
            let mode = match args["mode"].as_str() {
                Some("keyword") => kgx_retrieval::Mode::Keyword,
                Some("semantic") => kgx_retrieval::Mode::Semantic,
                _ => kgx_retrieval::Mode::Hybrid,
            };
            let hits = kgx_retrieval::search(
                &brain,
                embedder.as_ref(),
                args["query"].as_str().unwrap_or(""),
                kgx_retrieval::SearchOpts {
                    mode,
                    limit: args["limit"].as_u64().unwrap_or(10) as usize,
                    expand_ppr: true,
                },
            )?;
            Ok(json!(hits))
        }
        "get_note" => {
            let notes = kgx_vault::scan::scan_vault(root)?;
            let id = args["id"].as_str().unwrap_or("");
            let note = notes
                .into_iter()
                .find(|n| n.fm.id == id)
                .ok_or_else(|| KgError::NotFound(id.into()))?;
            Ok(json!({
                "id": note.fm.id,
                "title": note.fm.title,
                "body": note.body,
                "path": note.rel_path.display().to_string()
            }))
        }
        "ask_question" => {
            let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))?;
            let notes = kgx_vault::scan::scan_vault(root)?;
            let embedder = kgx_llm::select::embedder_from_env();
            let question = args["question"].as_str().unwrap_or("");
            let context = if args["scope"].as_str() == Some("global") {
                kgx_retrieval::global::global_context(&brain, question, embedder.as_ref(), 5)?
            } else {
                let hits = kgx_retrieval::search(
                    &brain,
                    embedder.as_ref(),
                    question,
                    kgx_retrieval::SearchOpts::default(),
                )?;
                hits.iter()
                    .filter_map(|h| notes.iter().find(|n| n.fm.id == h.id))
                    .map(|n| format!("[{}] {}: {}", n.fm.id, n.fm.title, n.body))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let provider = kgx_llm::select::provider_from_env()?;
            let resp = provider
                .complete(kgx_core::llm::LlmRequest {
                    system: "Answer from context, cite ids".into(),
                    prompt: format!("ANSWER_QUESTION\nContext:\n{context}\nQuestion: {question}"),
                    max_tokens: 1024,
                    temperature: 0.0,
                })
                .await?;
            Ok(serde_json::from_str(&resp.text)
                .unwrap_or_else(|_| json!({"answer": resp.text, "citations": []})))
        }
        "capture_raw" => {
            let content = args["content"].as_str().unwrap_or("");
            let title = content.lines().next().unwrap_or("capture");
            let stem = kgx_core::util::slugify(title);
            let rel = format!("raw/{}-{stem}.md", &kgx_core::util::now_iso()[..10]);
            let path = root.join(&rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| KgError::Io {
                    path: parent.display().to_string(),
                    source: e,
                })?;
            }
            if !path.exists() {
                std::fs::write(
                    &path,
                    format!(
                        "---\ntype: source\nid: {}\ntitle: \"{}\"\ncreated_via: mcp\n---\n{content}\n",
                        kgx_core::util::new_ulid(),
                        title.replace('"', "\\\"")
                    ),
                )
                .map_err(|e| KgError::Io {
                    path: path.display().to_string(),
                    source: e,
                })?;
            }
            Ok(json!({"raw": rel}))
        }
        "upsert_note" => upsert_note(root, args),
        "dream_step" => dream_step(root, args).await,
        other => Err(KgError::NotFound(format!("tool {other}"))),
    }
}

fn upsert_note(root: &Path, args: &Value) -> Result<Value> {
    use kgx_core::{Confidence, CreatedBy, CreatedVia, Frontmatter, Note, Status};

    let note_type = parse_note_type(args["type"].as_str().unwrap_or("fact"))?;
    let title = args["title"].as_str().unwrap_or("").trim();
    let body = args["body"].as_str().unwrap_or("").trim();
    if title.is_empty() {
        return Err(KgError::Other("upsert_note requires title".into()));
    }

    let requested_id = args["id"]
        .as_str()
        .filter(|id| !id.trim().is_empty())
        .map(ToString::to_string);
    let existing = if let Some(id) = requested_id.as_deref() {
        kgx_vault::scan::scan_vault(root)?
            .into_iter()
            .find(|note| note.fm.id == id)
    } else {
        None
    };

    let id = requested_id
        .or_else(|| existing.as_ref().map(|note| note.fm.id.clone()))
        .unwrap_or_else(kgx_core::util::new_ulid);
    let rel_path = existing
        .as_ref()
        .map(|note| note.rel_path.clone())
        .unwrap_or_else(|| note_rel_path(note_type, title));

    let fm = if let Some(existing) = existing {
        Frontmatter {
            r#type: note_type,
            id,
            title: title.to_string(),
            created_by: CreatedBy::Agent,
            created_via: CreatedVia::Mcp,
            ..existing.fm
        }
    } else {
        Frontmatter {
            r#type: note_type,
            id,
            title: title.to_string(),
            status: Status::Active,
            valid_from: None,
            valid_to: None,
            recorded_at: Some(kgx_core::util::now_iso()),
            supersedes: vec![],
            superseded_by: None,
            source: None,
            confidence: Confidence::Medium,
            sources_count: 0,
            tags: vec![],
            links: vec![],
            entity_type: None,
            aliases: vec![],
            created_by: CreatedBy::Agent,
            created_via: CreatedVia::Mcp,
            extra: Default::default(),
        }
    };

    let note = Note {
        fm,
        body: body.to_string(),
        rel_path,
    };
    kgx_vault::write::write_note(root, &note)?;
    Ok(json!({
        "status": "ok",
        "id": note.fm.id,
        "path": note.rel_path.display().to_string()
    }))
}

async fn dream_step(root: &Path, args: &Value) -> Result<Value> {
    let notes = kgx_vault::scan::scan_vault(root)?;
    let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))?;
    let provider = kgx_llm::select::provider_from_env()?;
    let embedder = kgx_llm::select::embedder_from_env();
    let passes = args["only"]
        .as_str()
        .map(|only| {
            only.split(',')
                .map(str::trim)
                .filter_map(kgx_dream::PassId::parse)
                .collect()
        })
        .unwrap_or_else(|| kgx_dream::PassId::all().to_vec());
    let max_iterations = args["max_iterations"].as_u64().unwrap_or(1) as u32;
    let ctx = kgx_dream::DreamContext {
        notes: &notes,
        brain: &brain,
        provider: provider.as_ref(),
        embedder: embedder.as_ref(),
    };
    let run = kgx_dream::run::dream(
        &ctx,
        kgx_dream::run::DreamOptions {
            passes,
            max_iterations: max_iterations.max(1),
        },
    )
    .await?;
    Ok(json!({
        "status": "ok",
        "iterations": run.iterations,
        "done_signal": run.done_signal,
        "diffs": run.diffs
    }))
}

fn parse_note_type(value: &str) -> Result<kgx_core::NoteType> {
    match value {
        "fact" => Ok(kgx_core::NoteType::Fact),
        "entity" => Ok(kgx_core::NoteType::Entity),
        "decision" => Ok(kgx_core::NoteType::Decision),
        "experience" => Ok(kgx_core::NoteType::Experience),
        "moc" => Ok(kgx_core::NoteType::Moc),
        "source" => Ok(kgx_core::NoteType::Source),
        "question" => Ok(kgx_core::NoteType::Question),
        other => Err(KgError::Other(format!("unknown note type {other}"))),
    }
}

fn note_rel_path(note_type: kgx_core::NoteType, title: &str) -> PathBuf {
    let dir = match note_type {
        kgx_core::NoteType::Fact => "notes/facts",
        kgx_core::NoteType::Entity => "notes/entities",
        kgx_core::NoteType::Decision => "notes/decisions",
        kgx_core::NoteType::Experience => "notes/experiences",
        kgx_core::NoteType::Moc => "notes/moc",
        kgx_core::NoteType::Source => "notes/sources",
        kgx_core::NoteType::Question => "notes/questions",
    };
    PathBuf::from(format!("{dir}/{}.md", kgx_core::util::slugify(title)))
}
