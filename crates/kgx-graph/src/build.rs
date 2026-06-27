use crate::{embed::f32_to_blob, Brain};
use kgx_core::llm::Embedder;
use kgx_core::{util, Edge, KgError, Note, RelType, Result};
use rusqlite::params;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct BuildStats {
    pub nodes: usize,
    pub edges: usize,
    pub embedded: usize,
}

pub fn derive_edges(notes: &[Note]) -> Vec<Edge> {
    use std::collections::BTreeMap;
    let by_title: BTreeMap<&str, &str> = notes
        .iter()
        .map(|n| (n.fm.title.as_str(), n.fm.id.as_str()))
        .collect();
    let by_id: std::collections::BTreeSet<&str> = notes.iter().map(|n| n.fm.id.as_str()).collect();
    // Index by path-without-extension (e.g. "raw/2026-01-15-arch-review")
    let by_path: BTreeMap<String, &str> = notes
        .iter()
        .map(|n| {
            let s = n.rel_path.to_string_lossy();
            let stem = s.trim_end_matches(".md").to_string();
            (stem, n.fm.id.as_str())
        })
        .collect();
    let resolve = |target: &str| -> Option<String> {
        // Try exact title match first
        if let Some(id) = by_title.get(target) {
            return Some(id.to_string());
        }
        // Try exact ID match
        if by_id.contains(target) {
            return Some(target.to_string());
        }
        // Try path stem match (handles "raw/2026-01-15-arch-review" style links)
        if let Some(id) = by_path.get(target) {
            return Some(id.to_string());
        }
        // Try title match after stripping "raw/" prefix
        let t = target.trim_start_matches("raw/");
        if let Some(id) = by_title.get(t) {
            return Some(id.to_string());
        }
        None
    };
    let mut edges = Vec::new();
    for n in notes {
        let mut targets = util::extract_wikilinks(&n.body);
        for l in &n.fm.links {
            targets.extend(util::extract_wikilinks(l));
        }
        for t in targets {
            if let Some(dst) = resolve(&t) {
                if dst != n.fm.id {
                    edges.push(Edge {
                        src_id: n.fm.id.clone(),
                        dst_id: dst,
                        rel_type: RelType::LinksTo,
                        valid_from: n.fm.valid_from.clone(),
                        valid_to: n.fm.valid_to.clone(),
                    });
                }
            }
        }
        for s in &n.fm.supersedes {
            if let Some(dst) = resolve(s) {
                edges.push(Edge {
                    src_id: n.fm.id.clone(),
                    dst_id: dst,
                    rel_type: RelType::Supersedes,
                    valid_from: n.fm.valid_from.clone(),
                    valid_to: n.fm.valid_to.clone(),
                });
            }
        }
        if let Some(src) = &n.fm.source {
            for t in util::extract_wikilinks(src) {
                if let Some(dst) = resolve(&t) {
                    edges.push(Edge {
                        src_id: n.fm.id.clone(),
                        dst_id: dst,
                        rel_type: RelType::DerivedFrom,
                        valid_from: None,
                        valid_to: None,
                    });
                }
            }
        }
    }
    edges.sort_by(|a, b| {
        (
            a.src_id.clone(),
            a.dst_id.clone(),
            format!("{:?}", a.rel_type),
        )
            .cmp(&(
                b.src_id.clone(),
                b.dst_id.clone(),
                format!("{:?}", b.rel_type),
            ))
    });
    edges.dedup();
    edges
}

pub fn build_full(
    brain: &mut Brain,
    notes: &[Note],
    embedder: &dyn Embedder,
) -> Result<BuildStats> {
    let tx = brain
        .conn_mut()
        .transaction()
        .map_err(|e| KgError::Brain(e.to_string()))?;
    tx.execute_batch("DELETE FROM notes; DELETE FROM edges; DELETE FROM notes_fts;")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let texts: Vec<String> = notes
        .iter()
        .map(|n| format!("{}\n{}", n.fm.title, n.body))
        .collect();
    let embeddings = embedder.embed(&texts)?;
    for (n, emb) in notes.iter().zip(&embeddings) {
        let tags = serde_json::to_string(&{
            let mut t = n.fm.tags.clone();
            t.sort();
            t
        })
        .unwrap();
        let typ = serde_json::to_string(&n.fm.r#type)
            .unwrap()
            .trim_matches('"')
            .to_string();
        let st = serde_json::to_string(&n.fm.status)
            .unwrap()
            .trim_matches('"')
            .to_string();
        tx.execute(
            "INSERT INTO notes (id,path,type,status,valid_from,valid_to,recorded_at,tags,raw_text,embedding)\
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                n.fm.id,
                n.rel_path.display().to_string(),
                typ,
                st,
                n.fm.valid_from,
                n.fm.valid_to,
                n.fm.recorded_at,
                tags,
                n.body,
                f32_to_blob(emb)
            ],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute(
            "INSERT INTO notes_fts (id, raw_text, tags) VALUES (?1,?2,?3)",
            params![n.fm.id, n.body, tags],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    let edges = derive_edges(notes);
    for e in &edges {
        let rt = serde_json::to_string(&e.rel_type)
            .unwrap()
            .trim_matches('"')
            .to_string();
        tx.execute(
            "INSERT OR IGNORE INTO edges (src_id,dst_id,rel_type,valid_from,valid_to) VALUES (?1,?2,?3,?4,?5)",
            params![e.src_id, e.dst_id, rt, e.valid_from, e.valid_to],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
    Ok(BuildStats {
        nodes: notes.len(),
        edges: edges.len(),
        embedded: embeddings.len(),
    })
}

pub fn build_incremental(
    brain: &mut Brain,
    notes: &[Note],
    changed_ids: &[String],
    embedder: &dyn Embedder,
) -> Result<BuildStats> {
    use std::collections::BTreeSet;
    let changed: BTreeSet<&str> = changed_ids.iter().map(|s| s.as_str()).collect();
    if changed.is_empty() {
        return Ok(BuildStats {
            nodes: 0,
            edges: 0,
            embedded: 0,
        });
    }
    let subset: Vec<&Note> = notes
        .iter()
        .filter(|n| changed.contains(n.fm.id.as_str()))
        .collect();
    let texts: Vec<String> = subset
        .iter()
        .map(|n| format!("{}\n{}", n.fm.title, n.body))
        .collect();
    let embs = embedder.embed(&texts)?;
    let tx = brain
        .conn_mut()
        .transaction()
        .map_err(|e| KgError::Brain(e.to_string()))?;
    for (n, emb) in subset.iter().zip(&embs) {
        let tags = serde_json::to_string(&{
            let mut t = n.fm.tags.clone();
            t.sort();
            t
        })
        .unwrap();
        let typ = serde_json::to_string(&n.fm.r#type)
            .unwrap()
            .trim_matches('"')
            .to_string();
        let st = serde_json::to_string(&n.fm.status)
            .unwrap()
            .trim_matches('"')
            .to_string();
        tx.execute(
            "INSERT INTO notes (id,path,type,status,valid_from,valid_to,recorded_at,tags,raw_text,embedding)\
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)\
             ON CONFLICT(id) DO UPDATE SET path=?2,type=?3,status=?4,valid_from=?5,valid_to=?6,recorded_at=?7,tags=?8,raw_text=?9,embedding=?10",
            params![
                n.fm.id,
                n.rel_path.display().to_string(),
                typ,
                st,
                n.fm.valid_from,
                n.fm.valid_to,
                n.fm.recorded_at,
                tags,
                n.body,
                f32_to_blob(emb)
            ],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute("DELETE FROM notes_fts WHERE id=?1", params![n.fm.id])
            .map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute(
            "INSERT INTO notes_fts (id, raw_text, tags) VALUES (?1,?2,?3)",
            params![n.fm.id, n.body, tags],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
        tx.execute("DELETE FROM edges WHERE src_id=?1", params![n.fm.id])
            .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    let all_edges = derive_edges(notes);
    for e in all_edges
        .iter()
        .filter(|e| changed.contains(e.src_id.as_str()))
    {
        let rt = serde_json::to_string(&e.rel_type)
            .unwrap()
            .trim_matches('"')
            .to_string();
        tx.execute(
            "INSERT OR IGNORE INTO edges (src_id,dst_id,rel_type,valid_from,valid_to) VALUES (?1,?2,?3,?4,?5)",
            params![e.src_id, e.dst_id, rt, e.valid_from, e.valid_to],
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    }
    tx.commit().map_err(|e| KgError::Brain(e.to_string()))?;
    let edge_count = all_edges
        .iter()
        .filter(|e| changed.contains(e.src_id.as_str()))
        .count();
    Ok(BuildStats {
        nodes: subset.len(),
        edges: edge_count,
        embedded: embs.len(),
    })
}
