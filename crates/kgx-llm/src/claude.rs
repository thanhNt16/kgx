use kgx_core::{
    llm::{LlmProvider, LlmRequest, LlmResponse},
    KgError, Result,
};

pub struct ClaudeProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            api_key,
            model,
            base_url,
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
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| KgError::Llm(e.to_string()))?;
        let v: serde_json::Value = resp.json().await.map_err(|e| KgError::Llm(e.to_string()))?;
        let text = v["content"]
            .as_array()
            .and_then(|arr| arr.iter().find(|c| c["type"] == "text"))
            .and_then(|c| c["text"].as_str())
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
