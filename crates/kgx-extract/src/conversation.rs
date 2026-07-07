use kgx_core::{
    llm::{LlmProvider, LlmRequest},
    util, Confidence, CreatedBy, CreatedVia, Frontmatter, KgError, Note, NoteType, Result, Status,
};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestReport {
    pub notes_created: Vec<String>,
    pub notes_updated: Vec<String>,
    pub decisions: Vec<String>,
}

const TRANSCRIPT_DIR: &str = "raw/transcripts";

/// Ingest conversation turns into the vault.
///
/// On "incremental" calls, appends turns to a temp raw transcript note.
/// On "finalize", compiles the full transcript through the LLM to extract
/// durable facts, decisions, and preferences, then writes extracted notes.
pub async fn ingest_conversation(
    root: &Path,
    provider: &dyn LlmProvider,
    turns: &[ConversationTurn],
    action: &str,
) -> Result<IngestReport> {
    let now = util::now_iso();
    let today = &now[..10];

    let transcript_path = find_or_create_transcript(root, today)?;

    if action == "incremental" {
        append_turns(root, &transcript_path, turns, &now)?;
        return Ok(IngestReport {
            notes_created: vec![],
            notes_updated: vec![transcript_path.clone()],
            decisions: vec![],
        });
    }

    append_turns(root, &transcript_path, turns, &now)?;

    let full_transcript =
        std::fs::read_to_string(root.join(&transcript_path)).map_err(|e| KgError::Io {
            path: transcript_path.clone(),
            source: e,
        })?;

    let report = compile_judgments(provider, &full_transcript, &transcript_path).await?;

    let mut notes_created = Vec::new();
    let mut decisions = Vec::new();

    for note in &report.extracted_notes {
        kgx_vault::write::write_note(root, note)?;
        notes_created.push(note.rel_path.display().to_string());
        if note.fm.r#type == NoteType::Decision {
            decisions.push(note.fm.title.clone());
        }
    }

    Ok(IngestReport {
        notes_created,
        notes_updated: vec![transcript_path],
        decisions,
    })
}

/// Verbatim, LLM-free conversation capture: appends turns to a raw transcript.
/// Both "incremental" and "finalize" append turns; "finalize" simply marks the
/// transcript as ready for the agent harness to extract from (the report's
/// `notes_updated` is the transcript path). No extraction is performed here —
/// the harness drives that via `upsert_note` per atomic fact/decision.
pub fn ingest_conversation_verbatim(
    root: &Path,
    turns: &[ConversationTurn],
    _action: &str,
) -> Result<IngestReport> {
    let now = util::now_iso();
    let today = &now[..10];
    let transcript_path = find_or_create_transcript(root, today)?;
    append_turns(root, &transcript_path, turns, &now)?;
    Ok(IngestReport {
        notes_created: vec![],
        notes_updated: vec![transcript_path],
        decisions: vec![],
    })
}

fn find_or_create_transcript(root: &Path, today: &str) -> Result<String> {
    let dir = root.join(TRANSCRIPT_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| KgError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;

    let prefix = format!("conversation-{today}");
    let mut existing: Option<String> = None;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".md") {
                existing = Some(format!("{TRANSCRIPT_DIR}/{name}"));
                break;
            }
        }
    }

    match existing {
        Some(path) => Ok(path),
        None => {
            let id = util::new_ulid();
            let slug = format!("{prefix}-{id}");
            let rel = format!("{TRANSCRIPT_DIR}/{slug}.md");
            let content = format!(
                "---\ntype: source\nid: {id}\ntitle: \"Conversation {today}\"\ncreated_via: mcp\n---\n"
            );
            let full = root.join(&rel);
            std::fs::write(&full, content).map_err(|e| KgError::Io {
                path: full.display().to_string(),
                source: e,
            })?;
            Ok(rel)
        }
    }
}

fn append_turns(root: &Path, rel: &str, turns: &[ConversationTurn], now: &str) -> Result<()> {
    let full = root.join(rel);
    let mut content = String::new();
    for turn in turns {
        content.push_str(&format!("\n## {} ({now})\n\n{}\n", turn.role, turn.content));
    }
    std::fs::OpenOptions::new()
        .append(true)
        .create(false)
        .open(&full)
        .map_err(|e| KgError::Io {
            path: rel.to_string(),
            source: e,
        })?
        .write_all(content.as_bytes())
        .map_err(|e| KgError::Io {
            path: rel.to_string(),
            source: e,
        })
}

struct CompiledReport {
    extracted_notes: Vec<Note>,
}

async fn compile_judgments(
    provider: &dyn LlmProvider,
    transcript: &str,
    source_rel: &str,
) -> Result<CompiledReport> {
    let source_link = format!("[[{source_rel}]]");
    let prompt = format!(
        r#"You are a judgment compiler. Given a conversation transcript, extract durable facts, decisions, preferences, and friction points. Discard ephemera (greetings, small talk, irrelevant tangents).

Transcript:
---
{transcript}
---

Return JSON with this exact structure:
{{
  "judgments": [
    {{
      "type": "fact|decision|preference|friction",
      "title": "short title",
      "body": "one-sentence summary of the durable knowledge",
      "confidence": "high|medium|low",
      "entities": ["entity1", "entity2"]
    }}
  ]
}}"#
    );

    let resp = provider
        .complete(LlmRequest {
            system: "You extract durable knowledge from conversations. Return only valid JSON."
                .into(),
            prompt,
            max_tokens: 2048,
            temperature: 0.0,
        })
        .await?;

    let v: serde_json::Value = serde_json::from_str(&resp.text)
        .map_err(|e| KgError::Llm(format!("failed to parse compilation JSON: {e}")))?;

    let judgments = v["judgments"].as_array().cloned().unwrap_or_default();

    let now = util::now_iso();
    let mut extracted_notes = Vec::new();

    for j in judgments {
        let type_str = j["type"].as_str().unwrap_or("fact");
        let title = j["title"].as_str().unwrap_or("").trim().to_string();
        if title.is_empty() {
            continue;
        }
        let body = j["body"].as_str().unwrap_or(&title).trim().to_string();
        let conf = match j["confidence"].as_str().unwrap_or("medium") {
            "high" => Confidence::High,
            "low" => Confidence::Low,
            _ => Confidence::Medium,
        };
        let entities: Vec<String> = j["entities"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|e| e.as_str())
            .map(|e| format!("[[{e}]]"))
            .collect();

        let note_type = match type_str {
            "decision" => NoteType::Decision,
            "preference" => NoteType::Preference,
            "friction" => NoteType::Friction,
            _ => NoteType::Fact,
        };

        let dir = match note_type {
            NoteType::Decision => "decisions",
            NoteType::Preference => "preferences",
            NoteType::Friction => "frictions",
            _ => "facts",
        };

        let id = util::new_ulid();
        extracted_notes.push(Note {
            rel_path: std::path::PathBuf::from(format!("notes/{dir}/{}.md", util::slugify(&title))),
            body: body.clone(),
            fm: Frontmatter {
                r#type: note_type,
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
                links: entities,
                entity_type: None,
                aliases: vec![],
                created_by: CreatedBy::Agent,
                created_via: CreatedVia::Mcp,
                extra: Default::default(),
            },
        });
    }

    Ok(CompiledReport { extracted_notes })
}
