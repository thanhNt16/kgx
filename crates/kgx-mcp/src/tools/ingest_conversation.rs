// ingest_conversation — incremental conversation capture + finalize compilation
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(root: &Path, args: &Value) -> Result<Value> {
    let turns_val = args["turns"]
        .as_array()
        .ok_or_else(|| KgError::Validation("ingest_conversation requires 'turns' array".into()))?;

    let action = args["action"].as_str().unwrap_or("incremental");

    let turns: Vec<kgx_extract::conversation::ConversationTurn> = turns_val
        .iter()
        .map(|v| kgx_extract::conversation::ConversationTurn {
            role: v["role"].as_str().unwrap_or("user").to_string(),
            content: v["content"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    if turns.is_empty() {
        return Err(KgError::Validation(
            "ingest_conversation requires at least one turn".into(),
        ));
    }

    let provider = kgx_llm::select::provider_from_env()?;

    let report =
        kgx_extract::conversation::ingest_conversation(root, provider.as_ref(), &turns, action)
            .await?;

    Ok(json!({
        "status": "ok",
        "action": action,
        "notes_created": report.notes_created,
        "notes_updated": report.notes_updated,
        "decisions": report.decisions,
    }))
}
