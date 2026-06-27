use std::path::Path;
use std::collections::BTreeMap;
use kgx_core::{Result, KgError};
use crate::record::TokenRecord;

#[derive(Debug, Clone, Copy)]
pub enum GroupBy {
    Operation,
    Command,
    Day,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenAgg {
    pub key: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub count: u32,
}

pub fn summarize(kg_dir: &Path, _since_days: u32, group: GroupBy) -> Result<Vec<TokenAgg>> {
    let path = kg_dir.join("metrics.log");
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| KgError::Io { path: path.display().to_string(), source: e })?;
    let mut map: BTreeMap<String, TokenAgg> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: TokenRecord = serde_json::from_str(line).map_err(|e| KgError::Other(e.to_string()))?;
        let key = match group {
            GroupBy::Operation => r.operation.clone(),
            GroupBy::Command => r.command.clone(),
            GroupBy::Day => r.ts.chars().take(10).collect(),
        };
        let e = map.entry(key.clone()).or_insert(TokenAgg {
            key,
            input_tokens: 0,
            output_tokens: 0,
            count: 0,
        });
        e.input_tokens += r.input_tokens;
        e.output_tokens += r.output_tokens;
        e.count += 1;
    }
    Ok(map.into_values().collect())
}
