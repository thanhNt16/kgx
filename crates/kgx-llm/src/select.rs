use kgx_core::{
    llm::{Embedder, LlmProvider},
    KgError, Result,
};

use crate::{
    claude::ClaudeProvider, mock::MockProvider, ollama::OllamaProvider, openai::OpenAiProvider,
};

pub fn provider_from_env() -> Result<Box<dyn LlmProvider>> {
    match std::env::var("KGX_LLM").as_deref().unwrap_or("claude") {
        "mock" => Ok(Box::new(MockProvider::new())),
        "claude" => {
            let k = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| KgError::Llm("ANTHROPIC_API_KEY not set".into()))?;
            let m = std::env::var("KGX_MODEL").unwrap_or_else(|_| "claude-opus-4-8".into());
            Ok(Box::new(ClaudeProvider::new(k, m)))
        }
        "openai" => {
            let k = std::env::var("OPENAI_API_KEY")
                .map_err(|_| KgError::Llm("OPENAI_API_KEY not set".into()))?;
            let m = std::env::var("KGX_MODEL").unwrap_or_else(|_| "gpt-4o".into());
            Ok(Box::new(OpenAiProvider::new(k, m)))
        }
        "ollama" => {
            let base = std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".into());
            let m = std::env::var("KGX_MODEL").unwrap_or_else(|_| "llama3".into());
            Ok(Box::new(OllamaProvider::new(base, m)))
        }
        other => Err(KgError::Llm(format!("unknown KGX_LLM provider: {other}"))),
    }
}

pub fn embedder_from_env() -> Box<dyn Embedder> {
    #[cfg(feature = "candle")]
    if std::env::var("KGX_EMBED").as_deref() == Ok("minilm") {
        match kgx_graph::embed::MiniLmEmbedder::load() {
            Ok(e) => return Box::new(e),
            Err(_) => return Box::new(kgx_graph::embed::MockEmbedder::new()),
        }
    }
    Box::new(kgx_graph::embed::MockEmbedder::new())
}
