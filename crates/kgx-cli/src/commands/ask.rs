// crates/kgx-cli/src/commands/ask.rs
use std::time::Instant;

use crate::output::emit;
use kgx_core::llm::LlmRequest;
use kgx_graph::Brain;
use kgx_retrieval::{search, Mode, SearchOpts};
use kgx_vault::scan::scan_vault;

pub fn run(
    json: bool,
    question: &str,
    scope: &str,
    mode: &str,
    _cite: bool,
    _write: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let brain_path = root.join(".kg/brain.sqlite");
    if !brain_path.exists() {
        anyhow::bail!("brain not built — run `kg index --full` first");
    }
    let brain = Brain::open(&brain_path)?;
    let notes = scan_vault(&root)?;
    let embedder = kgx_llm::select::embedder_from_env();
    let mut ctx = String::from("ANSWER_QUESTION\nContext:\n");
    if scope == "global" {
        ctx.push_str(&kgx_retrieval::global::global_context(
            &brain,
            question,
            embedder.as_ref(),
            5,
        )?);
        ctx.push('\n');
    } else {
        let m = match mode {
            "keyword" => Mode::Keyword,
            "semantic" => Mode::Semantic,
            _ => Mode::Hybrid,
        };
        let hits = search(
            &brain,
            embedder.as_ref(),
            question,
            SearchOpts {
                mode: m,
                limit: 8,
                expand_ppr: true,
            },
        )?;
        for h in &hits {
            if let Some(n) = notes.iter().find(|n| n.fm.id == h.id) {
                ctx.push_str(&format!("[{}] {}: {}\n", n.fm.id, n.fm.title, n.body));
            }
        }
    }
    ctx.push_str(&format!("\nQuestion: {question}\n"));
    let provider = kgx_llm::select::provider_from_env()?;
    let rt = tokio::runtime::Runtime::new()?;
    let resp = rt.block_on(provider.complete(LlmRequest {
        system: "Answer only from context. Cite note ids.".into(),
        prompt: ctx,
        max_tokens: 1024,
        temperature: 0.0,
    }))?;
    let parsed: serde_json::Value = serde_json::from_str(&resp.text)
        .unwrap_or(serde_json::json!({"answer": resp.text, "citations": []}));
    kgx_tokens::record::append(
        &root.join(".kg"),
        &kgx_tokens::TokenRecord {
            model: provider.model_id().into(),
            operation: "ask".into(),
            command: "ask".into(),
            input_tokens: resp.input_tokens,
            output_tokens: resp.output_tokens,
            elapsed_ms: start.elapsed().as_millis() as u64,
            correlation_id: kgx_core::util::new_ulid(),
            ts: kgx_core::util::now_iso(),
        },
    )?;
    emit("ask", parsed.clone(), json, start, |_| {
        println!("{}", parsed["answer"].as_str().unwrap_or(""));
        if let Some(c) = parsed["citations"].as_array() {
            println!("cites: {:?}", c);
        }
    });
    Ok(())
}
