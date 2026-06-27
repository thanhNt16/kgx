// crates/kgx-cli/src/commands/link.rs
use std::time::Instant;

use crate::output::emit;
use kgx_graph::links::{backlinks, orphans, phantoms};
use kgx_vault::scan::scan_vault;

pub fn run(json: bool, suggest: bool, show_orphans: bool, fix: bool) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    let notes = scan_vault(&root)?;
    let bl = backlinks(&notes);
    let orph = orphans(&notes);
    let ph = phantoms(&notes);

    if show_orphans {
        emit(
            "link",
            serde_json::json!({"orphans": orph}),
            json,
            start,
            |_| {
                for id in &orph {
                    println!("orphan: {id}");
                }
            },
        );
        return Ok(());
    }

    let _ = (suggest, fix); // deferred
    emit(
        "link",
        serde_json::json!({
            "backlinks": bl.len(),
            "orphans": orph.len(),
            "phantoms": ph.len(),
        }),
        json,
        start,
        |_| {
            println!("backlinks: {}", bl.len());
            println!("orphans: {}", orph.len());
            println!("phantoms: {}", ph.len());
        },
    );
    Ok(())
}
