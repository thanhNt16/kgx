// upsert_note — create or update a note file
use kgx_core::{
    Confidence, CreatedBy, CreatedVia, Frontmatter, KgError, Note, NoteType, Result, Status,
};
use serde_json::{json, Value};
use std::path::Path;

pub fn run(root: &Path, args: &Value) -> Result<Value> {
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
    let existing = if let Some(ref id) = requested_id {
        kgx_vault::scan::scan_vault(root)?
            .into_iter()
            .find(|note| note.fm.id == id.as_str())
    } else {
        None
    };

    let id = requested_id
        .or_else(|| existing.as_ref().map(|n| n.fm.id.clone()))
        .unwrap_or_else(kgx_core::util::new_ulid);
    let rel_path = existing
        .as_ref()
        .map(|n| n.rel_path.clone())
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
    Ok(json!({"status": "ok", "id": note.fm.id, "path": note.rel_path.display().to_string()}))
}

fn parse_note_type(value: &str) -> Result<NoteType> {
    match value {
        "fact" => Ok(NoteType::Fact),
        "entity" => Ok(NoteType::Entity),
        "decision" => Ok(NoteType::Decision),
        "experience" => Ok(NoteType::Experience),
        "moc" => Ok(NoteType::Moc),
        "source" => Ok(NoteType::Source),
        "question" => Ok(NoteType::Question),
        "preference" => Ok(NoteType::Preference),
        "friction" => Ok(NoteType::Friction),
        other => Err(KgError::Other(format!("unknown note type {other}"))),
    }
}

fn note_rel_path(note_type: NoteType, title: &str) -> std::path::PathBuf {
    let dir = match note_type {
        NoteType::Fact => "notes/facts",
        NoteType::Entity => "notes/entities",
        NoteType::Decision => "notes/decisions",
        NoteType::Experience => "notes/experiences",
        NoteType::Moc => "notes/moc",
        NoteType::Source => "notes/sources",
        NoteType::Question => "notes/questions",
        NoteType::Preference => "notes/preferences",
        NoteType::Friction => "notes/frictions",
    };
    std::path::PathBuf::from(format!("{dir}/{}.md", kgx_core::util::slugify(title)))
}
