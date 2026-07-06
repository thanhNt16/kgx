use kgx_core::{KgError, Result};
use std::path::Path;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Tool {
    ClaudeCode,
    Codex,
    Cursor,
    Opencode,
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
    write(
        ".kgx/hooks/verify-finished.sh",
        include_str!("../../../skills/hooks/verify-finished.sh"),
    )?;
    match tool {
        Tool::ClaudeCode => write(
            ".claude/settings.json",
            r#"{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "sh \"$(git rev-parse --show-toplevel)/.kgx/hooks/verify-finished.sh\" --json",
            "timeout": 600
          }
        ]
      }
    ],
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
        Tool::Opencode => write(
            ".opencode/rtk.md",
            "# RTK (Response Token Kiln)\n\nRTK compresses verbose Bash output. Pipe long-running or\noutput-heavy commands through `rtk compress`:\n\n    kg index --full | rtk compress\n\nOpencode has no native output filter hook, but `rtk`\nworks as a standard Unix pipe.\n",
        ),
    }
}
