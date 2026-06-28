use kgx_core::diff::{DiffKind, ProposedDiff, Severity};

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditFlag {
    pub code: String,
    pub msg: String,
}

pub fn audit_diff(d: &ProposedDiff) -> Vec<AuditFlag> {
    // Never simplify safety-critical diffs.
    if matches!(d.severity, Severity::Hard) || matches!(d.kind, DiffKind::FlagContradiction) {
        return vec![];
    }
    let mut flags = Vec::new();
    let writes = d.files.iter().filter(|f| f.after.is_some()).count();
    if writes > 3 {
        flags.push(AuditFlag {
            code: "over_broad".into(),
            msg: format!("diff writes {writes} files; consider splitting"),
        });
    }
    if d.rationale.trim().len() < 10 {
        flags.push(AuditFlag {
            code: "weak_rationale".into(),
            msg: "rationale too thin to justify the change".into(),
        });
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::diff::{DiffKind, FileChange, ProposedDiff, Severity};

    fn diff(files: usize, sev: Severity, kind: DiffKind) -> ProposedDiff {
        ProposedDiff {
            id: "x".into(),
            pass: "dedup".into(),
            kind,
            severity: sev,
            rationale: "test rationale that is long enough".into(),
            files: (0..files)
                .map(|i| FileChange {
                    rel_path: format!("notes/f{i}.md"),
                    before: None,
                    after: Some("x".into()),
                })
                .collect(),
        }
    }

    #[test]
    fn flags_over_broad_diff() {
        let flags = audit_diff(&diff(5, Severity::Soft, DiffKind::AddLink));
        assert!(flags.iter().any(|f| f.code == "over_broad"));
    }

    #[test]
    fn never_flags_hard_contradiction() {
        let flags = audit_diff(&diff(5, Severity::Hard, DiffKind::FlagContradiction));
        assert!(
            flags.is_empty(),
            "must never simplify safety-critical diffs"
        );
    }
}
