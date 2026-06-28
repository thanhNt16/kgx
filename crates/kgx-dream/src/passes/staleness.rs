use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    util, Result, Status,
};
use kgx_vault::write::render_note;

const STALENESS_DAYS: i64 = 365;

/// Flag active notes whose `source:` wikilink points to nonexistent raw/ stem
/// AND whose `valid_from` is older than STALENESS_DAYS.
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let now_str = util::now_iso();
    let today = &now_str[..10]; // "YYYY-MM-DD"

    // Build a set of existing raw/ stems from the note list
    let raw_stems: std::collections::BTreeSet<String> = ctx
        .notes
        .iter()
        .filter(|n| n.rel_path.starts_with("raw/"))
        .map(|n| {
            n.rel_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let mut diffs = Vec::new();

    for note in ctx
        .notes
        .iter()
        .filter(|n| matches!(n.fm.status, Status::Active))
    {
        // Check if note has a source wikilink pointing to raw/
        let source_stem = match source_raw_stem(&note.fm.source) {
            Some(s) => s,
            None => continue,
        };

        // Check if the raw file exists
        if raw_stems.contains(&source_stem) {
            continue;
        }

        // Check if valid_from is older than threshold
        let valid_from = match &note.fm.valid_from {
            Some(v) => v.clone(),
            None => continue,
        };

        if !is_older_than(today, &valid_from, STALENESS_DAYS) {
            continue;
        }

        let mut archived = note.clone();
        archived.fm.status = Status::Archived;

        diffs.push(ProposedDiff {
            id: util::new_ulid(),
            pass: "staleness".into(),
            kind: DiffKind::Archive,
            severity: Severity::Soft,
            rationale: format!(
                "Source '{}' not found and valid_from {} is older than {} days",
                source_stem, valid_from, STALENESS_DAYS
            ),
            files: vec![FileChange {
                rel_path: note.rel_path.display().to_string(),
                before: Some(render_note(note)),
                after: Some(render_note(&archived)),
            }],
        });
    }

    Ok(diffs)
}

/// Extract the raw stem from a source field like `[[raw/2026-01-15-arch-review]]`.
fn source_raw_stem(source: &Option<String>) -> Option<String> {
    let s = source.as_deref()?;
    // Extract wikilink target
    let inner = s.strip_prefix("[[")?.strip_suffix("]]")?;
    let stem = inner.strip_prefix("raw/")?;
    Some(stem.to_string())
}

/// Returns true if `valid_from` date is more than `days` days before `today`.
fn is_older_than(today: &str, valid_from: &str, days: i64) -> bool {
    if let (Some(t), Some(v)) = (parse_date(today), parse_date(valid_from)) {
        (t - v) > days
    } else {
        false
    }
}

/// Parse "YYYY-MM-DD" into days since epoch (approximate).
fn parse_date(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() < 3 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let m: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    // Simple Julian Day calculation
    Some(y * 365 + m * 30 + d)
}
