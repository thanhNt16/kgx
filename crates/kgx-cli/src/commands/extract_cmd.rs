// crates/kgx-cli/src/commands/extract_cmd.rs
use std::time::Instant;

use crate::output::emit;
use kgx_extract::{extract, Intensity};

pub fn run(
    json: bool,
    source_id: &str,
    _batch: bool,
    dry_run: bool,
    intensity: &str,
) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let notes = kgx_vault::scan::scan_vault(&root)?;
    let src = notes
        .iter()
        .find(|n| n.fm.id == source_id)
        .ok_or_else(|| anyhow::anyhow!("source {source_id} not found"))?
        .clone();
    let inten = match intensity {
        "lite" => Intensity::Lite,
        "ultra" => Intensity::Ultra,
        _ => Intensity::Full,
    };
    let provider = kgx_llm::select::provider_from_env()?;
    let rt = tokio::runtime::Runtime::new()?;
    let res = rt.block_on(extract(provider.as_ref(), &src, inten))?;
    if dry_run {
        emit(
            "extract",
            serde_json::json!({
                "dry_run": true,
                "would_create": res.notes.iter().map(|n| n.rel_path.display().to_string()).collect::<Vec<_>>()
            }),
            json,
            start,
            |_| {
                for n in &res.notes {
                    println!("+ {}", n.rel_path.display());
                }
            },
        );
        return Ok(());
    }
    for n in &res.notes {
        kgx_vault::write::write_note(&root, n)?;
    }
    kgx_tokens::record::append(
        &root.join(".kg"),
        &kgx_tokens::TokenRecord {
            model: provider.model_id().into(),
            operation: "extract".into(),
            command: "extract".into(),
            input_tokens: res.tokens.0,
            output_tokens: res.tokens.1,
            elapsed_ms: start.elapsed().as_millis() as u64,
            correlation_id: kgx_core::util::new_ulid(),
            ts: kgx_core::util::now_iso(),
        },
    )?;
    emit(
        "extract",
        serde_json::json!({"created": res.notes.len()}),
        json,
        start,
        |_| println!("extracted {} notes", res.notes.len()),
    );
    Ok(())
}
