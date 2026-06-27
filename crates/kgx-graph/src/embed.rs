use kgx_core::{llm::Embedder, Result};

pub fn f32_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn blob_to_f32(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub struct MockEmbedder;

impl MockEmbedder {
    pub fn new() -> Self {
        MockEmbedder
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

impl Embedder for MockEmbedder {
    fn dim(&self) -> usize {
        384
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let mut v = vec![0f32; 384];
                for word in t.split_whitespace() {
                    let h = word.bytes().fold(1469598103934665603u64, |a, b| {
                        (a ^ b as u64).wrapping_mul(1099511628211)
                    });
                    v[(h % 384) as usize] += 1.0;
                }
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
                for x in &mut v {
                    *x /= norm;
                }
                v
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kgx_core::llm::Embedder;

    #[test]
    fn mock_is_deterministic_384() {
        let e = MockEmbedder::new();
        let a = e.embed(&["hello world".into()]).unwrap();
        let b = e.embed(&["hello world".into()]).unwrap();
        assert_eq!(e.dim(), 384);
        assert_eq!(a[0].len(), 384);
        assert_eq!(a, b);
        let c = e.embed(&["different".into()]).unwrap();
        assert_ne!(a[0], c[0]);
    }

    #[test]
    fn blob_roundtrip() {
        let v = vec![1.0f32, -2.5, 3.25];
        assert_eq!(blob_to_f32(&f32_to_blob(&v)), v);
    }
}
