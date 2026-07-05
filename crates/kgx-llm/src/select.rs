use kgx_core::{
    llm::{Embedder, LlmProvider, Reranker},
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
                .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
                .map_err(|_| {
                    KgError::Llm("ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN not set".into())
                })?;
            let m = std::env::var("KGX_MODEL").unwrap_or_else(|_| "claude-opus-4-8".into());
            let base_url = std::env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".into());
            Ok(Box::new(ClaudeProvider::new(k, m, base_url)))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedChoice {
    Off,
    Mock,
    MiniLm,
    FastEmbed,
}

/// Pure selection logic so it is unit-testable. `var` is the value of KGX_EMBED.
/// `semantic_built`/`candle_built` reflect the `semantic`/`candle` cargo features.
pub fn embed_choice(var: Option<&str>, semantic_built: bool, candle_built: bool) -> EmbedChoice {
    match var {
        Some("off") => EmbedChoice::Off,
        Some("mock") => EmbedChoice::Mock,
        Some("minilm") if candle_built => EmbedChoice::MiniLm,
        Some("minilm") => EmbedChoice::Mock,
        Some("fastembed") | None if semantic_built => EmbedChoice::FastEmbed,
        Some(_) | None => EmbedChoice::Mock,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RerankChoice {
    Off,
    Mock,
    FastEmbed(String),
}

/// Pure selection logic. `var` = KGX_RERANK, `model_var` = KGX_RERANK_MODEL.
pub fn rerank_choice(
    var: Option<&str>,
    model_var: Option<&str>,
    semantic_built: bool,
) -> RerankChoice {
    match var {
        Some("off") => RerankChoice::Off,
        Some("mock") => RerankChoice::Mock,
        _ if semantic_built => {
            RerankChoice::FastEmbed(model_var.unwrap_or("jina-turbo").to_string())
        }
        _ => RerankChoice::Off,
    }
}

pub fn reranker_from_env() -> Option<Box<dyn Reranker>> {
    let var = std::env::var("KGX_RERANK").ok();
    let model_var = std::env::var("KGX_RERANK_MODEL").ok();
    match rerank_choice(
        var.as_deref(),
        model_var.as_deref(),
        cfg!(feature = "semantic"),
    ) {
        RerankChoice::Off => None,
        RerankChoice::Mock => Some(Box::new(kgx_graph::rerank::MockReranker)),
        #[cfg(feature = "semantic")]
        RerankChoice::FastEmbed(model) => {
            match kgx_graph::rerank::FastEmbedReranker::load(&model) {
                Ok(r) => Some(Box::new(r)),
                Err(e) => {
                    eprintln!("warning: reranker failed to load, rerank stage disabled: {e}");
                    None
                }
            }
        }
        #[cfg(not(feature = "semantic"))]
        RerankChoice::FastEmbed(_) => None,
    }
}

pub fn rerank_topk_from_env() -> usize {
    std::env::var("KGX_RERANK_TOPK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30)
}

/// Human-readable label for `kg status` / warnings.
pub fn embedder_label() -> String {
    let var = std::env::var("KGX_EMBED").ok();
    match embed_choice(
        var.as_deref(),
        cfg!(feature = "semantic"),
        cfg!(feature = "candle"),
    ) {
        EmbedChoice::FastEmbed => "fastembed (semantic)".into(),
        EmbedChoice::MiniLm => "minilm (semantic)".into(),
        EmbedChoice::Off => "off (keyword-only, explicit)".into(),
        EmbedChoice::Mock => "mock (keyword-only — semantic search DISABLED)".into(),
    }
}

pub fn embedder_from_env() -> Box<dyn Embedder> {
    let var = std::env::var("KGX_EMBED").ok();
    let choice = embed_choice(
        var.as_deref(),
        cfg!(feature = "semantic"),
        cfg!(feature = "candle"),
    );
    match choice {
        #[cfg(feature = "candle")]
        EmbedChoice::MiniLm => match kgx_graph::embed::MiniLmEmbedder::load() {
            Ok(e) => return Box::new(e),
            Err(e) => {
                eprintln!("warning: minilm failed to load, falling back to mock: {e}");
                return Box::new(kgx_graph::embed::MockEmbedder::new());
            }
        },
        #[cfg(feature = "semantic")]
        EmbedChoice::FastEmbed => match kgx_graph::embed::FastEmbedEmbedder::load() {
            Ok(e) => return Box::new(e),
            Err(e) => {
                eprintln!(
                    "warning: fastembed failed to load, falling back to mock (semantic search disabled): {e}"
                );
                return Box::new(kgx_graph::embed::MockEmbedder::new());
            }
        },
        _ => {}
    }
    if !matches!(choice, EmbedChoice::Off | EmbedChoice::Mock)
        || (var.is_none() && !cfg!(feature = "semantic"))
    {
        eprintln!("warning: using mock embedder — semantic search disabled (build with the default `semantic` feature)");
    }
    Box::new(kgx_graph::embed::MockEmbedder::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_choice_defaults_to_fastembed_when_semantic_built() {
        assert_eq!(embed_choice(None, true, false), EmbedChoice::FastEmbed);
    }

    #[test]
    fn embed_choice_falls_back_to_mock_without_semantic_build() {
        assert_eq!(embed_choice(None, false, false), EmbedChoice::Mock);
    }

    #[test]
    fn embed_choice_off_and_mock_opt_out() {
        assert_eq!(embed_choice(Some("off"), true, false), EmbedChoice::Off);
        assert_eq!(embed_choice(Some("mock"), true, false), EmbedChoice::Mock);
    }

    #[test]
    fn embed_choice_explicit_backends() {
        assert_eq!(
            embed_choice(Some("fastembed"), true, false),
            EmbedChoice::FastEmbed
        );
        assert_eq!(
            embed_choice(Some("minilm"), true, true),
            EmbedChoice::MiniLm
        );
        // requesting a backend that isn't compiled in falls back to mock
        assert_eq!(
            embed_choice(Some("fastembed"), false, false),
            EmbedChoice::Mock
        );
        assert_eq!(embed_choice(Some("minilm"), true, false), EmbedChoice::Mock);
    }

    #[test]
    fn rerank_choice_defaults_on_when_semantic_built() {
        assert_eq!(
            rerank_choice(None, None, true),
            RerankChoice::FastEmbed("jina-turbo".into())
        );
        assert_eq!(rerank_choice(None, None, false), RerankChoice::Off);
    }

    #[test]
    fn rerank_choice_off_mock_and_model_override() {
        assert_eq!(rerank_choice(Some("off"), None, true), RerankChoice::Off);
        assert_eq!(rerank_choice(Some("mock"), None, true), RerankChoice::Mock);
        assert_eq!(
            rerank_choice(None, Some("bge-base"), true),
            RerankChoice::FastEmbed("bge-base".into())
        );
    }
}
