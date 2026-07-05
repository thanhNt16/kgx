use crate::Brain;
use kgx_core::{KgError, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn sanitize_query(query: &str) -> String {
    query
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect()
}

pub fn extract_tokens(query: &str) -> Vec<String> {
    sanitize_query(query)
        .split_whitespace()
        .filter(|t| t.len() > 1)
        .map(|t| t.to_lowercase())
        .collect()
}

fn run_fts5_query(brain: &Brain, fts_query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
    let mut stmt = brain
        .conn()
        .prepare(
            "SELECT id, bm25(notes_fts) AS score FROM notes_fts \
             WHERE notes_fts MATCH ?1 ORDER BY score LIMIT ?2",
        )
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params![fts_query, limit as i64], |r| {
            let id: String = r.get(0)?;
            let score: f64 = r.get(1)?;
            Ok((id, -score as f32))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| KgError::Brain(e.to_string()))
}

pub fn bm25_search(brain: &Brain, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
    let sanitized = sanitize_query(query);
    let tokens: Vec<&str> = sanitized.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(vec![]);
    }
    // Quote each token as an FTS5 phrase ("token"). This neutralizes FTS5
    // reserved keywords (AND/OR/NOT/NEAR) and bare operators (*, ", :, etc.)
    // that appear in natural-language note bodies passed as queries by the
    // dream passes (orphan_repair, open_questions). Without quoting, a body
    // containing the word "AND" yields `tok1 OR AND OR tok2` → fts5 syntax error.
    let fts_query = tokens
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");
    run_fts5_query(brain, &fts_query, limit)
}

/// BM25 with fallback chain:
/// 1. Standard OR query (fast, best recall for well-formed queries)
/// 2. Individual term OR queries unioned in (handles FTS5 partial failures)
/// 3. LIKE substring fallback (catches anything FTS5 porter tokenizer misses)
pub fn bm25_search_loose(brain: &Brain, query: &str, limit: usize) -> Result<Vec<(String, f32)>> {
    let tokens = extract_tokens(query);
    if tokens.is_empty() {
        return Ok(vec![]);
    }

    // Step 1: Standard BM25 OR
    let mut results = bm25_search(brain, query, limit)?;
    if results.len() >= 3.min(limit) {
        return Ok(results);
    }

    let mut seen: HashSet<String> = results.iter().map(|(id, _)| id.clone()).collect();
    let mut min_score = results.last().map(|(_, s)| *s).unwrap_or(0.0);

    // Step 2: Individual term FTS5
    for token in &tokens {
        if let Ok(term_results) = bm25_search(brain, token, limit) {
            for (id, score) in term_results {
                if seen.insert(id.clone()) {
                    min_score = min_score.min(score);
                    results.push((id, score));
                }
            }
        }
    }

    if results.len() >= 3.min(limit) {
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        return Ok(results);
    }

    // Step 3: LIKE substring fallback
    let like_results = like_search(brain, &tokens, limit)?;
    for (id, score) in like_results {
        if seen.insert(id.clone()) {
            results.push((id, min_score - 1.0 + score * 0.1));
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    Ok(results)
}

/// LIKE-based substring search as ultimate fallback.
/// Reads all notes + tags, scores by count of matching tokens.
pub fn like_search(brain: &Brain, tokens: &[String], limit: usize) -> Result<Vec<(String, f32)>> {
    if tokens.is_empty() {
        return Ok(vec![]);
    }
    let mut stmt = brain
        .conn()
        .prepare("SELECT id, raw_text, COALESCE(tags, '') FROM notes")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let text: String = r.get(1)?;
            let tags: String = r.get(2)?;
            Ok((id, text, tags))
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let mut scored: Vec<(String, f32)> = Vec::new();
    for row in rows {
        let (id, text, tags) = row.map_err(|e| KgError::Brain(e.to_string()))?;
        let combined = text.to_lowercase() + " " + &tags.to_lowercase();
        let count = tokens
            .iter()
            .filter(|t| combined.contains(t.as_str()))
            .count();
        if count > 0 {
            scored.push((id, count as f32));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    Ok(scored)
}

pub fn entity_ids(brain: &Brain) -> Result<std::collections::HashSet<String>> {
    let mut stmt = brain
        .conn()
        .prepare("SELECT id FROM notes WHERE type = 'entity'")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(|e| KgError::Brain(e.to_string()))
}

/// Parse JSON tags column into a Vec of tag strings.
/// Tags are stored as JSON arrays: ["tag1","tag2",...]
pub fn parse_tags(tags_json: &str) -> HashSet<String> {
    if let Ok(Value::Array(arr)) = serde_json::from_str(tags_json) {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
            .collect()
    } else {
        HashSet::new()
    }
}

/// Tag-based expansion: given a set of source note IDs, find tags shared by
/// multiple source notes, score each tag by its frequency among source notes,
/// then retrieve other notes sharing those tags with a weighted sum.
///
/// Scoring: each tag contributes `count/n` (fraction of source notes with this
/// tag). A note's total score is the sum of weights for all tags it matches.
/// This naturally dilutes rare tags and amplifies common clusters.
///
/// Only tags appearing in 2+ source notes are expanded, to avoid individual
/// or overly narrow tags.
pub fn tag_expansion(
    brain: &Brain,
    source_ids: &[String],
    limit: usize,
) -> Result<Vec<(String, f32)>> {
    if source_ids.is_empty() {
        return Ok(vec![]);
    }

    let n = source_ids.len() as f32;

    // Collect tags from source notes, count frequency
    let mut tag_count: HashMap<String, usize> = HashMap::new();

    for id in source_ids {
        let mut stmt = brain
            .conn()
            .prepare("SELECT tags FROM notes WHERE id=?1")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let tags_json: Option<String> = stmt
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(|e| KgError::Brain(e.to_string()))?
            .next()
            .transpose()
            .map_err(|e| KgError::Brain(e.to_string()))?;

        if let Some(json) = tags_json {
            for t in parse_tags(&json) {
                *tag_count.entry(t).or_insert(0) += 1;
            }
        }
    }

    // Only expand tags shared by 2+ source notes
    let shared_tags: Vec<&String> = tag_count
        .iter()
        .filter(|(_, count)| **count >= 2)
        .map(|(tag, _)| tag)
        .collect();

    if shared_tags.is_empty() {
        return Ok(vec![]);
    }

    // Pre-compute weight for each shared tag: count / n
    let mut tag_weight: HashMap<&String, f32> = HashMap::new();
    for tag in &shared_tags {
        let count = tag_count.get(*tag).copied().unwrap_or(2);
        tag_weight.insert(tag, count as f32 / n);
    }

    // Find notes sharing any of these tags and compute weighted score.
    let mut expanded: HashMap<String, f32> = HashMap::new();

    for tag in &shared_tags {
        let weight = tag_weight[tag];
        let mut stmt = brain
            .conn()
            .prepare("SELECT id FROM notes WHERE tags LIKE ?1")
            .map_err(|e| KgError::Brain(e.to_string()))?;
        let rows = stmt
            .query_map([format!("%{}%", tag)], |r| r.get::<_, String>(0))
            .map_err(|e| KgError::Brain(e.to_string()))?;
        for r in rows {
            let id = r.map_err(|e| KgError::Brain(e.to_string()))?;
            *expanded.entry(id).or_insert(0.0) += weight;
        }
    }

    let mut result: Vec<(String, f32)> = expanded.into_iter().collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result.truncate(limit);
    Ok(result)
}

pub fn neighbors(brain: &Brain, id: &str, hops: u32) -> Result<Vec<String>> {
    use std::collections::BTreeSet;
    let mut frontier: BTreeSet<String> = BTreeSet::from([id.to_string()]);
    let mut seen = frontier.clone();
    for _ in 0..hops {
        let mut next = BTreeSet::new();
        for node in &frontier {
            let mut stmt = brain
                .conn()
                .prepare(
                    "SELECT dst_id FROM edges WHERE src_id=?1 \
                     UNION SELECT src_id FROM edges WHERE dst_id=?1",
                )
                .map_err(|e| KgError::Brain(e.to_string()))?;
            let rows = stmt
                .query_map([node], |r| r.get::<_, String>(0))
                .map_err(|e| KgError::Brain(e.to_string()))?;
            for r in rows {
                let n = r.map_err(|e| KgError::Brain(e.to_string()))?;
                if seen.insert(n.clone()) {
                    next.insert(n);
                }
            }
        }
        frontier = next;
    }
    let mut out: Vec<String> = seen.into_iter().filter(|n| n != id).collect();
    out.sort();
    Ok(out)
}
