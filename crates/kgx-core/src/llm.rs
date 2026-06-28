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
