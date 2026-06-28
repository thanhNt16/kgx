use crate::output::emit;
use std::{process::Command, time::Instant};

pub fn run(json: bool, action: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let mut cmd = Command::new("git");
    match action {
        "status" => {
            cmd.args(["status", "-s"]);
        }
        "push" => {
            cmd.arg("push");
        }
        "pull" => {
            cmd.arg("pull");
        }
        other => anyhow::bail!("unknown sync action: {other}"),
    }
    let out = kgx_rtk::run_with_rtk(&mut cmd)?;
    let data = serde_json::json!({
        "action": action,
        "raw_bytes": out.raw_bytes,
        "compressed_bytes": out.compressed_bytes,
        "stdout": out.stdout
    });
    emit("sync", data, json, start, |d| {
        print!("{}", d["stdout"].as_str().unwrap_or_default())
    });
    Ok(())
}
