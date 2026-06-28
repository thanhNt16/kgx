use kgx_core::{
    llm::{LlmProvider, LlmRequest, LlmResponse},
    Result,
};

pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        MockProvider
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn model_id(&self) -> &str {
        "mock"
    }

    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
        let input_tokens = (req.prompt.len() / 4 + req.system.len() / 4) as u32;
        let text = if req.prompt.contains("ANSWER_QUESTION") {
            serde_json::json!({
                "answer": "Based on the notes, Postgres is the primary datastore.",
                "citations": ["01FACT01POSTGRESPRIMARY00"]
            })
            .to_string()
        } else if req.prompt.contains("EXTRACT_FACTS") {
            let body = req
                .prompt
                .split_once("EXTRACT_FACTS\n")
                .map(|x| x.1)
                .unwrap_or("");
            let facts: Vec<_> = body
                .split('.')
                .filter(|s| !s.trim().is_empty())
                .map(|s| {
                    serde_json::json!({
                        "title": s.trim(),
                        "body": s.trim(),
                        "confidence": "medium",
                        "entities": s.split_whitespace()
                            .filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            let facts: Vec<_> = facts
                .into_iter()
                .filter(|f| {
                    let t = f["title"].as_str().unwrap_or("");
                    !t.is_empty() && !t.contains("EXTRACT_FACTS") && t.len() < 300
                })
                .collect();
            serde_json::json!({ "facts": facts }).to_string()
        } else if req.prompt.contains("CONTRADICTION") {
            serde_json::json!({
                "verdict": "hard",
                "rationale": "Two different primary datastores asserted."
            })
            .to_string()
        } else if req.prompt.contains("MERGE") {
            serde_json::json!({ "merge": false, "rationale": "Distinct facts." }).to_string()
        } else if req.prompt.contains("COMMUNITY_SUMMARY") {
            serde_json::json!({
                "summary": "This community covers datastore infrastructure decisions."
            })
            .to_string()
        } else {
            serde_json::json!({ "text": "ok" }).to_string()
        };
        let output_tokens = (text.len() / 4) as u32;
        Ok(LlmResponse {
            text,
            input_tokens,
            output_tokens,
            model: "mock".into(),
        })
    }
}
