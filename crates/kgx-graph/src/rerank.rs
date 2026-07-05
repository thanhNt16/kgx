use kgx_core::{llm::Reranker, Result};

/// Deterministic reranker for tests: score = number of query tokens
/// (lowercased, len > 1) that appear as substrings of the doc text.
pub struct MockReranker;

impl Reranker for MockReranker {
    fn rerank(&self, query: &str, docs: &[(String, String)]) -> Result<Vec<f32>> {
        let tokens: Vec<String> = query
            .split_whitespace()
            .filter(|t| t.len() > 1)
            .map(|t| t.to_lowercase())
            .collect();
        Ok(docs
            .iter()
            .map(|(_, text)| {
                let hay = text.to_lowercase();
                tokens.iter().filter(|t| hay.contains(t.as_str())).count() as f32
            })
            .collect())
    }

    fn model_name(&self) -> String {
        "mock".into()
    }
}

/// Local ONNX cross-encoder via fastembed. Downloads once, cached in
/// the fastembed cache dir, then fully offline.
#[cfg(feature = "semantic")]
pub struct FastEmbedReranker {
    model: fastembed::TextRerank,
    name: String,
}

#[cfg(feature = "semantic")]
impl FastEmbedReranker {
    pub fn load(model: &str) -> Result<Self> {
        let (m, name) = match model {
            "bge-base" => (fastembed::RerankerModel::BGERerankerBase, "bge-base"),
            _ => (
                fastembed::RerankerModel::JINARerankerV1TurboEn,
                "jina-turbo",
            ),
        };
        let model = fastembed::TextRerank::try_new(
            fastembed::RerankInitOptions::new(m).with_show_download_progress(false),
        )
        .map_err(|e| kgx_core::KgError::Other(format!("failed to load reranker: {e}")))?;
        Ok(Self {
            model,
            name: name.into(),
        })
    }
}

#[cfg(feature = "semantic")]
impl Reranker for FastEmbedReranker {
    fn rerank(&self, query: &str, docs: &[(String, String)]) -> Result<Vec<f32>> {
        let texts: Vec<&str> = docs.iter().map(|(_, t)| t.as_str()).collect();
        let results = self
            .model
            .rerank(query, texts, false, None)
            .map_err(|e| kgx_core::KgError::Other(format!("rerank error: {e}")))?;
        // fastembed returns results sorted by score; map back to input order.
        let mut scores = vec![0.0f32; docs.len()];
        for r in results {
            if r.index < scores.len() {
                scores[r.index] = r.score;
            }
        }
        Ok(scores)
    }

    fn model_name(&self) -> String {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::llm::Reranker;

    #[test]
    fn mock_scores_by_query_token_overlap() {
        let r = MockReranker;
        let docs = vec![
            (
                "a".to_string(),
                "flink checkpoint interval tuning".to_string(),
            ),
            ("b".to_string(), "s3 lifecycle policy".to_string()),
        ];
        let scores = r.rerank("flink checkpoint", &docs).unwrap();
        assert_eq!(scores.len(), 2);
        assert!(scores[0] > scores[1], "doc a mentions both query tokens");
        assert_eq!(r.model_name(), "mock");
    }

    #[test]
    fn mock_is_deterministic() {
        let r = MockReranker;
        let docs = vec![("x".to_string(), "kafka event bus".to_string())];
        let a = r.rerank("kafka", &docs).unwrap();
        let b = r.rerank("kafka", &docs).unwrap();
        assert_eq!(a, b);
    }
}
