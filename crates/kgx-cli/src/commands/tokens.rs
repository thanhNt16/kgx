use crate::output::emit;
use kgx_tokens::aggregate::{summarize, GroupBy};
use std::time::Instant;

pub fn run(json: bool, since: &str, by: &str) -> anyhow::Result<()> {
    let start = Instant::now();
    let root = crate::vault::vault_root()?;
    let since_days = parse_since(since)?;
    let group = match by {
        "operation" => GroupBy::Operation,
        "command" => GroupBy::Command,
        "day" => GroupBy::Day,
        other => anyhow::bail!("unknown token grouping: {other}"),
    };
    let aggregates = summarize(&root.join(".kg"), since_days, group)?;
    let data = serde_json::json!({ "since_days": since_days, "by": by, "aggregates": aggregates });
    emit("tokens", data, json, start, |d| {
        println!(
            "{} token groups",
            d["aggregates"].as_array().map(|a| a.len()).unwrap_or(0)
        )
    });
    Ok(())
}

fn parse_since(s: &str) -> anyhow::Result<u32> {
    let days = s.strip_suffix('d').unwrap_or(s);
    Ok(days.parse()?)
}
