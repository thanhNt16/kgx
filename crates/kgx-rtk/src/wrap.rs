use kgx_core::{KgError, Result};
use std::process::Command;

#[derive(Debug)]
pub struct RtkOutput {
    pub raw_bytes: usize,
    pub compressed_bytes: usize,
    pub stdout: String,
}

pub fn run_with_rtk(cmd: &mut Command) -> Result<RtkOutput> {
    let output = cmd
        .output()
        .map_err(|e| KgError::Other(format!("spawn failed: {e}")))?;
    let raw = String::from_utf8_lossy(&output.stdout).to_string();
    let raw_bytes = raw.len();
    let rtk_off = std::env::var("KGX_RTK").as_deref() == Ok("off")
        || std::env::var("KGX_RTK_DISABLE").is_ok();
    if rtk_off || !rtk_available() {
        return Ok(RtkOutput {
            raw_bytes,
            compressed_bytes: raw_bytes,
            stdout: raw,
        });
    }
    match pipe_through_rtk(&raw) {
        Ok(compressed) => Ok(RtkOutput {
            raw_bytes,
            compressed_bytes: compressed.len(),
            stdout: compressed,
        }),
        Err(_) => Ok(RtkOutput {
            raw_bytes,
            compressed_bytes: raw_bytes,
            stdout: raw,
        }),
    }
}

fn rtk_available() -> bool {
    Command::new("rtk")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn pipe_through_rtk(input: &str) -> Result<String> {
    use std::io::Write;
    let mut child = Command::new("rtk")
        .arg("compress")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| KgError::Other(e.to_string()))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| KgError::Other("failed to open stdin".into()))?
        .write_all(input.as_bytes())
        .map_err(|e| KgError::Other(e.to_string()))?;
    let out = child
        .wait_with_output()
        .map_err(|e| KgError::Other(e.to_string()))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
