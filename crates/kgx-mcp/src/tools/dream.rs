// dream_step — surface LLM-free dream candidates (orphans, stale notes, open
// questions) for the agent harness to act on. Does NOT call an external LLM;
// the judgment passes (dedup, contradiction, supersession, community
// re-summarization) are the harness's job.
use kgx_core::{KgError, Result};
use serde_json::{json, Value};
use std::path::Path;

/// Dream passes that require no LLM — safe to run from the MCP tool.
const NON_LLM_PASSES: &[&str] = &["orphan_repair", "staleness", "open_questions"];

pub async fn run(root: &Path, args: &Value) -> Result<Value> {
    let notes = kgx_vault::scan::scan_vault(root)?;
    let brain = kgx_graph::Brain::open(&root.join(".kg/brain.sqlite"))
        .map_err(|e| KgError::Brain(e.to_string()))?;
    // MockProvider satisfies DreamContext's provider bound but is never invoked
    // by the non-LLM passes below.
    let provider = kgx_llm::MockProvider::new();
    let embedder = kgx_llm::select::embedder_from_env();

    // Restrict to LLM-free passes regardless of what the caller asked for.
    let requested: Vec<kgx_dream::PassId> = args["only"]
        .as_str()
        .map(|only| {
            only.split(',')
                .map(str::trim)
                .filter_map(kgx_dream::PassId::parse)
                .collect()
        })
        .unwrap_or_else(|| {
            NON_LLM_PASSES
                .iter()
                .filter_map(|s| kgx_dream::PassId::parse(s))
                .collect()
        });
    let passes: Vec<kgx_dream::PassId> = requested
        .into_iter()
        .filter(|p| NON_LLM_PASSES.contains(&p.name()))
        .collect::<Vec<_>>();
    if passes.is_empty() {
        return Ok(json!({
            "status": "ok",
            "note": "dream_step only runs LLM-free passes (orphan_repair, staleness, open_questions); dedup/contradiction/supersession/community are the agent harness's job"
        }));
    }

    let max_iterations = args["max_iterations"].as_u64().unwrap_or(1) as u32;
    let ctx = kgx_dream::DreamContext {
        notes: &notes,
        brain: &brain,
        provider: &provider,
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
        "passes": NON_LLM_PASSES,
        "iterations": run.iterations,
        "done_signal": run.done_signal,
        "diffs": run.diffs,
        "note": "these are LLM-free candidates (orphans/stale/open-questions). dedup, contradiction, supersession, and community reconciliation are the agent harness's job — produce those diffs and stage them via kg review."
    }))
}
