use std::path::Path;
use std::io::Write;
use kgx_core::{Result, KgError};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenRecord {
    pub model: String,
    pub operation: String,
    pub command: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub elapsed_ms: u64,
    pub correlation_id: String,
    pub ts: String,
}

pub fn append(kg_dir: &Path, r: &TokenRecord) -> Result<()> {
    std::fs::create_dir_all(kg_dir)
        .map_err(|e| KgError::Io { path: kg_dir.display().to_string(), source: e })?;
    let path = kg_dir.join("metrics.log");
    let line = serde_json::to_string(r).map_err(|e| KgError::Other(e.to_string()))?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| KgError::Io { path: path.display().to_string(), source: e })?;
    writeln!(f, "{line}").map_err(|e| KgError::Io { path: path.display().to_string(), source: e })?;
    Ok(())
}
