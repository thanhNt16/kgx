use kgx_core::{Frontmatter, KgError, Note, Result};
use std::path::{Path, PathBuf};

pub fn parse_note(rel_path: &Path, raw: &str) -> Result<Note> {
    let rest = raw
        .strip_prefix("---\n")
        .ok_or_else(|| KgError::Frontmatter {
            path: rel_path.display().to_string(),
            msg: "missing opening '---'".into(),
        })?;
    let end = rest
        .find("\n---\n")
        .or_else(|| rest.strip_suffix("\n---").map(|_| rest.len() - 4))
        .ok_or_else(|| KgError::Frontmatter {
            path: rel_path.display().to_string(),
            msg: "missing closing '---'".into(),
        })?;
    let (yaml, body) = rest.split_at(end);
    let fm: Frontmatter = serde_yaml::from_str(yaml).map_err(|e| KgError::Frontmatter {
        path: rel_path.display().to_string(),
        msg: e.to_string(),
    })?;
    let body = body
        .trim_start_matches("\n---\n")
        .trim_start_matches("\n---")
        .trim_start()
        .to_string();
    Ok(Note {
        fm,
        body,
        rel_path: PathBuf::from(rel_path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn parses_frontmatter_and_body() {
        let raw = "---\ntype: fact\nid: 01J9X2ABC\ntitle: T\n---\nBody [[Postgres]] here.\n";
        let note = parse_note(Path::new("notes/facts/t.md"), raw).unwrap();
        assert_eq!(note.fm.id, "01J9X2ABC");
        assert!(note.body.contains("Body [[Postgres]]"));
        assert_eq!(note.rel_path, Path::new("notes/facts/t.md"));
    }
    #[test]
    fn missing_frontmatter_errors() {
        let err = parse_note(Path::new("x.md"), "no frontmatter").unwrap_err();
        assert!(matches!(err, kgx_core::KgError::Frontmatter { .. }));
    }
}
