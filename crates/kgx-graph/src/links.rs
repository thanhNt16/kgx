// crates/kgx-graph/src/links.rs
use std::collections::BTreeMap;
use kgx_core::{Note, NoteType, util};

fn resolver(notes: &[Note]) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for n in notes {
        m.insert(n.fm.title.clone(), n.fm.id.clone());
        m.insert(n.fm.id.clone(), n.fm.id.clone());
    }
    m
}

pub fn backlinks(notes: &[Note]) -> BTreeMap<String, Vec<String>> {
    let res = resolver(notes);
    let mut bl: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in notes {
        let mut targets = util::extract_wikilinks(&n.body);
        for l in &n.fm.links {
            targets.extend(util::extract_wikilinks(l));
        }
        for t in targets {
            if let Some(dst) = res.get(t.trim_start_matches("raw/")) {
                if dst != &n.fm.id {
                    bl.entry(dst.clone()).or_default().push(n.fm.id.clone());
                }
            }
        }
    }
    for v in bl.values_mut() {
        v.sort();
        v.dedup();
    }
    bl
}

pub fn orphans(notes: &[Note]) -> Vec<String> {
    let bl = backlinks(notes);
    let res = resolver(notes);
    let mut out = Vec::new();
    for n in notes {
        // Skip structural/input note types — Moc and Source are roots or raw inputs,
        // not knowledge nodes expected to have bi-directional links.
        if matches!(n.fm.r#type, NoteType::Moc | NoteType::Source) {
            continue;
        }
        let has_in = bl.get(&n.fm.id).map(|v| !v.is_empty()).unwrap_or(false);
        let mut targets = util::extract_wikilinks(&n.body);
        for l in &n.fm.links {
            targets.extend(util::extract_wikilinks(l));
        }
        let has_out = targets
            .iter()
            .any(|t| res.contains_key(t.trim_start_matches("raw/")));
        if !has_in && !has_out {
            out.push(n.fm.id.clone());
        }
    }
    out.sort();
    out
}

pub fn phantoms(notes: &[Note]) -> Vec<(String, String)> {
    let res = resolver(notes);
    let mut out = Vec::new();
    for n in notes {
        for t in util::extract_wikilinks(&n.body) {
            let key = t.trim_start_matches("raw/");
            // Skip raw/ references — they are valid source pointers, not broken links.
            if !res.contains_key(key) && !t.starts_with("raw/") {
                out.push((n.fm.id.clone(), t));
            }
        }
    }
    out.sort();
    out
}
