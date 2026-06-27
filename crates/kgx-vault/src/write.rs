use std::path::Path;
use kgx_core::{Note, Result, KgError};

pub fn render_note(note: &Note) -> String {
    // Sort collections for stable diffs (Global Constraint: determinism).
    let mut fm = note.fm.clone();
    fm.tags.sort();
    fm.links.sort();
    fm.aliases.sort();
    fm.supersedes.sort();
    let yaml = serde_yaml::to_string(&fm).expect("frontmatter serialize");
    format!("---\n{yaml}---\n\n{}\n", note.body.trim_end())
}

pub fn write_note(vault_root: &Path, note: &Note) -> Result<()> {
    let full = vault_root.join(&note.rel_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KgError::Io {
            path: parent.display().to_string(),
            source: e,
        })?;
    }
    std::fs::write(&full, render_note(note)).map_err(|e| KgError::Io {
        path: full.display().to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::{Note, Frontmatter, NoteType, Status, Confidence, CreatedBy, CreatedVia};
    use std::path::PathBuf;
    fn note() -> Note {
        Note {
            rel_path: PathBuf::from("notes/facts/t.md"),
            body: "Body.".into(),
            fm: Frontmatter {
                r#type: NoteType::Fact,
                id: "01J9X2ABC".into(),
                title: "T".into(),
                status: Status::Active,
                valid_from: None,
                valid_to: None,
                recorded_at: None,
                supersedes: vec![],
                superseded_by: None,
                source: None,
                confidence: Confidence::High,
                sources_count: 0,
                tags: vec!["b".into(), "a".into()],
                links: vec![],
                entity_type: None,
                aliases: vec![],
                created_by: CreatedBy::Agent,
                created_via: CreatedVia::Cli,
                extra: Default::default(),
            },
        }
    }
    #[test]
    fn render_is_deterministic_and_sorted() {
        let r1 = render_note(&note());
        let r2 = render_note(&note());
        assert_eq!(r1, r2);
        assert!(r1.starts_with("---\n"));
        // tags serialized in sorted order for stable diffs
        // Since tags are inline, check ordering in the YAML
        let tag_a_pos = r1.find("- a\n").expect("tag a not found");
        let tag_b_pos = r1.find("- b\n").expect("tag b not found");
        assert!(tag_a_pos < tag_b_pos, "tags should be sorted: a before b");
        assert!(r1.trim_end().ends_with("Body."));
    }
}
