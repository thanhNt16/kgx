use kgx_core::{llm::{LlmProvider, LlmRequest, LlmResponse}, KgError, Result};

pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for OllamaProvider {
    fn model_id(&self) -> &str {
        &self.model
    }

    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse> {
        let body = serde_json::json!({
            "model": self.model,
            "stream": false,
            "messages": [
                {"role": "system", "content": req.system},
                {"role": "user", "content": req.prompt}
            ]
        });
        let resp = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| KgError::Llm(e.to_string()))?;
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| KgError::Llm(e.to_string()))?;
        let text = v["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        Ok(LlmResponse {
            text,
            input_tokens: v["prompt_eval_count"].as_u64().unwrap_or(0) as u32,
            output_tokens: v["eval_count"].as_u64().unwrap_or(0) as u32,
            model: self.model.clone(),
        })
    }
}
