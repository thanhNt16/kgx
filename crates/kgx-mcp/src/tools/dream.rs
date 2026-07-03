// dream_step — run one bounded dream iteration
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

pub async fn run(root: &Path, args: &Value) -> Result<Value> {
    let notes = kgx_vault::scan::scan_vault(root)?;
    let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    let provider = kgx_llm::select::provider_from_env()?;
    let embedder = kgx_llm::select::embedder_from_env();

    let passes = args["only"]
        .as_str()
        .map(|only| {
            only.split(',')
                .map(str::trim)
                .filter_map(kgx_dream::PassId::parse)
                .collect()
        })
        .unwrap_or_else(|| kgx_dream::PassId::all().to_vec());

    let max_iterations = args["max_iterations"].as_u64().unwrap_or(1) as u32;
    let ctx = kgx_dream::DreamContext {
        notes: &notes,
        brain: &brain,
        provider: provider.as_ref(),
        embedder: embedder.as_ref(),
    };
    let run = kgx_dream::run::dream(
        &ctx,
        kgx_dream::run::DreamOptions {
            passes,
            max_iterations: max_iterations.max(1),
        },
    )
    .await?;

    Ok(json!({
        "status": "ok",
        "iterations": run.iterations,
        "done_signal": run.done_signal,
        "diffs": run.diffs
    }))
}
