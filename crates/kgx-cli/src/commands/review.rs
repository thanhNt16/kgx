use crate::output::emit;
use kgx_core::diff::{ProposedDiff, Severity};
use std::collections::BTreeSet;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Apply,
    Reject,
    Keep,
}

pub(crate) fn resolve_action(
    diff: &ProposedDiff,
    approve_all: bool,
    approve_ids: &BTreeSet<String>,
    reject_all: bool,
    reject_ids: &BTreeSet<String>,
) -> Action {
    if reject_all || reject_ids.contains(&diff.id) {
        return Action::Reject;
    }
    if approve_ids.contains(&diff.id) {
        return Action::Apply;
    }
    if approve_all && !matches!(diff.severity, Severity::Hard) {
        return Action::Apply;
    }
    Action::Keep
}

fn ids_from(v: Option<&str>) -> BTreeSet<String> {
    v.filter(|s| *s != "all")
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default()
}

fn review_log_line(diff: &ProposedDiff, action: &str) -> String {
    serde_json::json!({
        "ts": kgx_core::util::now_iso(),
        "id": diff.id,
        "pass": diff.pass,
        "action": action,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::diff::DiffKind;

    fn diff(id: &str, sev: Severity) -> ProposedDiff {
        ProposedDiff {
            id: id.into(),
            pass: "dedup".into(),
            kind: DiffKind::Merge,
            severity: sev,
            rationale: "t".into(),
            files: vec![],
        }
    }

    #[test]
    fn reject_ids_win_over_approve_all() {
        let d = diff("A", Severity::Soft);
        let rej: BTreeSet<String> = ["A".to_string()].into();
        assert!(matches!(
            resolve_action(&d, true, &BTreeSet::new(), false, &rej),
            Action::Reject
        ));
    }

    #[test]
    fn approve_all_skips_hard_but_explicit_id_applies() {
        let d = diff("H", Severity::Hard);
        assert!(matches!(
            resolve_action(&d, true, &BTreeSet::new(), false, &BTreeSet::new()),
            Action::Keep
        ));
        let ids: BTreeSet<String> = ["H".to_string()].into();
        assert!(matches!(
            resolve_action(&d, false, &ids, false, &BTreeSet::new()),
            Action::Apply
        ));
    }

    #[test]
    fn reject_all_rejects_everything_including_hard() {
        let d = diff("H", Severity::Hard);
        assert!(matches!(
            resolve_action(&d, false, &BTreeSet::new(), true, &BTreeSet::new()),
            Action::Reject
        ));
    }

    #[test]
    fn untouched_diffs_are_kept() {
        let d = diff("X", Severity::Soft);
        assert!(matches!(
            resolve_action(&d, false, &BTreeSet::new(), false, &BTreeSet::new()),
            Action::Keep
        ));
    }
}

pub fn run(
    json: bool,
    approve: Option<String>,
    reject: Option<String>,
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
    let approve_ids = ids_from(approve.as_deref());
    let reject_all = reject.as_deref() == Some("all");
    let reject_ids = ids_from(reject.as_deref());

    let mut applied = 0u32;
    let mut rejected = 0u32;
    let mut blocked_hard = 0u32;
    let mut audit_flags = Vec::new();
    let mut remaining: Vec<ProposedDiff> = Vec::new();
    let mut log_lines: Vec<String> = Vec::new();

    for diff in staged {
        match resolve_action(&diff, approve_all, &approve_ids, reject_all, &reject_ids) {
            Action::Reject => {
                rejected += 1;
                log_lines.push(review_log_line(&diff, "reject"));
            }
            Action::Apply => {
                if ponytail_audit {
                    for flag in kgx_ponytail::audit_diff(&diff) {
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
                    }
                }
                applied += 1;
                log_lines.push(review_log_line(&diff, "apply"));
            }
            Action::Keep => {
                if matches!(diff.severity, Severity::Hard) && approve_all {
                    blocked_hard += 1;
                }
                remaining.push(diff);
            }
        }
    }

    // Resolved diffs leave the staged file; unresolved stay.
    let remaining_count = remaining.len();
    std::fs::create_dir_all(root.join(".kg"))?;
    std::fs::write(&staged_path, serde_json::to_string_pretty(&remaining)?)?;
    if !log_lines.is_empty() {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(".kg/review-log.jsonl"))?;
        for line in &log_lines {
            writeln!(f, "{line}")?;
        }
    }

    emit(
        "review",
        serde_json::json!({
            "applied": applied,
            "rejected": rejected,
            "blocked_hard": blocked_hard,
            "remaining": remaining_count,
            "audit_flags": audit_flags,
        }),
        json,
        start,
        |_| {
            println!(
                "applied {applied}; rejected {rejected}; {blocked_hard} hard blocked; {remaining_count} left staged"
            )
        },
    );
    Ok(())
}
