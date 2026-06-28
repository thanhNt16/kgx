use crate::output::emit;
use std::{path::PathBuf, time::Instant};

pub fn run_usecase(json: bool, name: &str, out: PathBuf) -> anyhow::Result<()> {
    let start = Instant::now();
    let usecase = kgx_docs::usecase::parse(name)
        .ok_or_else(|| anyhow::anyhow!("unknown use case: {name}"))?;
    let html = kgx_docs::usecase::render(usecase);
    std::fs::write(&out, html)?;
    let data = serde_json::json!({ "name": name, "out": out.display().to_string() });
    emit("docs", data, json, start, |d| {
        println!("wrote {}", d["out"].as_str().unwrap_or("usecase.html"))
    });
    Ok(())
}
