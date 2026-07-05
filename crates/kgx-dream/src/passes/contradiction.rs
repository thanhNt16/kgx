use crate::context::DreamContext;
use kgx_core::{
    diff::{DiffKind, FileChange, ProposedDiff, Severity},
    llm::LlmRequest,
    util, Note, NoteType, Result, Status,
};

/// For same-subject active fact pairs, classify via LLM.
/// Emits FlagContradiction with mapped severity (hard→Hard, scope→Scope, soft→Soft).
pub async fn run(ctx: &DreamContext<'_>) -> Result<Vec<ProposedDiff>> {
    let mut diffs = Vec::new();
    let facts: Vec<&Note> = ctx
        .notes
        .iter()
        .filter(|n| matches!(n.fm.r#type, NoteType::Fact) && matches!(n.fm.status, Status::Active))
        .collect();

    if facts.is_empty() {
        return Ok(vec![]);
    }

    let bodies: Vec<String> = facts.iter().map(|n| n.body.clone()).collect();
    let embeddings = ctx.embedder.embed(&bodies)?;
    let threshold = env_f32("KGX_CONTRADICTION_COSINE", 0.80);
    let cap = env_usize("KGX_DREAM_MAX_PAIRS", 200);

    for (i, j) in candidate_pairs(&facts, &embeddings, threshold, cap) {
        let (a, b) = (facts[i], facts[j]);
        let resp = ctx
            .provider
            .complete(LlmRequest {
                system: "Reply JSON {verdict: agree|soft|scope|hard, rationale}".into(),
                prompt: format!("CONTRADICTION\nA: {}\nB: {}", a.body, b.body),
                max_tokens: 256,
                temperature: 0.0,
            })
            .await?;
        let v: serde_json::Value =
            serde_json::from_str(&resp.text).unwrap_or(serde_json::json!({"verdict": "agree"}));
        let sev = match v["verdict"].as_str() {
            Some("hard") => Severity::Hard,
            Some("scope") => Severity::Scope,
            Some("soft") => Severity::Soft,
            _ => continue,
        };
        diffs.push(ProposedDiff {
            id: util::new_ulid(),
            pass: "contradiction".into(),
            kind: DiffKind::FlagContradiction,
            severity: sev,
            rationale: v["rationale"]
                .as_str()
                .unwrap_or("conflict detected")
                .to_string(),
            files: vec![
                FileChange {
                    rel_path: a.rel_path.display().to_string(),
                    before: None,
                    after: None,
                },
                FileChange {
                    rel_path: b.rel_path.display().to_string(),
                    before: None,
                    after: None,
                },
            ],
        });
    }

    diffs.sort_by(|x, y| x.id.cmp(&y.id));
    Ok(diffs)
}

pub(crate) fn candidate_pairs(
    facts: &[&Note],
    embeddings: &[Vec<f32>],
    threshold: f32,
    cap: usize,
) -> Vec<(usize, usize)> {
    if cap == 0 {
        return vec![];
    }
    let mut pairs = Vec::new();
    'outer: for i in 0..facts.len() {
        for j in (i + 1)..facts.len() {
            let shared_tag = facts[i]
                .fm
                .tags
                .iter()
                .any(|t| facts[j].fm.tags.contains(t));
            let shared_link = facts[i]
                .fm
                .links
                .iter()
                .any(|l| facts[j].fm.links.contains(l));
            let similar = embeddings
                .get(i)
                .zip(embeddings.get(j))
                .map(|(a, b)| super::cosine(a, b) >= threshold)
                .unwrap_or(false);
            if shared_tag || shared_link || similar {
                pairs.push((i, j));
                if pairs.len() >= cap {
                    break 'outer;
                }
            }
        }
    }
    pairs
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::Note;

    fn note(id: &str, tags: &[&str], links: &[&str]) -> Note {
        let yaml = format!("type: fact\nid: {id}\ntitle: {id}\n");
        let mut fm: kgx_core::Frontmatter = serde_yaml::from_str(&yaml).unwrap();
        fm.tags = tags.iter().map(|s| s.to_string()).collect();
        fm.links = links.iter().map(|s| format!("[[{s}]]")).collect();
        Note {
            fm,
            body: String::new(),
            rel_path: format!("notes/facts/{id}.md").into(),
        }
    }

    #[test]
    fn disjoint_tags_but_similar_embeddings_are_paired() {
        let a = note("A", &["billing"], &[]);
        let b = note("B", &["finance"], &[]);
        let facts = vec![&a, &b];
        let emb = vec![vec![1.0, 0.0], vec![1.0, 0.0]];
        let pairs = candidate_pairs(&facts, &emb, 0.80, 200);
        assert_eq!(pairs, vec![(0, 1)]);
    }

    #[test]
    fn shared_entity_link_pairs_even_with_dissimilar_embeddings() {
        let a = note("A", &[], &["alice"]);
        let b = note("B", &[], &["alice"]);
        let facts = vec![&a, &b];
        let emb = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        assert_eq!(candidate_pairs(&facts, &emb, 0.80, 200), vec![(0, 1)]);
    }

    #[test]
    fn unrelated_facts_are_not_paired_and_cap_is_respected() {
        let a = note("A", &["x"], &["p"]);
        let b = note("B", &["y"], &["q"]);
        let facts = vec![&a, &b];
        let emb = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        assert!(candidate_pairs(&facts, &emb, 0.80, 200).is_empty());

        let c = note("C", &["t"], &[]);
        let d = note("D", &["t"], &[]);
        let e = note("E", &["t"], &[]);
        let all = vec![&c, &d, &e];
        let emb3 = vec![vec![1.0, 0.0]; 3];
        assert_eq!(
            candidate_pairs(&all, &emb3, 0.80, 2).len(),
            2,
            "cap limits pairs"
        );
    }
}
