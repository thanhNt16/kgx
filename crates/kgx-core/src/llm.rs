// crates/kgx-core/src/llm.rs  — the provider trait every LLM caller depends on.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub model: String,
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, req: LlmRequest) -> crate::Result<LlmResponse>;
    fn model_id(&self) -> &str;
}

/// Embedding vector provider.
pub trait Embedder: Send + Sync {
    fn embed(&self, texts: &[String]) -> crate::Result<Vec<Vec<f32>>>;
    fn dim(&self) -> usize;
    /// Returns true if this embedder produces real semantic embeddings
    /// (vs mock/deterministic). When false, vector search adds noise and should be skipped.
    fn is_semantic(&self) -> bool {
        false
    }
}

/// Sparse embedding: (term_id, weight) pairs. term_id is i64 for SQLite affinity.
pub type SparseVec = Vec<(i64, f32)>;

/// Sparse (SPLADE-style) text embedder for lexical-expansion retrieval.
pub trait SparseEmbedder: Send + Sync {
    fn embed_sparse(&self, texts: &[String]) -> crate::Result<Vec<SparseVec>>;
}

/// Cross-encoder relevance scorer: reads query and document together.
pub trait Reranker: Send + Sync {
    /// Score each (id, text) doc for relevance to `query`.
    /// Returns one score per doc, in the same order as `docs`.
    fn rerank(&self, query: &str, docs: &[(String, String)]) -> crate::Result<Vec<f32>>;
    fn model_name(&self) -> String;
}
