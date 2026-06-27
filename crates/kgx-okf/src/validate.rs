use kgx_core::Result;
use kgx_vault::scan::scan_vault;
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub struct OkfViolation {
    pub path: String,
    pub code: String,
    pub msg: String,
}

#[derive(Debug, serde::Serialize)]
pub struct OkfReport {
    pub ok: bool,
    pub errors: Vec<OkfViolation>,
}

pub fn check_okf(root: &Path) -> Result<OkfReport> {
    let mut errors = Vec::new();
    check_reserved(root, &mut errors);
    let notes = scan_vault(root)?;
    check_frontmatter(&notes, &mut errors);
    check_bitemporal(&notes, &mut errors);
    check_links(&notes, &mut errors);
    errors.sort_by(|a, b| (&a.path, &a.code).cmp(&(&b.path, &b.code)));
    Ok(OkfReport {
        ok: errors.is_empty(),
        errors,
    })
}

fn check_reserved(root: &Path, errors: &mut Vec<OkfViolation>) {
    for f in ["index.md", "log.md"] {
        if !root.join(f).exists() {
            errors.push(OkfViolation {
                path: f.into(),
                code: "missing_reserved".into(),
                msg: format!("OKF reserved file '{f}' missing"),
            });
        }
    }
}

pub fn check_frontmatter(notes: &[kgx_core::Note], errors: &mut Vec<OkfViolation>) {
    for n in notes {
        if n.fm.id.trim().is_empty() {
            errors.push(OkfViolation {
                path: n.rel_path.display().to_string(),
                code: "missing_id".into(),
                msg: "frontmatter 'id' required".into(),
            });
        }
        if n.fm.title.trim().is_empty() {
            errors.push(OkfViolation {
                path: n.rel_path.display().to_string(),
                code: "missing_title".into(),
                msg: "frontmatter 'title' required".into(),
            });
        }
    }
}

pub fn check_bitemporal(notes: &[kgx_core::Note], errors: &mut Vec<OkfViolation>) {
    for n in notes {
        if let (Some(from), Some(to)) = (&n.fm.valid_from, &n.fm.valid_to) {
            if to.as_str() < from.as_str() {
                errors.push(OkfViolation {
                    path: n.rel_path.display().to_string(),
                    code: "bitemporal_order".into(),
                    msg: format!("valid_to {to} precedes valid_from {from}"),
                });
            }
        }
    }
}

pub fn check_links(notes: &[kgx_core::Note], errors: &mut Vec<OkfViolation>) {
    use std::collections::BTreeSet;
    let titles: BTreeSet<&str> = notes.iter().map(|n| n.fm.title.as_str()).collect();
    let ids: BTreeSet<&str> = notes.iter().map(|n| n.fm.id.as_str()).collect();
    for n in notes {
        for link in kgx_core::util::extract_wikilinks(&n.body) {
            // raw/ prefixed links are provenance pointers — lenient
            if link.starts_with("raw/") {
                continue;
            }
            let target = link.as_str();
            if !titles.contains(target) && !ids.contains(target) {
                errors.push(OkfViolation {
                    path: n.rel_path.display().to_string(),
                    code: "phantom_link".into(),
                    msg: format!("wikilink [[{link}]] resolves to nothing"),
                });
            }
        }
    }
}
