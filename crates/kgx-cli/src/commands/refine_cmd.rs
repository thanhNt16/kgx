use crate::output::emit;
use kgx_dream::{
    context::DreamContext,
    refine::{select_scope, RefineScope},
    run::{dream, DreamOptions},
    PassId,
};
use kgx_graph::Brain;
use std::time::Instant;

#[allow(clippy::too_many_arguments)]
pub fn run(
    json: bool,
    query: Option<String>,
    note: Option<String>,
    tag: Option<String>,
    max_iterations: u32,
    dry_run: bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;

    let notes = kgx_vault::scan::scan_vault(&root)?;
    let brain = Brain::open(&root.join(".kg/brain.sqlite"))?;
    let provider = kgx_llm::select::provider_from_env()?;
    let embedder = kgx_llm::select::embedder_from_env();

    let scope = RefineScope {
        query,
        note_id: note,
        tag,
        limit: 25,
    };
    let scoped = select_scope(&notes, &brain, &scope)?;
    let scoped_count = scoped.len();

    let ctx = DreamContext {
        notes: &scoped,
        brain: &brain,
        provider: provider.as_ref(),
        embedder: embedder.as_ref(),
    };

    let rt = tokio::runtime::Runtime::new()?;
    let result = rt.block_on(dream(
        &ctx,
        DreamOptions {
            passes: PassId::all().to_vec(),
            max_iterations,
        },
    ))?;

    if !dry_run {
        std::fs::create_dir_all(root.join(".kg"))?;
        std::fs::write(
            root.join(".kg/staged_diffs.json"),
            serde_json::to_string_pretty(&result.diffs)?,
        )?;
        crate::git::ensure_branch(&root, "kg/dream").ok();
    }

    let data = serde_json::json!({
        "scoped_notes": scoped_count,
        "staged": result.diffs.len(),
        "iterations": result.iterations,
        "dry_run": dry_run,
    });
    emit("refine", data, json, start, |d| {
        println!(
            "refined {} note(s): staged {} diff(s) — run `kg review` to apply",
            d["scoped_notes"], d["staged"]
        )
    });
    Ok(())
}
