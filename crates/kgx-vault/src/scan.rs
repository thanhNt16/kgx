use std::path::Path;
use kgx_core::{Note, Result, KgError};
use crate::parse::parse_note;

pub fn scan_vault(vault_root: &Path) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    for sub in ["notes", "raw"] {
        let base = vault_root.join(sub);
        if !base.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_file() || p.extension().map(|e| e != "md").unwrap_or(true) {
                continue;
            }
            let raw = std::fs::read_to_string(p).map_err(|e| KgError::Io {
                path: p.display().to_string(),
                source: e,
            })?;
            let rel = p.strip_prefix(vault_root).unwrap_or(p);
            notes.push(parse_note(rel, &raw)?);
        }
    }
    notes.sort_by(|a, b| a.fm.id.cmp(&b.fm.id));
    Ok(notes)
}
