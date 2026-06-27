use kgx_core::llm::{LlmProvider, LlmRequest};
use kgx_llm::mock::MockProvider;

#[tokio::test]
async fn mock_extract_returns_canned_facts_json() {
    let p = MockProvider::new();
    let req = LlmRequest {
        system: "extract".into(),
        prompt: "EXTRACT_FACTS\nPostgres is the primary datastore.".into(),
        max_tokens: 512,
        temperature: 0.0,
    };
    let r = p.complete(req).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&r.text).unwrap();
    assert!(v["facts"].as_array().unwrap().len() >= 1);
    assert!(r.input_tokens > 0);
}
