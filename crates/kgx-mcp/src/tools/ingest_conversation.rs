// ingest_conversation — incremental conversation capture (verbatim, no LLM).
//
// Appends turns to a raw transcript note. There is no LLM finalize step here:
// fact/decision extraction from the transcript is the agent harness's job —
// drive it through the kgx:ingest / kgx:extract methodology and `upsert_note`.
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

    // Verbatim capture only — no LLM. Both "incremental" and "finalize" append
    // turns to the same raw transcript; "finalize" additionally signals that
    // the transcript is ready for the harness to extract from.
    let report = kgx_extract::conversation::ingest_conversation_verbatim(root, &turns, action)?;

    Ok(json!({
        "status": "ok",
        "action": action,
        "transcript": report.notes_updated,
        "note": if action == "finalize" {
            "transcript finalized — extract facts/decisions via the kgx:ingest methodology (upsert_note per atomic fact)"
        } else {
            "turns appended to transcript"
        }
    }))
}
