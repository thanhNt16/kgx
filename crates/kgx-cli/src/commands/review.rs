use crate::output::emit;
use kgx_core::diff::{ProposedDiff, Severity};
use std::collections::BTreeSet;
use std::time::Instant;

pub fn run(
    json: bool,
    approve: Option<String>,
    _reject: Option<String>,
    interactive: bool,
    ponytail_audit: bool,
) -> anyhow::Result<()> {
    if interactive {
        use std::io::IsTerminal;
        if !std::io::stdin().is_terminal() {
            anyhow::bail!("--interactive requires a terminal (stdin is not a TTY)");
        }
        // Interactive TUI not yet implemented; fall through to non-interactive
    }
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let staged_path = root.join(".kg/staged_diffs.json");
    let staged: Vec<ProposedDiff> = if staged_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&staged_path)?)?
    } else {
        vec![]
    };
    let approve_all = approve.as_deref() == Some("all");
    let approve_ids: BTreeSet<String> = approve
        .as_deref()
        .filter(|value| *value != "all")
        .map(|value| {
            value
                .split(',')
                .map(|part| part.trim().to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut applied = 0u32;
    let mut blocked_hard = 0u32;
    let mut audit_flags = Vec::new();
    for diff in &staged {
        let explicitly_approved = approve_ids.contains(&diff.id);
        let selected =
            explicitly_approved || (approve_all && !matches!(diff.severity, Severity::Hard));
        if matches!(diff.severity, Severity::Hard) && !explicitly_approved {
            blocked_hard += 1;
            continue;
        }
        if !selected {
            continue;
        }
        if ponytail_audit {
            for flag in kgx_ponytail::audit_diff(diff) {
                audit_flags.push(format!("{}: {}", flag.code, flag.msg));
            }
        }
        for file in &diff.files {
            if let Some(after) = &file.after {
                let path = root.join(&file.rel_path);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, after)?;
                applied += 1;
            }
        }
    }
    emit(
        "review",
        serde_json::json!({
            "applied": applied,
            "blocked_hard": blocked_hard,
            "audit_flags": audit_flags,
        }),
        json,
        start,
        |_| println!("applied {applied}; {blocked_hard} hard diff(s) blocked"),
    );
    Ok(())
}
