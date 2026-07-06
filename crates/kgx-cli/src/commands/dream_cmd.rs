use crate::output::emit;
use kgx_dream::{
    context::DreamContext,
    run::{dream, DreamOptions},
    PassId,
};
use kgx_graph::Brain;
use std::time::Instant;

pub fn run(
    json: bool,
    max_iterations: u32,
    passes: Option<String>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;

    let notes = kgx_vault::scan::scan_vault(&root)?;
    let brain_path = root.join(".kg/brain.sqlite");
    let brain = Brain::open(&brain_path)?;
    let provider = kgx_llm::select::provider_from_env()?;
    let embedder = kgx_llm::select::embedder_from_env();

    let selected_passes: Vec<PassId> = match passes {
        Some(s) => s
            .split(',')
            .map(|p| p.trim())
            .filter_map(PassId::parse)
            .collect(),
        None => PassId::all().to_vec(),
    };

    let ctx = DreamContext {
        notes: &notes,
        brain: &brain,
        provider: provider.as_ref(),
        embedder: embedder.as_ref(),
    };

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(dream(
        &ctx,
        DreamOptions {
            passes: selected_passes,
            max_iterations,
        },
    ))?;

    let hard_blocks = result
        .diffs
        .iter()
        .filter(|d| matches!(d.severity, kgx_core::diff::Severity::Hard))
        .count();

    if !dry_run {
        // Ensure .kg directory exists
        std::fs::create_dir_all(root.join(".kg"))?;
        // Stage diffs to .kg/staged_diffs.json
        std::fs::write(
            root.join(".kg/staged_diffs.json"),
            serde_json::to_string_pretty(&result.diffs)?,
        )?;
        // Create/checkout git branch (best-effort)
        crate::git::ensure_branch(&root, "kg/dream").ok();
    }

    let data = serde_json::json!({
        "staged": result.diffs.len(),
        "iterations": result.iterations,
        "done_signal": result.done_signal,
        "hard_blocks": hard_blocks,
        "dry_run": dry_run,
    });

    emit("dream", data, json, start, |d| {
        println!(
            "staged {} diffs over {} iter(s) ({} hard blocks) — run `kg review` to apply",
            d["staged"], d["iterations"], d["hard_blocks"]
        );
    });

    Ok(())
}
