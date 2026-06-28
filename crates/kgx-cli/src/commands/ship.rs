use crate::output::emit;
use std::{path::PathBuf, time::Instant};

pub fn run(json: bool, out: PathBuf) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = std::env::current_dir()?;
    kgx_okf::bundle::ship(&root, &out)?;
    let data = serde_json::json!({ "out": out.display().to_string() });
    emit("ship", data, json, start, |d| {
        println!("wrote {}", d["out"].as_str().unwrap_or("bundle.okf.tar.gz"))
    });
    Ok(())
}
