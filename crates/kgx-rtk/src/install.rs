use kgx_core::{KgError, Result};
use std::path::Path;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Tool {
    ClaudeCode,
    Codex,
    Cursor,
}

pub fn install_hooks(tool: Tool, root: &Path) -> Result<()> {
    let write = |rel: &str, content: &str| -> Result<()> {
        let p = root.join(rel);
        std::fs::create_dir_all(
            p.parent()
                .ok_or_else(|| KgError::Other("path has no parent".into()))?,
        )
        .map_err(|e| KgError::Io {
            path: rel.into(),
            source: e,
        })?;
        std::fs::write(&p, content).map_err(|e| KgError::Io {
            path: rel.into(),
            source: e,
        })
    };
    match tool {
        Tool::ClaudeCode => write(
            ".claude/settings.json",
            r#"{
  "hooks": {
    "PostToolUse": [
      { "matcher": "Bash",
        "hooks": [ { "type": "command", "command": "rtk compress" } ] }
    ]
  }
}"#,
        ),
        Tool::Codex => write(
            ".codex/rtk.toml",
            "# pipe shell output through rtk\n[output]\nfilter = \"rtk compress\"\n",
        ),
        Tool::Cursor => write(
            ".cursor/rtk.json",
            r#"{ "terminal.outputFilter": "rtk compress" }"#,
        ),
    }
}
