use kgx_core::{llm::{LlmProvider, LlmRequest, LlmResponse}, KgError, Result};

pub struct ClaudeProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for ClaudeProvider {
    fn model_id(&self) -> &str {
        &self.model
    }

    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": req.max_tokens,
            "system": req.system,
            "messages": [{"role": "user", "content": req.prompt}]
        });
        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| KgError::Llm(e.to_string()))?;
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| KgError::Llm(e.to_string()))?;
        let text = v["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(LlmResponse {
            text,
            input_tokens: v["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: v["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            model: self.model.clone(),
        })
    }
}
