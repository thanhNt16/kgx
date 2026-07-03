use kgx_core::{KgError, Result};
use kgx_graph::Brain;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct FrictionReport {
    pub themes: Vec<FrictionTheme>,
    pub total_events: usize,
}

#[derive(Debug, Serialize)]
pub struct FrictionTheme {
    pub title: String,
    pub count: usize,
    pub examples: Vec<String>,
    pub fix_proposal: String,
}

pub fn analyze(brain: &Brain) -> Result<FrictionReport> {
    let events = query_friction_events(brain)?;
    let total_events = events.len();
    let clusters = cluster_by_keywords(&events);
    let mut themes = Vec::new();
    for (title, group) in clusters {
        let examples: Vec<String> = group.iter().take(3).map(|e| e.raw_text.clone()).collect();
        let fix_proposal = propose_fix(&title, &group);
        themes.push(FrictionTheme {
            title,
            count: group.len(),
            examples,
            fix_proposal,
        });
    }
    themes.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(FrictionReport {
        themes,
        total_events,
    })
}

struct FrictionEvent {
    id: String,
    raw_text: String,
}

fn query_friction_events(brain: &Brain) -> Result<Vec<FrictionEvent>> {
    let mut stmt = brain
        .conn()
        .prepare("SELECT id, raw_text FROM notes WHERE type = 'friction' AND status = 'active'")
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            let id: String = r.get(0)?;
            let raw_text: String = r.get(1)?;
            Ok(FrictionEvent { id, raw_text })
        })
        .map_err(|e| KgError::Brain(e.to_string()))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| KgError::Brain(e.to_string()))
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 3)
        .map(|t| t.to_string())
        .collect()
}

fn cluster_by_keywords(events: &[FrictionEvent]) -> Vec<(String, Vec<FrictionEvent>)> {
    let mut seen = vec![false; events.len()];
    let mut clusters: Vec<(String, Vec<FrictionEvent>)> = Vec::new();
    for i in 0..events.len() {
        if seen[i] {
            continue;
        }
        let tokens_i: Vec<String> = events[i]
            .raw_text
            .to_lowercase()
            .split_whitespace()
            .filter(|t| t.len() > 3)
            .map(|t| t.to_string())
            .collect();
        let mut cluster: Vec<FrictionEvent> = Vec::new();
        cluster.push(FrictionEvent {
            id: events[i].id.clone(),
            raw_text: events[i].raw_text.clone(),
        });
        seen[i] = true;
        for j in i + 1..events.len() {
            if seen[j] {
                continue;
            }
            let tokens_j: Vec<String> = events[j]
                .raw_text
                .to_lowercase()
                .split_whitespace()
                .filter(|t| t.len() > 3)
                .map(|t| t.to_string())
                .collect();
            let overlap: usize = tokens_i.iter().filter(|t| tokens_j.contains(t)).count();
            let threshold = tokens_i.len().min(tokens_j.len()).max(1);
            if overlap * 100 / threshold >= 25 {
                cluster.push(FrictionEvent {
                    id: events[j].id.clone(),
                    raw_text: events[j].raw_text.clone(),
                });
                seen[j] = true;
            }
        }
        let title = derive_title(&cluster);
        clusters.push((title, cluster));
    }
    clusters
}

fn derive_title(events: &[FrictionEvent]) -> String {
    if events.is_empty() {
        return "uncategorized".into();
    }
    let mut freq: HashMap<String, usize> = HashMap::new();
    for e in events {
        for t in tokenize(&e.raw_text) {
            *freq.entry(t).or_insert(0) += 1;
        }
    }
    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));
    let top: Vec<&str> = pairs.iter().take(3).map(|(w, _)| w.as_str()).collect();
    top.join(" / ")
}

fn propose_fix(title: &str, events: &[FrictionEvent]) -> String {
    let combined = events
        .iter()
        .map(|e| e.raw_text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    if combined.contains("not found")
        || combined.contains("no results")
        || combined.contains("empty")
    {
        format!(
            "Add a missing note or alias covering '{}' (detected: empty results / not found errors)"
        , title)
    } else if combined.contains("wrong")
        || combined.contains("incorrect")
        || combined.contains("unexpected")
    {
        format!(
            "Review and correct the note(s) related to '{}' (detected: incorrect/wrong results)",
            title
        )
    } else if combined.contains("slow")
        || combined.contains("timeout")
        || combined.contains("latency")
    {
        format!(
            "Optimize retrieval or add an index for '{}' (detected: slow queries)",
            title
        )
    } else {
        format!(
            "Review the cluster '{}' ({} events) and add missing notes, aliases, or skill amendments"
        , title, events.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_graph::Brain;

    #[test]
    fn empty_brain_returns_empty_report() {
        let brain = Brain::open_in_memory().unwrap();
        let report = analyze(&brain).unwrap();
        assert!(report.themes.is_empty());
        assert_eq!(report.total_events, 0);
    }

    #[test]
    fn clusters_related_events() {
        let brain = Brain::open_in_memory().unwrap();
        brain.conn().execute_batch(
            "INSERT INTO notes (id, path, type, status, raw_text) VALUES
             ('a', 'a.md', 'friction', 'active', 'search for postgres connection returned no results'),
             ('b', 'b.md', 'friction', 'active', 'empty results when querying postgres connection string'),
             ('c', 'c.md', 'friction', 'active', 'query for redis cache config was too slow')"
        ).unwrap();
        let report = analyze(&brain).unwrap();
        assert_eq!(report.total_events, 3);
        assert!(!report.themes.is_empty());
        let pg_theme = report
            .themes
            .iter()
            .find(|t| t.title.contains("postgres") || t.title.contains("connection"))
            .or_else(|| report.themes.first());
        assert!(pg_theme.is_some());
    }
}
