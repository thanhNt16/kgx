use crate::output::emit;
use std::{path::PathBuf, time::Instant};

pub fn run(json: bool, file: PathBuf, namespace: Option<String>) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;
    let imported = kgx_okf::bundle::pull(&file, &root, namespace.as_deref())?;
    let data = serde_json::json!({ "file": file.display().to_string(), "namespace": namespace, "imported": imported });
    emit("pull", data, json, start, |d| {
        println!("imported {} files", d["imported"])
    });
    Ok(())
}
