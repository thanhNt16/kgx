use crate::tools::{dispatch, tool_schemas};
use kgx_core::Result;
use serde_json::{json, Value};
use std::path::Path;

pub async fn handle_message(root: &Path, msg: Value) -> Result<Value> {
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
            match dispatch(root, name, args).await {
                Ok(out) => {
                    json!({"content": [{"type": "text", "text": serde_json::to_string(&out).unwrap_or_default()}]})
                }
                Err(e) => {
                    return Ok(
                        json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":e.to_string()}}),
                    )
                }
            }
        }
        _ => json!({"error": "unknown method"}),
    };
    Ok(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}
