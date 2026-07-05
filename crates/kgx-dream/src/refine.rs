use kgx_core::{Note, Result};
use kgx_graph::Brain;

pub struct RefineScope {
    pub query: Option<String>,
    pub note_id: Option<String>,
    pub tag: Option<String>,
    pub limit: usize,
}

pub fn select_scope(notes: &[Note], brain: &Brain, scope: &RefineScope) -> Result<Vec<Note>> {
    use std::collections::BTreeSet;

    let mut seed_ids: BTreeSet<String> = BTreeSet::new();

    if let Some(id) = &scope.note_id {
        seed_ids.insert(id.clone());
    }
    if let Some(tag) = &scope.tag {
        for n in notes.iter().filter(|n| n.fm.tags.iter().any(|t| t == tag)) {
            seed_ids.insert(n.fm.id.clone());
        }
    }
    if let Some(q) = &scope.query {
        for (id, _score) in kgx_graph::query::bm25_search(brain, q, scope.limit)? {
            seed_ids.insert(id);
        }
    }
    if scope.note_id.is_none() && scope.tag.is_none() && scope.query.is_none() {
        return Err(kgx_core::KgError::Other(
            "kg refine needs a scope: a query, --note <id>, or --tag <tag>".into(),
        ));
    }
    if seed_ids.is_empty() {
        return Err(kgx_core::KgError::Other(
            "refine scope matched no notes — nothing to refine".into(),
        ));
    }

    let mut selected = seed_ids.clone();
    for id in &seed_ids {
        if let Ok(neigh) = kgx_graph::query::neighbors(brain, id, 1) {
            selected.extend(neigh);
        }
    }

    Ok(notes
        .iter()
        .filter(|n| selected.contains(&n.fm.id))
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str, tag: Option<&str>) -> Note {
        let yaml = format!("type: fact\nid: {id}\ntitle: {id}\n");
        let mut fm: kgx_core::Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        if let Some(t) = tag {
            fm.tags = vec![t.into()];
        }
        Note {
            fm,
            body: String::new(),
            rel_path: format!("notes/facts/{id}.md").into(),
        }
    }

    #[test]
    fn tag_scope_selects_tagged_notes_without_brain_queries() {
        let notes = vec![
            note("A", Some("billing")),
            note("B", None),
            note("C", Some("billing")),
        ];
        let tmp = tempfile::tempdir().unwrap();
        let brain = Brain::open(&tmp.path().join("brain.sqlite")).unwrap();
        let scope = RefineScope {
            query: None,
            note_id: None,
            tag: Some("billing".into()),
            limit: 50,
        };
        let picked = select_scope(&notes, &brain, &scope).unwrap();
        let ids: Vec<&str> = picked.iter().map(|n| n.fm.id.as_str()).collect();
        assert!(ids.contains(&"A") && ids.contains(&"C") && !ids.contains(&"B"));
    }

    #[test]
    fn note_scope_selects_exact_id() {
        let notes = vec![note("A", None), note("B", None)];
        let tmp = tempfile::tempdir().unwrap();
        let brain = Brain::open(&tmp.path().join("brain.sqlite")).unwrap();
        let scope = RefineScope {
            query: None,
            note_id: Some("B".into()),
            tag: None,
            limit: 50,
        };
        let picked = select_scope(&notes, &brain, &scope).unwrap();
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].fm.id, "B");
    }

    #[test]
    fn empty_scope_errors() {
        let notes = vec![note("A", None)];
        let tmp = tempfile::tempdir().unwrap();
        let brain = Brain::open(&tmp.path().join("brain.sqlite")).unwrap();
        let scope = RefineScope {
            query: None,
            note_id: None,
            tag: None,
            limit: 50,
        };
        assert!(select_scope(&notes, &brain, &scope).is_err());
    }
}
