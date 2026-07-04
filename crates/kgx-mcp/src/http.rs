use crate::tools::{dispatch, tool_schemas};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use kgx_core::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    root: PathBuf,
    start: Instant,
}

pub async fn serve(port: u16) -> Result<()> {
    let state = AppState {
        root: std::env::current_dir().map_err(|e| kgx_core::KgError::Other(e.to_string()))?,
        start: Instant::now(),
    };

    let app = Router::new()
        .route("/jsonrpc", post(handle_jsonrpc))
        .route("/health", get(handle_health))
        .route("/hooks/conversation", post(handle_conversation_hook))
        .route("/briefing/:project", get(handle_briefing))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| kgx_core::KgError::Other(e.to_string()))?;
    axum::serve(listener, app)
        .await
        .map_err(|e| kgx_core::KgError::Other(e.to_string()))?;
    Ok(())
}

async fn handle_jsonrpc(
    State(state): State<AppState>,
    Json(msg): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let id = msg["id"].clone();
    let method = msg["method"].as_str().unwrap_or("");

    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {"name": "kgx", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"tools": {}}
        }),
        "tools/list" => json!({"tools": tool_schemas()}),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            let args = &msg["params"]["arguments"];
            match dispatch(&state.root, name, args).await {
                Ok(out) => {
                    json!({"content": [{"type": "text", "text": serde_json::to_string(&out).unwrap_or_default()}]})
                }
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(
                            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}),
                        ),
                    );
                }
            }
        }
        _ => json!({"error": "unknown method"}),
    };

    (
        StatusCode::OK,
        Json(json!({"jsonrpc": "2.0", "id": id, "result": result})),
    )
}

async fn handle_health(State(state): State<AppState>) -> Json<Value> {
    let uptime = state.start.elapsed().as_secs();
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": uptime,
    }))
}

async fn handle_conversation_hook(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let turns = payload["turns"].as_array();
    let action = payload["action"].as_str().unwrap_or("unknown");

    let session_id = Uuid::new_v4().to_string();
    let body = format!(
        "## Conversation Hook\n\n\
         - **action**: {action}\n\
         - **turns**: {}\n\
         - **session**: {session_id}\n\n",
        turns
            .map(|t| t.len().to_string())
            .unwrap_or_else(|| "0".to_string()),
    );

    let note = kgx_core::Note {
        fm: kgx_core::Frontmatter {
            r#type: kgx_core::NoteType::Experience,
            id: session_id.clone(),
            title: format!("Conversation Hook \u{2014} {action}"),
            status: kgx_core::Status::Active,
            valid_from: None,
            valid_to: None,
            recorded_at: None,
            supersedes: vec![],
            superseded_by: None,
            source: None,
            confidence: kgx_core::Confidence::High,
            sources_count: 0,
            tags: vec!["conversation".to_string(), "hook".to_string()],
            links: vec![],
            entity_type: None,
            aliases: vec![],
            created_by: kgx_core::CreatedBy::Agent,
            created_via: kgx_core::CreatedVia::Mcp,
            extra: std::collections::BTreeMap::new(),
        },
        body,
        rel_path: std::path::PathBuf::from("wiki/hooks"),
    };

    match kgx_vault::write::write_note(&state.root, &note) {
        Ok(_) => Json(json!({"status": "ok", "session_id": session_id})),
        Err(e) => Json(json!({"status": "error", "message": e.to_string()})),
    }
}

async fn handle_briefing(
    State(state): State<AppState>,
    axum::extract::Path(project): axum::extract::Path<String>,
) -> Json<Value> {
    let brain_path = if project == "_default" || project == "default" {
        state.root.join(".kg/brain.sqlite")
    } else {
        state
            .root
            .join("projects")
            .join(&project)
            .join(".kg/brain.sqlite")
    };

    let info = if brain_path.exists() {
        match kgx_graph::Brain::open(&brain_path) {
            Ok(brain) => {
                let node_count: i64 = brain
                    .conn()
                    .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
                    .unwrap_or(0);
                let edge_count: i64 = brain
                    .conn()
                    .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0))
                    .unwrap_or(0);
                let community_ids: Vec<String> = brain
                    .conn()
                    .prepare("SELECT DISTINCT community_id FROM communities LIMIT 20")
                    .ok()
                    .map(|mut stmt| {
                        stmt.query_map([], |r| r.get::<_, String>(0))
                            .ok()
                            .map(|rows| rows.filter_map(|r| r.ok()).collect())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let recent: Vec<String> = brain
                    .conn()
                    .prepare("SELECT id FROM notes ORDER BY rowid DESC LIMIT 5")
                    .ok()
                    .map(|mut stmt| {
                        stmt.query_map([], |r| r.get::<_, String>(0))
                            .ok()
                            .map(|rows| rows.filter_map(|r| r.ok()).collect())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                json!({
                    "node_count": node_count,
                    "edge_count": edge_count,
                    "community_count": community_ids.len(),
                    "recent_notes": recent,
                })
            }
            Err(e) => json!({"error": e.to_string()}),
        }
    } else {
        json!({"status": "no_brain", "hint": "run `kg index` first"})
    };

    Json(json!({
        "project": project,
        "info": info,
        "tool_count": tool_schemas()["tools"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0),
    }))
}
