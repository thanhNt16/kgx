use kgx_core::{KgError, Result};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Resolve the knowledge-vault root for a single message.
///
/// `project` is the process cwd (the directory Claude Code launched the
/// server from). The vault lives at `<project>/.brain` if it exists; otherwise
/// we fall back to `project` itself so that `initialize` / `tools/list`
/// succeed from any directory and only vault-needing tool *calls* surface a
/// helpful per-call error (instead of killing the server at startup).
fn vault_root_for(project: &Path) -> PathBuf {
    let brain = project.join(".brain");
    if brain.is_dir() {
        brain
    } else {
        project.to_path_buf()
    }
}

pub async fn serve_stdio(project: PathBuf) -> Result<()> {
    let mut reader = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| KgError::Other(e.to_string()))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        // Resolve the vault lazily per message so the server stays up and
        // answers initialize/tools-list even outside an initialized vault.
        let root = vault_root_for(&project);
        let resp = match crate::protocol::handle_message(&root, msg).await {
            Ok(r) => r,
            Err(e) => {
                let err = serde_json::json!({"jsonrpc":"2.0","id":serde_json::Value::Null,"error":{"code":-32603,"message":e.to_string()}});
                let out = serde_json::to_string(&err).unwrap_or_default();
                stdout
                    .write_all(out.as_bytes())
                    .await
                    .map_err(|e| KgError::Other(e.to_string()))?;
                stdout
                    .write_all(b"\n")
                    .await
                    .map_err(|e| KgError::Other(e.to_string()))?;
                stdout
                    .flush()
                    .await
                    .map_err(|e| KgError::Other(e.to_string()))?;
                continue;
            }
        };
        let out = serde_json::to_string(&resp).map_err(|e| KgError::Other(e.to_string()))?;
        stdout
            .write_all(out.as_bytes())
            .await
            .map_err(|e| KgError::Other(e.to_string()))?;
        stdout
            .write_all(b"\n")
            .await
            .map_err(|e| KgError::Other(e.to_string()))?;
        stdout
            .flush()
            .await
            .map_err(|e| KgError::Other(e.to_string()))?;
    }
    Ok(())
}
