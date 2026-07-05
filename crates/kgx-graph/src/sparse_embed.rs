use kgx_core::llm::{SparseEmbedder, SparseVec};
use kgx_core::Result;
use std::collections::BTreeMap;

/// Deterministic sparse embedder for tests: FNV-hash each token
/// (lowercased, len > 1) into a term id, weight = occurrence count.
pub struct MockSparseEmbedder;

impl SparseEmbedder for MockSparseEmbedder {
    fn embed_sparse(&self, texts: &[String]) -> Result<Vec<SparseVec>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut counts: BTreeMap<i64, f32> = BTreeMap::new();
                for word in t.split_whitespace().filter(|w| w.len() > 1) {
                    let h = word
                        .to_lowercase()
                        .bytes()
                        .fold(1469598103934665603u64, |a, b| {
                            (a ^ b as u64).wrapping_mul(1099511628211)
                        });
                    *counts.entry((h % 100_000) as i64).or_insert(0.0) += 1.0;
                }
                counts.into_iter().collect()
            })
            .collect())
    }
}

#[cfg(feature = "semantic")]
pub struct FastEmbedSparse {
    model: fastembed::SparseTextEmbedding,
}

#[cfg(feature = "semantic")]
impl FastEmbedSparse {
    pub fn load() -> Result<Self> {
        let model = fastembed::SparseTextEmbedding::try_new(
            fastembed::SparseInitOptions::new(fastembed::SparseModel::SPLADEPPV1)
                .with_show_download_progress(false),
        )
        .map_err(|e| kgx_core::KgError::Other(format!("failed to load SPLADE model: {e}")))?;
        Ok(Self { model })
    }
}

#[cfg(feature = "semantic")]
impl SparseEmbedder for FastEmbedSparse {
    fn embed_sparse(&self, texts: &[String]) -> Result<Vec<SparseVec>> {
        let out = self
            .model
            .embed(texts.to_vec(), None)
            .map_err(|e| kgx_core::KgError::Other(format!("splade error: {e}")))?;
        Ok(out
            .into_iter()
            .map(|se| {
                se.indices
                    .into_iter()
                    .zip(se.values)
                    .map(|(i, v)| (i as i64, v))
                    .collect()
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::llm::SparseEmbedder;

    #[test]
    fn mock_sparse_is_deterministic_and_counts_tokens() {
        let e = MockSparseEmbedder;
        let a = e.embed_sparse(&["kafka kafka bus".into()]).unwrap();
        let b = e.embed_sparse(&["kafka kafka bus".into()]).unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0].len(), 2);
        assert!(a[0].iter().any(|(_, w)| (*w - 2.0).abs() < 1e-6));
    }
}
